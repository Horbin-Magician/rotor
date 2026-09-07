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

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileState {
    Unavailable,
    #[serde(rename = "unbuilt")]
    Unbuild,
    Building,
    Released,
    Loading,
    Ready,
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
            FileState::Error => "error",
        }
    }
}

struct VolumePack {
    volume: Arc<Mutex<Volume>>,
    find_sender: mpsc::Sender<VolumeFindTask>,
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

        for VolumePack { find_sender, .. } in volume_packs {
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
                        let ok = file_data.init_volumes();
                        file_data.set_state(if ok {
                            FileState::Released
                        } else {
                            FileState::Error
                        });
                    }
                    Ok(SearcherMessage::Update) => {
                        file_data.set_state(FileState::Loading);
                        let ok = file_data.update_index();
                        file_data.set_state(if ok {
                            FileState::Ready
                        } else {
                            FileState::Error
                        });
                    }
                    Ok(SearcherMessage::Find(filename)) => match file_data.state() {
                        FileState::Released => {
                            wait_deals.push_back(SearcherMessage::Update);
                            wait_deals.push_back(SearcherMessage::Find(filename));
                        }
                        FileState::Ready => {
                            let rtn = file_data.find(filename, &msg_reciever);
                            if let Some(rtn) = rtn {
                                wait_deals.push_back(rtn);
                            }
                        }
                        _ => {}
                    },
                    Ok(SearcherMessage::Release) => {
                        if let FileState::Ready = file_data.state() {
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

            self.volume_packs.retain(|volume_pack| {
                if let Ok(volume) = volume_pack.volume.lock() {
                    return self.vols.contains(&volume.drive.to_string());
                }
                false
            });

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

    pub fn init_volumes(&mut self) -> bool {
        self.volume_packs.clear();
        self.update_valid_vols();

        let handles = self
            .vols
            .iter()
            .map(|c| {
                let (find_sender, find_receiver) = mpsc::channel::<VolumeFindTask>();

                let volume = Arc::new(Mutex::new(Volume::new(c.clone())));
                self.volume_packs.push(VolumePack {
                    volume: volume.clone(),
                    find_sender,
                });

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

                thread::spawn(move || {
                    volume
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .build_index();
                })
            })
            .collect::<Vec<_>>();

        let mut ok = true;
        for handle in handles {
            if let Err(e) = handle.join() {
                log::error!("Init volume failed: {:?}", e);
                ok = false;
            }
        }
        ok && !self.volume_packs.is_empty()
    }

    pub fn update_index(&mut self) -> bool {
        self.update_valid_vols();

        let handles = self
            .volume_packs
            .iter()
            .map(|VolumePack { volume, .. }| {
                let volume = volume.clone();
                thread::spawn(move || {
                    volume
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .update_index();
                })
            })
            .collect::<Vec<_>>();

        let mut ok = true;
        for handle in handles {
            if let Err(e) = handle.join() {
                log::error!("Update index failed: {:?}", e);
                ok = false;
            }
        }
        ok && !self.volume_packs.is_empty()
    }

    pub fn release_index(&mut self) -> bool {
        self.update_valid_vols();

        self.finding_name = String::new();
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
