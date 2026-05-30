mod volume;

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
pub use volume::SearchResultItem;

pub enum SearcherMessage {
    Init,
    Update,
    Find(String),
    Release,
}

const SEARCH_WAIT_TIMEOUT: Duration = Duration::from_millis(50);
const SEARCH_CANCEL_DRAIN_TIMEOUT: Duration = Duration::from_millis(200);

#[derive(Debug)]
enum FileState {
    Unbuild,
    Released,
    Ready,
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
    state: FileState,
    show_num: usize,
    batch: u8,
    find_result_callback: Box<dyn Fn(String, Vec<SearchResultItem>, bool) + Send>,
}

impl FileData {
    pub fn new<F>(find_result_callback: F) -> FileData
    where
        F: Fn(String, Vec<SearchResultItem>, bool) + Send + 'static,
    {
        FileData {
            vols: Vec::new(),
            volume_packs: Vec::new(),
            finding_name: String::new(),
            finding_result: SearchResult {
                items: Vec::new(),
                query: String::new(),
            },
            state: FileState::Unbuild,
            show_num: 20,
            batch: 20,
            find_result_callback: Box::new(find_result_callback),
        }
    }

    pub fn event_loop(msg_reciever: mpsc::Receiver<SearcherMessage>, mut file_data: FileData) {
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
                        file_data.init_volumes();
                        file_data.state = FileState::Released;
                    }
                    Ok(SearcherMessage::Update) => {
                        file_data.update_index();
                        file_data.state = FileState::Ready;
                    }
                    Ok(SearcherMessage::Find(filename)) => match file_data.state {
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
                        if let FileState::Ready = file_data.state {
                            file_data.release_index();
                            file_data.state = FileState::Released;
                        }
                    }
                    Err(_) => {}
                }
            }
        });
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
        filename: String,
        update_result: Vec<SearchResultItem>,
        if_increase: bool,
    ) {
        self.show_num += update_result.len();
        (self.find_result_callback)(filename, update_result, if_increase);
    }

    pub fn find(
        &mut self,
        filename: String,
        msg_reciever: &mpsc::Receiver<SearcherMessage>,
    ) -> Option<SearcherMessage> {
        let mut reply: Option<SearcherMessage> = None;
        let mut if_increase = false;
        let need_num;

        if self.finding_name == filename {
            need_num = self.show_num + self.batch as usize;
            if_increase = true;
            if self.finding_result.items.len() >= need_num {
                let return_result = self.finding_result.items[self.show_num..need_num].to_vec();
                self.find_result(filename, return_result, if_increase);
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
            if let Ok(searcher_msg) = msg_reciever.try_recv() {
                task.cancel();
                task.drain_cancelled();
                self.finding_result.items.clear();
                reply = Some(searcher_msg);
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
                .sort_by(|a, b| b.rank.cmp(&a.rank)); // sort by rank
            let return_result = if self.finding_result.items.len() > self.show_num {
                let max = std::cmp::min(self.finding_result.items.len(), need_num);
                self.finding_result.items[self.show_num..max].to_vec()
            } else {
                vec![]
            };
            self.find_result(filename, return_result, if_increase);
        }

        reply
    }

    pub fn init_volumes(&mut self) {
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

        for handle in handles {
            if let Err(e) = handle.join() {
                log::error!("Init volume failed: {:?}", e);
            }
        }
    }

    pub fn update_index(&mut self) {
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

        for handle in handles {
            if let Err(e) = handle.join() {
                log::error!("Update index failed: {:?}", e);
            }
        }
    }

    pub fn release_index(&mut self) {
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

        for handle in handles {
            if let Err(e) = handle.join() {
                log::error!("Release index failed: {:?}", e);
            }
        }
    }
}
