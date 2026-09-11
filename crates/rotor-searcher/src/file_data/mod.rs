mod volume;
use crate::{QueryId, SearchBatch, SearchRequest};

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
    Init,
    Update,
    Find(SearchRequest),
    Release,
    Shutdown,
    Status(mpsc::Sender<SearchIndexStatus>),
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

const SEARCH_WAIT_TIMEOUT: Duration = Duration::from_millis(50);
const SEARCH_CANCEL_DRAIN_TIMEOUT: Duration = Duration::from_millis(200);

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

impl FileState {
    pub fn as_str(self) -> &'static str {
        match self {
            FileState::Unavailable => "unavailable",
            FileState::Unbuild => "unbuilt",
            FileState::Building => "building",
            FileState::Released => "released",
            FileState::Loading => "loading",
            FileState::Ready => "ready",
            FileState::Partial => "partial",
            FileState::Error => "error",
        }
    }
}

fn refresh_state(results: impl IntoIterator<Item = std::io::Result<()>>) -> FileState {
    let mut succeeded = 0;
    let mut failed = 0;
    for result in results {
        match result {
            Ok(()) => succeeded += 1,
            Err(error) => {
                failed += 1;
                log::error!("Index refresh failed: {error}");
            }
        }
    }
    match (succeeded, failed) {
        (0, _) => FileState::Error,
        (_, 0) => FileState::Ready,
        _ => FileState::Partial,
    }
}

struct VolumePack {
    available: bool,
    volume: Arc<Mutex<Volume>>,
    find_sender: mpsc::Sender<VolumeFindTask>,
}

impl VolumePack {
    fn new(drive: String) -> Self {
        let (find_sender, find_receiver) = mpsc::channel::<VolumeFindTask>();
        let volume = Arc::new(Mutex::new(Volume::new(drive)));
        let worker_volume = volume.clone();
        thread::spawn(move || {
            while let Ok(task) = find_receiver.recv() {
                if task.cancel.load(Ordering::Relaxed) {
                    let _ = task.result_sender.send(None);
                    continue;
                }
                worker_volume
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .find(task.filename, task.batch, task.cancel, task.result_sender);
            }
        });
        Self {
            available: true,
            volume,
            find_sender,
        }
    }
}

struct VolumeFindTask {
    filename: String,
    batch: u8,
    cancel: Arc<AtomicBool>,
    result_sender: mpsc::Sender<Option<Vec<SearchResultItem>>>,
}

struct SearchTask {
    cancel: Arc<AtomicBool>,
    result_receiver: mpsc::Receiver<Option<Vec<SearchResultItem>>>,
    pending: usize,
}

