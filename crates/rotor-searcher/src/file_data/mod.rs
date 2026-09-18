mod paging;
mod volume;
use crate::{latest, mailbox, QueryId, SearchBatch, SearchRequest};
use paging::MergePages;
use volume::{SearchCursor, SearchPage};

mod excluded_dirs;
use std::collections::VecDeque;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem;

#[cfg(target_os = "windows")]
use rotor_platform::sys_util::is_ntfs;
#[cfg(target_os = "macos")]
use volume::default_volume::Volume;
#[cfg(target_os = "windows")]
use volume::ntfs_volume::Volume;
pub use volume::{SearchResultItem, VolumeIndexStatus};

pub enum SearcherMessage {
    Startup,
    Init,
    Update,
    Find(SearchRequest),
    Release,
    Shutdown,
}

#[derive(Clone, Debug)]

pub struct SearchIndexStatus {
    pub state: FileState,
    pub volume_count: usize,
    pub indexed_volume_count: usize,
    pub index_item_count: usize,
    pub index_file_size_bytes: u64,
    pub latest_index_modified_at: Option<u64>,
    pub volumes: Vec<VolumeIndexStatus>,
}

impl SearchIndexStatus {
    pub fn empty() -> Self {
        Self {
            state: FileState::Unavailable,
            volume_count: 0,
            indexed_volume_count: 0,
            index_item_count: 0,
            index_file_size_bytes: 0,
            latest_index_modified_at: None,
            volumes: Vec::new(),
        }
    }
}

const SEARCH_WAIT_TIMEOUT: Duration = Duration::from_millis(10);
/// Shutdown checkpoints dirty volumes like Release, but exit must not wait on a
/// slow snapshot write; atomic replacement keeps an abandoned write harmless.
const SHUTDOWN_CHECKPOINT_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum FileState {
    Unavailable,

    Unbuild,
    Building,
    Released,
    Loading,
    Ready,
    Partial,
    Error,
}

pub(crate) type SharedFileState = Arc<Mutex<FileState>>;

/// Cancellation (Release, Init or Shutdown pre-empting the work) is not a
/// failure: interrupted volumes already dropped their index, so nothing was
/// loaded and the follow-up command sees a released index rather than an error.
fn refresh_state(results: impl IntoIterator<Item = std::io::Result<()>>) -> FileState {
    let mut succeeded = 0;
    let mut failed = 0;
    let mut interrupted = 0;
    for result in results {
        match result {
            Ok(()) => succeeded += 1,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {
                interrupted += 1;
                log::info!("Index refresh cancelled: {error}");
            }
            Err(error) => {
                failed += 1;
                log::error!("Index refresh failed: {error}");
            }
        }
    }
    match (succeeded, failed, interrupted) {
        (0, 0, 1..) => FileState::Released,
        (0, _, _) => FileState::Error,
        (_, 0, 0) => FileState::Ready,
        _ => FileState::Partial,
    }
}

struct VolumePack {
    drive: String,
    available: bool,
    volume: Arc<Mutex<Volume>>,
    find_sender: latest::Sender<VolumeFindTask>,
}

impl VolumePack {
    fn new(drive: String) -> Self {
        let (find_sender, find_receiver) = latest::channel::<VolumeFindTask>();
        let volume = Arc::new(Mutex::new(Volume::new(drive.clone())));
        let worker_volume = volume.clone();
        thread::spawn(move || {
            while let Ok(task) = find_receiver.recv() {
                if task.cancel.load(Ordering::Relaxed) {
                    let _ = task.result_sender.send((task.volume_index, None));
                    continue;
                }
                let result = worker_volume
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .find(task.filename, task.cursor, task.batch, task.cancel);
                let _ = task.result_sender.send((task.volume_index, result));
            }
        });
        Self {
            drive,
            available: true,
            volume,
            find_sender,
        }
    }
}

struct VolumeFindTask {
    volume_index: usize,
    cursor: Option<SearchCursor>,
    filename: String,
    batch: u8,
    cancel: Arc<AtomicBool>,
    result_sender: mpsc::Sender<(usize, Option<SearchPage>)>,
}

