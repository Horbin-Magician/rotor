use std::ffi::{CStr, CString};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::collections::VecDeque;
use windows::Win32::Storage::FileSystem;
use windows::Win32::Foundation;
use slint::{Model, VecModel};

use crate::util::{file_util, log_util};
use crate::ui::SearchResult_slint;
use super::{SearchWindow, SearcherMessage};
use super::volume::{Volume, SearchResultItem};


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
    vols: Vec<char>,
    finding_name: String,
    finding_result: SearchResult,
    waiting_finder: u8,
    search_win: slint::Weak<SearchWindow>,
    volume_packs: Vec<VolumePack>,
    state: FileState,
    show_num: usize,
    batch: u8,
}

impl FileData {
    pub fn new(search_win: slint::Weak<SearchWindow>) -> FileData {
        FileData {
            vols: Vec::new(),
            volume_packs: Vec::new(),
            finding_name: String::new(),
            finding_result: SearchResult{items: Vec::new(), query: String::new()},
            waiting_finder: 0,
            search_win,
            state: FileState::Unbuild,
            show_num: 20,
            batch: 20,
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

    // Check whether the disk represented by a drive letter is in ntfs format
    fn is_ntfs(vol: char) -> bool {
        if let Ok(root_path_name) = CString::new(format!("{}:\\", vol)) {
            let mut volume_name_buffer = vec![0u8; Foundation::MAX_PATH as usize];
            let mut volume_serial_number: u32 = 0;
            let mut maximum_component_length: u32 = 0;
            let mut file_system_flags: u32 = 0;
            let mut file_system_name_buffer = vec![0u8; Foundation::MAX_PATH as usize];
    
            unsafe {
                if FileSystem::GetVolumeInformationA(
                        windows::core::PCSTR(root_path_name.as_ptr() as *const u8),
                        Some(&mut volume_name_buffer),
                        Some(&mut volume_serial_number),
                        Some(&mut maximum_component_length),
                        Some(&mut file_system_flags),
                        Some(&mut file_system_name_buffer),
                    ).is_ok() {
    
                    let result = CStr::from_ptr(file_system_name_buffer.as_ptr() as *const i8);
                    return result.to_string_lossy() == "NTFS";
                }
            }
        }
        false
    }

    fn update_valid_vols(&mut self) -> u8 {
        let mut bit_mask = unsafe { FileSystem::GetLogicalDrives() };
        self.vols.clear();
        let mut vol = 'A';
        while bit_mask != 0 {
            if bit_mask & 0x1 != 0 && Self::is_ntfs(vol) { self.vols.push(vol); }
            vol = (vol as u8 + 1) as char;
            bit_mask >>= 1;
        }

        self.volume_packs.retain(|volume_pack| {
            if let Ok(volume)  = volume_pack.volume.lock() {
                return self.vols.contains(&volume.drive);
            }
            false
        });

        self.vols.len() as u8
    }

    fn update_result_model(&mut self, filename: String, update_result: Vec<SearchResultItem>, increment_find: bool) {
        self.search_win.clone().upgrade_in_event_loop(move |search_win| {
            if search_win.get_query() != filename {return;}

            let mut result_list = Vec::new();
            for (id, item) in update_result.into_iter().enumerate() {
                let icon = 
                    file_util::get_icon((item.path.clone() + item.file_name.as_str()).as_str())
                        .unwrap_or_else(|| slint::Image::default());
                result_list.push(
                    SearchResult_slint { 
                        id: id as i32,
                        icon,
                        filename: slint::SharedString::from(item.file_name.clone()),
                        path: slint::SharedString::from(item.path.clone()),
                    }
                );
            }

            if let Some(search_result_model) = search_win.get_search_result().as_any().downcast_ref::<VecModel<SearchResult_slint>>() {
                search_result_model.set_vec(result_list);
                if !increment_find {
                    search_win.set_viewport_y(0.);
                    search_win.set_active_id(0);
                }
            }
        }).unwrap_or_else(|e| log_util::log_error(format!("update_result_model: {}", e)));
    }

    pub fn find(&mut self, filename: String, msg_reciever: &mpsc::Receiver<SearcherMessage>) -> Option<SearcherMessage> {
        let mut reply: Option<SearcherMessage> = None;
        let mut increment_find = false;

        if self.finding_name == filename { 
            increment_find = true;
            self.show_num += self.batch as usize;

            if self.finding_result.items.len() > self.show_num {
                let return_result = self.finding_result.items[..self.show_num].to_vec();
                self.update_result_model(filename, return_result, increment_find);
                return reply;
            }
        } else { 
            self.finding_name = filename.clone();
            self.show_num = self.batch as usize;
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
                        if self.finding_result.items.len() > self.show_num { self.finding_result.items[..self.show_num].to_vec() }
                        else { self.finding_result.items.to_vec() };
                    self.update_result_model(filename, return_result, increment_find);
                }
                break;
            }
        }
        reply
    }

    pub fn init_volumes(&mut self) {
        self.volume_packs.clear();
        self.update_valid_vols();

        let handles = self.vols.iter().map(|&c| {
            let (stop_sender, stop_receiver) = mpsc::channel::<()>();
            let volume = Arc::new(Mutex::new(Volume::new(c, stop_receiver)));
            
            self.volume_packs.push(VolumePack { volume: volume.clone(), stop_sender });

            thread::spawn(move || {
                if let Ok(mut volume) = volume.lock() {
                    volume.build_index();
                }
            })
        }).collect::<Vec<_>>();

        for handle in handles {
            if let Err(e) = handle.join() {
                eprintln!("Thread panicked: {:?}", e); // TODO handle error
            }
        }
    }

    pub fn update_index(&mut self) {
        self.update_valid_vols();

        let handles = self.volume_packs.iter().map(|VolumePack{volume, ..}| {
            let volume = volume.clone();
            thread::spawn(move || {
                if let Ok(mut volume) = volume.lock() {
                    volume.update_index();
                }
            })
        }).collect::<Vec<_>>();

        for handle in handles {
            if let Err(e) = handle.join() {
                eprintln!("Thread panicked: {:?}", e); // TODO handle error
            }
        }
    }

    pub fn release_index(&mut self) {
        self.update_valid_vols();
        
        self.finding_name = String::new();
        let handles = self.volume_packs.iter().map(|VolumePack{volume, ..}| {
            let volume = volume.clone();
            thread::spawn(move || {
                if let Ok(mut volume) = volume.lock() {
                    volume.release_index();
                }
            })
        }).collect::<Vec<_>>();

        for handle in handles {
            if let Err(e) = handle.join() {
                eprintln!("Thread panicked: {:?}", e); // TODO handle error
            }
        }
    }
}