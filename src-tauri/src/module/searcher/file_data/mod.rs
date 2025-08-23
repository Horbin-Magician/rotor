mod volume;

use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::collections::VecDeque;

#[cfg(target_os = "windows")]
use volume::ntfs_volume::Volume;
#[cfg(target_os = "windows")]
pub use volume::ntfs_file_map::SearchResultItem;
#[cfg(target_os = "windows")]
use windows::Win32::Storage::FileSystem;
#[cfg(target_os = "windows")]
use crate::util::sys_util::is_ntfs;

#[cfg(target_os = "macos")]
use volume::default_volume::Volume;
#[cfg(target_os = "macos")]
pub use volume::default_file_map::SearchResultItem;

pub enum SearcherMessage {
    Init,
    Update,
    Find(String),
    Release,
}

#[derive(Debug)]
enum FileState {
    Unbuild,
    Released,
    Ready,
}

struct VolumePack {
    volume: Arc<Mutex<Volume>>,
    stop_sender: mpsc::Sender<()>,
}

pub struct SearchResult {
    pub items: Vec<SearchResultItem>,
    pub query: String,
}

pub struct FileData {
    vols: Vec<String>,
    finding_name: String,
    finding_result: SearchResult,
    waiting_finder: u8,
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
            finding_result: SearchResult{items: Vec::new(), query: String::new()},
            waiting_finder: 0,
            state: FileState::Unbuild,
            show_num: 20,
            batch: 20,
            find_result_callback: Box::new(find_result_callback),
        }
    }

    pub fn event_loop (
        msg_reciever: mpsc::Receiver<SearcherMessage>,
        mut file_data: FileData
    ) {
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
                    },
                    Ok(SearcherMessage::Update) => {
                        file_data.update_index();
                        file_data.state = FileState::Ready;
                    },
                    Ok(SearcherMessage::Find(filename)) => {
                        match file_data.state {
                            FileState::Released => { 
                                wait_deals.push_back(SearcherMessage::Update);
                                wait_deals.push_back(SearcherMessage::Find(filename));
                            },
                            FileState::Ready => {
                                let rtn = file_data.find(filename, &msg_reciever);
                                if let Some(rtn) = rtn {
                                    wait_deals.push_back(rtn);
                                }
                            },
                            _ => {},
                        }
                    },
                    Ok(SearcherMessage::Release) => {
                        if let FileState::Ready = file_data.state { 
                            file_data.release_index();
                            file_data.state = FileState::Released;
                        }
                    },
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
                if bit_mask & 0x1 != 0 && is_ntfs(vol) { self.vols.push(vol.to_string()); }
                vol = (vol as u8 + 1) as char;
                bit_mask >>= 1;
            }

            self.volume_packs.retain(|volume_pack| {
                if let Ok(volume)  = volume_pack.volume.lock() {
                    return self.vols.contains(&volume.drive.to_string());
                }
                false
            });

            self.vols.len() as u8
        }
        #[cfg(target_os = "macos")]
        {
            self.vols.clear();

            let home = std::env::var("HOME").unwrap();
            self.vols.push(home);

            self.vols.push("/Applications".to_string());

            self.vols.len() as u8
        }
    }

    fn find_result(&mut self, filename: String, update_result: Vec<SearchResultItem>, if_increase: bool) {
        self.show_num += update_result.len();
        (self.find_result_callback)(filename, update_result, if_increase);
    }

    pub fn find(&mut self, filename: String, msg_reciever: &mpsc::Receiver<SearcherMessage>) -> Option<SearcherMessage> {
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
        
        if filename.is_empty() { return reply; } 

        self.waiting_finder = self.volume_packs.len() as u8;
        let (find_result_sender, find_result_receiver) = mpsc::channel::<Option<Vec<SearchResultItem>>>();
        for VolumePack{volume, ..} in &mut self.volume_packs {
            let find_result_sender: mpsc::Sender<Option<Vec<SearchResultItem>>> = find_result_sender.clone();
            let batch = self.batch;
            let volume = volume.clone();
            let filename = filename.clone();
            thread::spawn(move || {
                let mut volume = volume.lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                volume.find(filename, batch, find_result_sender);
            });
        }

        // begin loop
        loop {
            if let Ok(searcher_msg) = msg_reciever.try_recv() {
                for volume_pack in &mut self.volume_packs {
                    let _ = volume_pack.stop_sender.send(());
                }
                reply = Some(searcher_msg);
                break;
            }
            
            if let Ok( op_result ) = find_result_receiver.try_recv() {
                if let Some( mut result ) = op_result {
                    self.finding_result.items.append(&mut result);
                    self.waiting_finder -= 1;
                    if self.waiting_finder != 0 { continue; }

                    self.finding_result.items.sort_by(|a, b| b.rank.cmp(&a.rank)); // sort by rank
                    let return_result = 
                        if self.finding_result.items.len() > self.show_num { 
                            let max = std::cmp::min(self.finding_result.items.len(), need_num);
                            self.finding_result.items[self.show_num..max].to_vec()
                        }
                        else { vec![] };
                    self.find_result(filename, return_result, if_increase);
                }
                break;
            }
        }
        reply
    }

    pub fn init_volumes(&mut self) {
        self.volume_packs.clear();
        self.update_valid_vols();

        let handles = self.vols.iter().map(|c| {
            let (stop_sender, stop_receiver) = mpsc::channel::<()>();
            let volume = Arc::new(Mutex::new(Volume::new(c.chars().next().unwrap(), stop_receiver)));
            
            self.volume_packs.push(VolumePack { volume: volume.clone(), stop_sender });

            thread::spawn(move || {
                volume
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .build_index();
            })
        }).collect::<Vec<_>>();

        for handle in handles {
            if let Err(e) = handle.join() {
                log::error!("Init volume failed: {:?}", e);
            }
        }
    }

    pub fn update_index(&mut self) {
        self.update_valid_vols();

        let handles = self.volume_packs.iter().map(|VolumePack{volume, ..}| {
            let volume = volume.clone();
            thread::spawn(move || {
                volume
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .update_index();
            })
        }).collect::<Vec<_>>();

        for handle in handles {
            if let Err(e) = handle.join() {
                log::error!("Update index failed: {:?}", e);
            }
        }
    }

    pub fn release_index(&mut self) {
        self.update_valid_vols();
        
        self.finding_name = String::new();
        let handles = self.volume_packs.iter().map(|VolumePack{volume, ..}| {
            let volume = volume.clone();
            thread::spawn(move || {
                volume
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .release_index();
            })
        }).collect::<Vec<_>>();

        for handle in handles {
            if let Err(e) = handle.join() {
                log::error!("Release index failed: {:?}", e);
            }
        }
    }
}