struct SearchTask {
    cancel: Arc<AtomicBool>,
    result_receiver: mpsc::Receiver<(usize, Option<SearchPage>)>,
    pending: usize,
}

impl SearchTask {
    fn dispatch(
        volume_packs: &[VolumePack],
        pages: &mut MergePages,
        filename: String,
        batch: u8,
        cancel: Arc<AtomicBool>,
    ) -> SearchTask {
        let (result_sender, result_receiver) = mpsc::channel::<(usize, Option<SearchPage>)>();
        let mut pending = 0;

        for (volume_index, pack) in volume_packs.iter().enumerate() {
            if !pages.needs_page(volume_index) {
                continue;
            }
            if !pack.available {
                pages.accept(volume_index, None);
                continue;
            }
            let task = VolumeFindTask {
                volume_index,
                cursor: pages.cursor(volume_index),
                filename: filename.clone(),
                batch,
                cancel: cancel.clone(),
                result_sender: result_sender.clone(),
            };

            if pack.find_sender.send(task).is_err() {
                pages.accept(volume_index, None);
                log::error!("Dispatch search task failed");
                continue;
            }
            pending += 1;
        }

        SearchTask {
            cancel,
            result_receiver,
            pending,
        }
    }

    fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

pub struct FileData {
    pub(crate) icons: Option<crate::icons::IconWorker>,
    pub(crate) snapshot: Arc<Mutex<SearchIndexStatus>>,
    work_cancel: Arc<AtomicBool>,
    vols: Vec<String>,
    finding_name: String,
    pages: MergePages,
    volume_packs: Vec<VolumePack>,
    /// Volumes hold an in-memory index (or dirty journal progress). Startup
    /// releases every volume even when the resulting state is `Partial`.
    loaded: bool,
    state: SharedFileState,
    batch: u8,
    find_result_callback: Box<dyn Fn(SearchBatch) + Send>,
    state_change_callback: Option<Box<dyn Fn(FileState) + Send>>,
}

impl FileData {
    pub(crate) fn new<F>(
        find_result_callback: F,
        state_change_callback: Option<Box<dyn Fn(FileState) + Send>>,
        state: SharedFileState,
    ) -> FileData
    where
        F: Fn(SearchBatch) + Send + 'static,
    {
        FileData {
            icons: None,
            snapshot: Arc::new(Mutex::new(SearchIndexStatus::empty())),
            work_cancel: Arc::new(AtomicBool::new(false)),
            vols: Vec::new(),
            volume_packs: Vec::new(),
            loaded: false,
            finding_name: String::new(),
            pages: MergePages::default(),
            state,
            batch: 20,
            find_result_callback: Box::new(find_result_callback),
            state_change_callback,
        }
    }