impl SearchTask {
    fn dispatch(volume_packs: &[VolumePack], filename: String, batch: u8) -> SearchTask {
        let cancel = Arc::new(AtomicBool::new(false));
        let (result_sender, result_receiver) = mpsc::channel::<Option<Vec<SearchResultItem>>>();
        let mut pending = 0;

        for VolumePack {
            find_sender,
            available,
            ..
        } in volume_packs
        {
            if !available {
                continue;
            }
            let task = VolumeFindTask {
                filename: filename.clone(),
                batch,
                cancel: cancel.clone(),
                result_sender: result_sender.clone(),
            };

            if find_sender.send(task).is_err() {
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

    fn drain_cancelled(&mut self) {
        let deadline = Instant::now() + SEARCH_CANCEL_DRAIN_TIMEOUT;
        while self.pending > 0 {
            let now = Instant::now();
            if now >= deadline {
                break;
            }

            let timeout = std::cmp::min(SEARCH_WAIT_TIMEOUT, deadline - now);
            match self.result_receiver.recv_timeout(timeout) {
                Ok(_) => {
                    self.pending -= 1;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }
    }
}

pub struct SearchResult {
    pub items: Vec<SearchResultItem>,
    pub query: String,
}

pub struct FileData {
    vols: Vec<String>,
    finding_name: String,
    finding_result: SearchResult,
    volume_packs: Vec<VolumePack>,
    state: SharedFileState,
    show_num: usize,
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
            vols: Vec::new(),
            volume_packs: Vec::new(),
            finding_name: String::new(),
            finding_result: SearchResult {
                items: Vec::new(),
                query: String::new(),
            },
            state,
            show_num: 20,
            batch: 20,
            find_result_callback: Box::new(find_result_callback),
            state_change_callback,
        }
    }

    pub(crate) fn event_loop(
        msg_reciever: mpsc::Receiver<SearcherMessage>,
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

                match msg {
                    Ok(SearcherMessage::Init) => {
                        file_data.set_state(FileState::Building);
                        file_data.init_volumes();
                    }
                    Ok(SearcherMessage::Update) => {
                        file_data.set_state(FileState::Loading);
                        let state = file_data.update_index();
                        file_data.set_state(state);
                    }
                    Ok(SearcherMessage::Find(filename)) => match file_data.state() {
                        FileState::Released => {
                            wait_deals.push_back(SearcherMessage::Update);
                            wait_deals.push_back(SearcherMessage::Find(filename));
                        }
                        FileState::Ready | FileState::Partial => {
                            let rtn = file_data.find(filename, &msg_reciever);
                            if let Some(rtn) = rtn {
                                wait_deals.push_back(rtn);
                            }
                        }
                        _ => {}
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
                    Ok(SearcherMessage::Status(sender)) => {
                        if sender.send(file_data.index_status()).is_err() {
                            log::warn!("Send search index status failed");
                        }
                    }
                    Ok(SearcherMessage::Shutdown) | Err(_) => break,
                }
            }
        })
    }

    fn state(&self) -> FileState {
        *self.state.lock().unwrap_or_else(|poisoned| {
            log::error!("Search index state lock poisoned; recovering inner state");
            poisoned.into_inner()
        })
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
        self.show_num += update_result.len();
        let update_result = update_result
            .into_iter()
            .map(SearchResultItem::attach_icon_data)
            .collect();
        (self.find_result_callback)(SearchBatch {
            id,
            query: filename,
            items: update_result,
            append: if_increase,
        });
    }

    pub fn find(
        &mut self,
        request: SearchRequest,
        msg_reciever: &mpsc::Receiver<SearcherMessage>,
    ) -> Option<SearcherMessage> {
        let SearchRequest {
            id,
            query: filename,
        } = request;
        let mut reply: Option<SearcherMessage> = None;
        let mut if_increase = false;
        let need_num;

        if self.finding_name == filename {
            need_num = self.show_num + self.batch as usize;
            if_increase = true;
            if self.finding_result.items.len() >= need_num {
                let return_result = self.finding_result.items[self.show_num..need_num].to_vec();
                self.find_result(id, filename, return_result, if_increase);
                return reply;
            }
        } else {
            self.finding_name = filename.clone();
            need_num = self.batch as usize;
            self.show_num = 0;
            self.finding_result.items.clear();
            self.finding_result.query = filename.clone();
        }

        if filename.is_empty() {
            return reply;
        }

        let mut task = SearchTask::dispatch(&self.volume_packs, filename.clone(), self.batch);

        while task.pending > 0 {
            while let Ok(searcher_msg) = msg_reciever.try_recv() {
                match searcher_msg {
                    SearcherMessage::Status(sender) => {
                        if sender.send(self.index_status()).is_err() {
                            log::warn!("Send search index status failed");
                        }
                    }
                    searcher_msg => {
                        task.cancel();
                        task.drain_cancelled();
                        self.finding_result.items.clear();
                        reply = Some(searcher_msg);
                        break;
                    }
                }
            }

            if reply.is_some() {
                break;
            }

            match task.result_receiver.recv_timeout(SEARCH_WAIT_TIMEOUT) {
                Ok(op_result) => {
                    task.pending -= 1;
                    if let Some(mut result) = op_result {
                        self.finding_result.items.append(&mut result);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }

        if reply.is_none() {
            self.finding_result
                .items
                .sort_by_key(|item| std::cmp::Reverse(item.rank)); // sort by rank desc
            let return_result = if self.finding_result.items.len() > self.show_num {
                let max = std::cmp::min(self.finding_result.items.len(), need_num);
                self.finding_result.items[self.show_num..max].to_vec()
            } else {
                vec![]
            };
            self.find_result(id, filename, return_result, if_increase);
        }

        reply
    }

    pub fn init_volumes(&mut self) {
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

                thread::spawn(move || {
                    let mut volume = volume
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    volume.build_index().map_err(|error| {
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
        self.set_state(if state == FileState::Ready {
            FileState::Released
        } else {
            state
        });
    }

    fn reset_search_results(&mut self) {
        self.finding_name.clear();
        self.finding_result = SearchResult {
            items: Vec::new(),
            query: String::new(),
        };
        self.show_num = 0;
    }

    fn sync_volume_packs(&mut self) {
        self.volume_packs.retain(|pack| {
            let volume = pack
                .volume
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            self.vols.contains(&volume.drive)
        });
        for drive in &self.vols {
            if !self.volume_packs.iter().any(|pack| {
                pack.volume
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .drive
                    == *drive
            }) {
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
                thread::spawn(move || {
                    volume
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .update_index()
                })
            })
            .collect::<Vec<_>>();

        refresh_state(
            self.volume_packs
                .iter_mut()
                .zip(handles)
                .map(|(pack, handle)| {
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
                }),
        )
    }

    pub fn release_index(&mut self) -> bool {
        self.update_valid_vols();

        self.reset_search_results();
        let handles = self
            .volume_packs
            .iter()
            .map(|VolumePack { volume, .. }| {
                let volume = volume.clone();
                thread::spawn(move || {
                    volume
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .release_index();
                })
            })
            .collect::<Vec<_>>();

        let mut ok = true;
        for handle in handles {
            if let Err(e) = handle.join() {
                log::error!("Release index failed: {:?}", e);
                ok = false;
            }
        }
        ok && !self.volume_packs.is_empty()
    }

    pub fn index_status(&self) -> SearchIndexStatus {
        let volumes = self
            .volume_packs
            .iter()
            .filter_map(|VolumePack { volume, .. }| {
                volume
                    .lock()
                    .map(|volume| volume.index_status())
                    .map_err(|error| {
                        log::warn!("Failed to lock volume for index status: {error}");
                        error
                    })
                    .ok()
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
            data.finding_result.query = "same-query".into();
            data.finding_result.items = Vec::with_capacity(128);
            data.finding_result.items.push(SearchResultItem {
                path: "fixture".repeat(1024),
                file_path: "fixture/file".into(),
                file_name: "file".into(),
                rank: 0,
                icon_data: None,
                alias: None,
            });
            data.show_num = 80;
            // No volume workers are attached: even a failed/missing-volume release
            // must relinquish result ownership and old paging state.
            if refresh {
                assert_eq!(data.refresh_volume_packs(), FileState::Error);
            } else {
                let _ = data.release_index();
            }
            assert!(data.finding_result.items.is_empty());
            assert_eq!(data.finding_result.items.capacity(), 0);
            assert!(data.finding_result.query.is_empty());
            assert_eq!(data.show_num, 0);
            let (_sender, receiver) = mpsc::channel();
            data.find(
                SearchRequest {
                    id: QueryId(20),
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
        let (_sender, receiver) = mpsc::channel();
        for id in [QueryId(10), QueryId(11)] {
            data.find(
                SearchRequest {
                    id,
                    query: "same-query".into(),
                },
                &receiver,
            );
        }
        let batches = batches.lock().unwrap();
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].id, QueryId(10));
        assert_eq!(batches[1].id, QueryId(11));
        assert!(!batches[0].append);
        assert!(batches[1].append);
    }

    #[test]
    fn disconnected_request_channel_terminates_instead_of_spinning() {
        let data = FileData::new(|_| {}, None, Arc::new(Mutex::new(FileState::Unbuild)));
        let (sender, receiver) = mpsc::channel();
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