    pub(crate) fn event_loop(
        msg_reciever: mailbox::Receiver,
        mut file_data: FileData,
    ) -> thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let mut wait_deals: VecDeque<SearcherMessage> = VecDeque::new();
            loop {
                let msg: Result<SearcherMessage, mpsc::RecvError> = if !wait_deals.is_empty() {
                    wait_deals.pop_front().ok_or(mpsc::RecvError)
                } else {
                    msg_reciever.recv()
                };

                file_data.work_cancel = Arc::new(AtomicBool::new(false));
                msg_reciever.activate(file_data.work_cancel.clone(), false);
                match msg {
                    Ok(SearcherMessage::Startup) => {
                        file_data.set_state(FileState::Building);
                        file_data.init_volumes(false);
                    }
                    Ok(SearcherMessage::Init) => {
                        file_data.set_state(FileState::Building);
                        file_data.init_volumes(true);
                    }
                    Ok(SearcherMessage::Update) => {
                        file_data.set_state(FileState::Loading);
                        let state = file_data.update_index();
                        file_data.set_state(state);
                        if file_data.work_cancel.load(Ordering::Acquire) {
                            // Only Release, Init or Shutdown cancel a refresh and
                            // they run next. A deferred search would re-queue the
                            // refresh ahead of them forever, so complete it now.
                            file_data.discard_deferred(&mut wait_deals);
                        }
                    }
                    Ok(SearcherMessage::Find(filename)) if file_data.needs_reload() => {
                        wait_deals.push_back(SearcherMessage::Update);
                        wait_deals.push_back(SearcherMessage::Find(filename));
                    }
                    Ok(SearcherMessage::Find(filename)) => match file_data.state() {
                        FileState::Ready | FileState::Partial => {
                            wait_deals.extend(file_data.find(filename, &msg_reciever));
                        }
                        _ => {
                            // The accepted request must finish even when every
                            // volume failed; otherwise the view stays loading.
                            file_data.find_result(filename.id, filename.query, Vec::new(), false);
                        }
                    },
                    Ok(SearcherMessage::Release) => {
                        if matches!(file_data.state(), FileState::Ready | FileState::Partial) {
                            let ok = file_data.release_index();
                            file_data.set_state(if ok {
                                FileState::Released
                            } else {
                                FileState::Error
                            });
                        }
                    }
                    Ok(SearcherMessage::Shutdown) | Err(_) => {
                        if file_data.loaded {
                            file_data.release_index_within(Some(SHUTDOWN_CHECKPOINT_TIMEOUT));
                        }
                        break;
                    }
                }
                msg_reciever.deactivate();
                // All maintenance workers have completed here. Status readers
                // use this snapshot and never contend with a volume worker.
                file_data.publish_status();
            }
        })
    }

    fn state(&self) -> FileState {
        *self.state.lock().unwrap_or_else(|poisoned| {
            log::error!("Search index state lock poisoned; recovering inner state");
            poisoned.into_inner()
        })
    }

    /// A released index must refresh before serving a search. A partial startup
    /// is released too, unlike a partial refresh whose volumes stay loaded.
    fn needs_reload(&self) -> bool {
        match self.state() {
            FileState::Released => true,
            FileState::Partial => !self.loaded,
            _ => false,
        }
    }

    /// Complete deferred searches with an empty batch so the view stops loading.
    fn discard_deferred(&mut self, wait_deals: &mut VecDeque<SearcherMessage>) {
        for message in wait_deals.drain(..) {
            if let SearcherMessage::Find(request) = message {
                self.find_result(request.id, request.query, Vec::new(), false);
            }
        }
    }

    fn set_state(&self, next_state: FileState) {
        let changed = {
            let mut state = self.state.lock().unwrap_or_else(|poisoned| {
                log::error!("Search index state lock poisoned; recovering inner state");
                poisoned.into_inner()
            });
            if *state == next_state {
                false
            } else {
                *state = next_state;
                true
            }
        };

        if changed {
            if let Some(callback) = &self.state_change_callback {
                callback(next_state);
            }
        }
    }

    fn update_valid_vols(&mut self) -> u8 {
        #[cfg(target_os = "windows")]
        {
            let mut bit_mask = unsafe { FileSystem::GetLogicalDrives() };
            self.vols.clear();
            let mut vol = 'A';
            while bit_mask != 0 {
                if bit_mask & 0x1 != 0 && is_ntfs(vol) {
                    self.vols.push(vol.to_string());
                }
                vol = (vol as u8 + 1) as char;
                bit_mask >>= 1;
            }

            self.vols.len() as u8
        }
        #[cfg(target_os = "macos")]
        {
            self.vols.clear();

            match std::env::var("HOME") {
                Ok(home) => self.vols.push(home),
                Err(error) => log::warn!("Failed to read HOME for search volumes: {error}"),
            }
            self.vols.push("/Applications".to_string());
            self.vols.push("/System/Applications".to_string());

            self.vols.len() as u8
        }
    }

    fn find_result(
        &mut self,
        id: QueryId,
        filename: String,
        update_result: Vec<SearchResultItem>,
        if_increase: bool,
    ) {
        let paths = update_result
            .iter()
            .map(|item| item.file_path.clone())
            .collect();
        (self.find_result_callback)(SearchBatch {
            id,
            query: filename,
            items: update_result,
            append: if_increase,
        });
        if let Some(icons) = &mut self.icons {
            icons.enqueue(id, if_increase, paths);
        }
    }

    /// Returns the commands to run next: empty once the request completed, or
    /// the pre-empting command (plus this request again after an `Update`).
    pub(crate) fn find(
        &mut self,
        request: SearchRequest,
        msg_reciever: &mailbox::Receiver,
    ) -> Vec<SearcherMessage> {
        let cancel = Arc::new(AtomicBool::new(false));
        msg_reciever.activate(cancel.clone(), true);
        let append = request.append && self.finding_name == request.query;
        if !append {
            if let Some(icons) = &mut self.icons {
                icons.reset();
            }
            self.finding_name = request.query.clone();
            self.pages = MergePages::new(self.volume_packs.len());
        }
        if request.query.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::with_capacity(self.batch as usize);
        while result.len() < self.batch as usize {
            if cancel.load(Ordering::Acquire) {
                return self.interrupted_search(request, msg_reciever);
            }
            if self.pages.needs_refill() {
                let mut task = SearchTask::dispatch(
                    &self.volume_packs,
                    &mut self.pages,
                    request.query.clone(),
                    self.batch,
                    cancel.clone(),
                );
                while task.pending > 0 {
                    if cancel.load(Ordering::Acquire) {
                        task.cancel();
                        return self.interrupted_search(request, msg_reciever);
                    }
                    match task.result_receiver.recv_timeout(SEARCH_WAIT_TIMEOUT) {
                        Ok((index, page)) => {
                            task.pending -= 1;
                            self.pages.accept(index, page);
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            self.pages.finish_missing();
                            break;
                        }
                    }
                }
            }
            let Some(item) = self.pages.pop_best() else {
                break;
            };
            result.push(item);
        }
        self.find_result(request.id, request.query, result, append);
        Vec::new()
    }

    /// A newer command pre-empted this search. A superseding search, Release,
    /// Init or Shutdown drops it; an Update runs it again afterwards so the
    /// accepted request still completes.
    fn interrupted_search(
        &mut self,
        request: SearchRequest,
        msg_reciever: &mailbox::Receiver,
    ) -> Vec<SearcherMessage> {
        self.reset_search_results();
        match msg_reciever.try_recv() {
            Ok(next @ SearcherMessage::Update) => vec![next, SearcherMessage::Find(request)],
            Ok(next) => vec![next],
            Err(_) => Vec::new(),
        }
    }

    pub fn init_volumes(&mut self, rebuild: bool) {
        self.reset_search_results();
        self.volume_packs.clear();
        self.update_valid_vols();

        let handles = self
            .vols
            .iter()
            .map(|c| {
                let pack = VolumePack::new(c.clone());
                let volume = pack.volume.clone();
                self.volume_packs.push(pack);
                let cancel = self.work_cancel.clone();
                thread::spawn(move || {
                    let mut volume = volume
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let result = if rebuild {
                        volume.build_index_with_cancel(Some(&cancel))
                    } else {
                        volume.initialize_index(&cancel)
                    };
                    result.map_err(|error| {
                        std::io::Error::new(error.kind(), format!("{}: {error}", volume.drive))
                    })
                })
            })
            .collect::<Vec<_>>();

        self.finish_index_builds(handles);
    }

    fn finish_index_builds(&mut self, handles: Vec<thread::JoinHandle<std::io::Result<()>>>) {
        let state = refresh_state(handles.into_iter().enumerate().map(|(index, handle)| {
            let result = match handle.join() {
                Ok(result) => result,
                Err(error) => Err(std::io::Error::other(format!(
                    "Volume build worker panicked: {error:?}"
                ))),
            };
            if let Some(pack) = self.volume_packs.get_mut(index) {
                pack.available = result.is_ok();
            }
            result
        }));
        self.loaded = false;
        self.set_state(if state == FileState::Ready {
            FileState::Released
        } else {
            state
        });
    }

    fn reset_search_results(&mut self) {
        if let Some(icons) = &mut self.icons {
            icons.reset();
        }
        self.finding_name.clear();
        self.pages = MergePages::default();
    }

    fn sync_volume_packs(&mut self) {
        self.volume_packs
            .retain(|pack| self.vols.contains(&pack.drive));
        for drive in &self.vols {
            if !self.volume_packs.iter().any(|pack| pack.drive == *drive) {
                self.volume_packs.push(VolumePack::new(drive.clone()));
            }
        }
    }

    pub fn update_index(&mut self) -> FileState {
        self.update_valid_vols();
        self.sync_volume_packs();
        self.refresh_volume_packs()
    }

    fn refresh_volume_packs(&mut self) -> FileState {
        self.reset_search_results();

        let handles = self
            .volume_packs
            .iter()
            .map(|VolumePack { volume, .. }| {
                let volume = volume.clone();
                let cancel = self.work_cancel.clone();
                thread::spawn(move || {
                    volume
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .update_index_with_cancel(Some(&cancel))
                })
            })
            .collect::<Vec<_>>();

        let state = refresh_state(self.volume_packs.iter_mut().zip(handles).map(
            |(pack, handle)| {
                let result = match handle.join() {
                    Ok(result) => result,
                    Err(error) => {
                        let volume = pack
                            .volume
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        Err(std::io::Error::other(format!(
                            "{}: volume worker panicked: {error:?}",
                            volume.drive
                        )))
                    }
                };
                pack.available = result.is_ok();
                result
            },
        ));
        self.loaded = matches!(state, FileState::Ready | FileState::Partial);
        state
    }

    pub fn release_index(&mut self) -> bool {
        self.release_index_within(None)
    }

    /// Checkpoint and drop every volume index. With a timeout, volumes that
    /// have not finished by the deadline are abandoned rather than awaited.
    fn release_index_within(&mut self, timeout: Option<Duration>) -> bool {
        // Only the existing volume packs are released; re-enumerating drives here
        // would block on network volumes without changing what gets checkpointed.
        self.reset_search_results();
        self.loaded = false;
        let (result_sender, result_receiver) = mpsc::channel();
        for VolumePack { volume, .. } in &self.volume_packs {
            let volume = volume.clone();
            let result_sender = result_sender.clone();
            thread::spawn(move || {
                let result = volume
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .release_index();
                let _ = result_sender.send(result);
            });
        }
        drop(result_sender);

        let deadline = timeout.map(|timeout| Instant::now() + timeout);
        let mut ok = !self.volume_packs.is_empty();
        for _ in 0..self.volume_packs.len() {
            let result = match deadline {
                Some(deadline) => {
                    result_receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                }
                None => result_receiver
                    .recv()
                    .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
            };
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    log::error!("Release index failed: {error}");
                    ok = false;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    log::warn!("Release index timed out; remaining checkpoints abandoned");
                    return false;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    log::error!("Release index failed: volume worker panicked");
                    ok = false;
                    break;
                }
            }
        }
        ok
    }

    fn publish_status(&self) {
        let status = self.index_status();
        *self
            .snapshot
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = status;
    }

    pub fn index_status(&self) -> SearchIndexStatus {
        let previous = self
            .snapshot
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        let volumes = self
            .volume_packs
            .iter()
            .map(|pack| {
                if let Ok(volume) = pack.volume.try_lock() {
                    return volume.index_status();
                }
                previous
                    .volumes
                    .iter()
                    .find(|volume| volume.name == pack.drive)
                    .cloned()
                    .unwrap_or(VolumeIndexStatus {
                        name: pack.drive.clone(),
                        indexed: false,
                        index_item_count: None,
                        index_file_size_bytes: 0,
                        index_file_modified_at: None,
                    })
            })
            .collect::<Vec<_>>();

        let indexed_volume_count = volumes.iter().filter(|volume| volume.indexed).count();
        let index_item_count = volumes
            .iter()
            .filter_map(|volume| volume.index_item_count)
            .sum::<usize>();
        let index_file_size_bytes = volumes
            .iter()
            .map(|volume| volume.index_file_size_bytes)
            .sum::<u64>();
        let latest_index_modified_at = volumes
            .iter()
            .filter_map(|volume| volume.index_file_modified_at)
            .max();

        SearchIndexStatus {
            state: self.state(),
            volume_count: volumes.len(),
            indexed_volume_count,
            index_item_count,
            index_file_size_bytes,
            latest_index_modified_at,
            volumes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_distinguishes_partial_all_failed_and_ready() {
        assert_eq!(refresh_state([]), FileState::Error);
        assert_eq!(
            refresh_state([Err(std::io::Error::other("A: denied"))]),
            FileState::Error
        );
        assert_eq!(
            refresh_state([Ok(()), Err(std::io::Error::other("B: unreadable"))]),
            FileState::Partial
        );
        assert_eq!(refresh_state([Ok(()), Ok(())]), FileState::Ready);
    }

    #[test]
    fn volume_sync_adds_removes_and_reuses_workers() {
        let mut data = FileData::new(|_| {}, None, Arc::new(Mutex::new(FileState::Unbuild)));
        data.vols = vec!["A".into()];
        data.sync_volume_packs();
        let first = data.volume_packs[0].volume.clone();
        data.vols.push("B".into());
        data.sync_volume_packs();
        assert_eq!(data.volume_packs.len(), 2);
        assert!(Arc::ptr_eq(&first, &data.volume_packs[0].volume));
        let second = data.volume_packs[1].volume.clone();
        data.vols = vec!["B".into()];
        data.sync_volume_packs();
        assert_eq!(data.volume_packs.len(), 1);
        assert!(Arc::ptr_eq(&second, &data.volume_packs[0].volume));
        data.sync_volume_packs();
        assert_eq!(data.volume_packs.len(), 1);
    }

    #[test]
    fn failed_volume_build_reports_partial_and_successful_retry_reports_released() {
        let observed = Arc::new(Mutex::new(Vec::new()));
        let callback_states = observed.clone();
        let mut data = FileData::new(
            |_| {},
            Some(Box::new(move |state| {
                callback_states.lock().unwrap().push(state)
            })),
            Arc::new(Mutex::new(FileState::Unbuild)),
        );
        data.set_state(FileState::Building);
        let completed = Arc::new(AtomicBool::new(false));
        let worker_completed = completed.clone();
        data.finish_index_builds(vec![
            thread::spawn(|| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "synthetic cache write failure",
                ))
            }),
            thread::spawn(move || {
                worker_completed.store(true, Ordering::Release);
                Ok(())
            }),
        ]);
        assert_eq!(data.state(), FileState::Partial);
        assert!(
            completed.load(Ordering::Acquire),
            "all volume workers must finish before publishing state"
        );
        assert_eq!(
            *observed.lock().unwrap(),
            vec![FileState::Building, FileState::Partial]
        );

        data.set_state(FileState::Building);
        data.finish_index_builds(vec![thread::spawn(|| Ok(())), thread::spawn(|| Ok(()))]);
        assert_eq!(data.state(), FileState::Released);
        assert_eq!(
            *observed.lock().unwrap(),
            vec![
                FileState::Building,
                FileState::Partial,
                FileState::Building,
                FileState::Released
            ]
        );
    }

    #[test]
    fn missing_or_panicked_volume_builds_never_report_released() {
        let mut data = FileData::new(|_| {}, None, Arc::new(Mutex::new(FileState::Building)));
        data.finish_index_builds(Vec::new());
        assert_eq!(data.state(), FileState::Error);
        data.set_state(FileState::Building);
        data.finish_index_builds(vec![thread::spawn(|| panic!("synthetic build panic"))]);
        assert_eq!(data.state(), FileState::Error);
    }

    #[test]
    fn refresh_and_release_drop_cached_results_and_restart_query_paging() {
        for refresh in [false, true] {
            let batches = Arc::new(Mutex::new(Vec::new()));
            let observed = batches.clone();
            let mut data = FileData::new(
                move |batch| observed.lock().unwrap().push(batch),
                None,
                Arc::new(Mutex::new(FileState::Ready)),
            );
            data.finding_name = "same-query".into();
            data.pages = MergePages::new(1);
            data.pages.accept(
                0,
                Some(SearchPage {
                    items: vec![SearchResultItem {
                        path: "fixture".repeat(1024),
                        file_path: "fixture/file".into(),
                        file_name: "file".into(),
                        rank: 0,
                        alias: None,
                    }],
                    cursor: None,
                    exhausted: true,
                }),
            );
            // No volume workers are attached: even a failed/missing-volume release
            // must relinquish result ownership and old paging state.
            if refresh {
                assert_eq!(data.refresh_volume_packs(), FileState::Error);
            } else {
                let _ = data.release_index();
            }
            assert_eq!(data.pages.buffered_len(), 0);
            assert!(data.finding_name.is_empty());
            let (_sender, receiver) = mailbox::channel();
            data.find(
                SearchRequest {
                    id: QueryId(20),
                    append: false,
                    query: "same-query".into(),
                },
                &receiver,
            );
            let batches = batches.lock().unwrap();
            assert_eq!(batches.len(), 1);
            assert!(!batches[0].append);
            assert!(batches[0].items.is_empty());
        }
    }

    #[test]
    fn repeated_query_batches_keep_the_original_request_identity() {
        let batches = Arc::new(Mutex::new(Vec::new()));
        let observed = batches.clone();
        let mut data = FileData::new(
            move |batch| observed.lock().unwrap().push(batch),
            None,
            Arc::new(Mutex::new(FileState::Ready)),
        );
        let (_sender, receiver) = mailbox::channel();
        for id in [QueryId(10), QueryId(11), QueryId(12)] {
            data.find(
                SearchRequest {
                    id,
                    append: id == QueryId(11),
                    query: "same-query".into(),
                },
                &receiver,
            );
        }
        let batches = batches.lock().unwrap();
        assert_eq!(batches.len(), 3);
        assert!(!batches[2].append);
        assert_eq!(batches[2].id, QueryId(12));
        assert_eq!(batches[0].id, QueryId(10));
        assert_eq!(batches[1].id, QueryId(11));
        assert!(!batches[0].append);
        assert!(batches[1].append);
    }

    #[test]
    fn unavailable_index_completes_accepted_query_without_loading_forever() {
        let (batches, results) = mpsc::channel();
        let data = FileData::new(
            move |batch| {
                batches.send(batch).unwrap();
            },
            None,
            Arc::new(Mutex::new(FileState::Error)),
        );
        let (sender, receiver) = mailbox::channel();
        let worker = FileData::event_loop(receiver, data);
        sender
            .send(SearcherMessage::Find(SearchRequest {
                id: QueryId(1),
                query: "fixture".into(),
                append: false,
            }))
            .unwrap();
        let batch = results.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(batch.id, QueryId(1));
        assert!(batch.items.is_empty());
        sender.send(SearcherMessage::Shutdown).unwrap();
        worker.join().unwrap();
    }

    #[test]
    fn disconnected_request_channel_terminates_instead_of_spinning() {
        let data = FileData::new(|_| {}, None, Arc::new(Mutex::new(FileState::Unbuild)));
        let (sender, receiver) = mailbox::channel();
        let worker = FileData::event_loop(receiver, data);
        drop(sender);
        let deadline = Instant::now() + Duration::from_secs(1);
        while !worker.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            worker.is_finished(),
            "disconnected event loop did not terminate"
        );
        worker.join().unwrap();
    }

    #[test]
    fn state_change_callback_only_runs_for_new_states() {
        let state = Arc::new(Mutex::new(FileState::Unbuild));
        let observed_states = Arc::new(Mutex::new(Vec::new()));
        let callback_states = observed_states.clone();
        let file_data = FileData::new(
            |_| {},
            Some(Box::new(move |state| {
                callback_states
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(state);
            })),
            state,
        );

        file_data.set_state(FileState::Building);
        file_data.set_state(FileState::Building);
        file_data.set_state(FileState::Ready);

        assert_eq!(
            *observed_states
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec![FileState::Building, FileState::Ready]
        );
    }
}
