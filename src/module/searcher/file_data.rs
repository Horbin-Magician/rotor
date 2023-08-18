use std::ffi::CString;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use slint::{Model, VecModel};

use windows_sys::Win32::Storage::FileSystem;
use windows_sys::Win32::Foundation;

use super::volume;
use super::slint_generatedSearchWindow::SearchResult_slint;
use super::slint_generatedSearchWindow::SearchWindow;

struct volume_pack {
    volume: Arc<Mutex<volume::Volume>>,
    stop_sender: mpsc::Sender<()>,
}

pub struct FileData {
    vols: Vec<char>,
    // volumes: Vec<Arc<Mutex<volume::Volume>>>,
    finding_name: String,
    finding_result: volume::SearchResult,
    waiting_init: u8,
    waiting_finder: u8,
    // stop_senders: Vec<mpsc::Sender<()>>,
    search_win: slint::Weak<SearchWindow>,
    stop_finder_receiver: mpsc::Receiver<()>,
    volume_packs: Vec<volume_pack>,
}

impl FileData {
    pub fn new(search_win: slint::Weak<SearchWindow>, stop_finder_receiver: mpsc::Receiver<()>) -> FileData {
        FileData {
            vols: Vec::new(),
            volume_packs: Vec::new(),
            // volumes: Vec::new(),
            finding_name: String::new(),
            finding_result: volume::SearchResult{items: Vec::new(), query: String::new()},
            waiting_finder: 0,
            waiting_init: 0,
            // stop_senders: vec![],
            search_win,
            stop_finder_receiver,
        }
    }

    // Check whether the disk represented by a drive letter is in ntfs format
    fn is_ntfs(vol: char) -> bool {
        let root_path_name = CString::new(format!("{}:\\", vol)).unwrap();
        let mut volume_name_buffer: [u8; Foundation::MAX_PATH as usize] = [0; Foundation::MAX_PATH as usize];
        let mut volume_serial_number: u32 = 0;
        let mut maximum_component_length: u32 = 0;
        let mut file_system_flags: u32 = 0;
        let mut file_system_name_buffer:[u8; Foundation::MAX_PATH as usize] = [0; Foundation::MAX_PATH as usize];

        unsafe {
            if FileSystem::GetVolumeInformationA(
                    root_path_name.as_ptr() as *const u8,
                    &mut (volume_name_buffer[0]) as *mut _ as *mut u8,
                    Foundation::MAX_PATH,
                    &mut volume_serial_number,
                    &mut maximum_component_length,
                    &mut file_system_flags,
                    &mut (file_system_name_buffer[0]) as *mut _ as *mut u8,
                    Foundation::MAX_PATH
                ) != 0 {
                let result = String::from_utf8_lossy(&file_system_name_buffer);
                let mut result = String::from(result);
                result.retain(|c| c!='\0');
                return result == "NTFS";
            }
        }
        false
    }

    fn init_valid_vols(&mut self) -> u8 {
        let mut bit_mask = unsafe { FileSystem::GetLogicalDrives() };
        self.vols.clear();
        let mut vol = 'a';
        while bit_mask != 0 {
            if bit_mask & 0x1 != 0 {
                if Self::is_ntfs(vol) { 
                    self.vols.push(vol);
                    println!("{vol}");
                }
            }
            vol = (vol as u8 + 1) as char;
            bit_mask >>= 1;
        }
        self.vols.len() as u8
    }

    pub fn init_volumes(&mut self) -> bool {
        self.waiting_init = self.init_valid_vols();

        let (build_sender, build_receiver) = mpsc::channel::<bool>();

        for c in &self.vols {
            let (stop_sender, stop_receiver) = mpsc::channel::<()>();
            let volume = Arc::new(Mutex::new(volume::Volume::new(c.clone(), stop_receiver)));
            
            self.volume_packs.push(volume_pack {
                volume: volume.clone(),
                stop_sender,
            });

            let build_sender_clone = build_sender.clone();
            let _ = thread::spawn(move || {
                volume.lock().unwrap().build_index_with_sender(build_sender_clone);
            });
        }

        loop{
            let result = build_receiver.recv().unwrap();
            if result {
                self.waiting_init -= 1;
                if self.waiting_init == 0 { return true; }
            }
            else { return false; }
        }
    }

    fn stop_find(&self) {
        for volume_pack in &self.volume_packs {
            match volume_pack.volume.try_lock() {
                Ok( _ ) => {},
                Err( _ ) => { let _ = volume_pack.stop_sender.send(()); },
            }
        }
    }

    pub fn find(&mut self, filename: String) {
        println!("Begin FileData::find_file Filename: {filename}");

        if filename == "" {
            self.finding_name = String::from("");
            return;
        } else if self.finding_name == filename { return; }

        self.finding_name = filename.clone();

        self.waiting_finder = self.volume_packs.len() as u8;

        self.finding_result.items.clear();
        self.finding_result.query = filename.clone();

        let (find_result_sender, find_result_receiver) = mpsc::channel::<Vec<volume::SearchResultItem>>();

        for volume_pack{volume, ..} in &self.volume_packs { 
            let find_result_sender: mpsc::Sender<Vec<volume::SearchResultItem>> = find_result_sender.clone();
            let volume = volume.clone();
            let filename = filename.clone();
            let _ = thread::spawn(move || {
                let mut volume = volume.lock().unwrap();
                volume.find(filename.clone(), find_result_sender);
            });
        }

        loop {
            match self.stop_finder_receiver.try_recv() {
                Ok( _ ) => { self.stop_find(); },
                Err( _ ) => {},
            }
            
            match find_result_receiver.try_recv() {
                Ok( mut result ) => {
                    // TODO 检查接收到结果的原因是搜索中断还是搜索完成，若中断，则不进行下面的步骤
                    self.finding_result.items.append(&mut result);
                    self.waiting_finder -= 1;
                    if self.waiting_finder == 0 {
                        // TODO 排序
                        // sort(m_findingResult->begin(), m_findingResult->end());
                        println!("End FileData::find_file");
        
                        let return_result;
                        if self.finding_result.items.len() > 20 { return_result = self.finding_result.items[..20].to_vec(); }
                        else { return_result = self.finding_result.items.to_vec(); }
                        
                        self.search_win.clone().upgrade_in_event_loop(move |search_win| {
                            // TODO 确认此时的结果为有效结果
                            let search_result_model = search_win.get_search_result();
                            let search_result_model = search_result_model.as_any().downcast_ref::<VecModel<SearchResult_slint>>()
                                .expect("search_result_model set a VecModel earlier");
                            
                            let mut result_list = Vec::new();
                            let mut i = 0;
                            for item in return_result {
                                result_list.push(
                                    SearchResult_slint { 
                                        id: i,
                                        filename: slint::SharedString::from(item.file_name.clone()),
                                        path: slint::SharedString::from(item.path.clone()),
                                    }
                                );
                                i += 1;
                            }
                            search_result_model.set_vec(result_list);
                        }).unwrap();
                        break;
                    }
                },
                Err( _ ) => {},
            }
        }
    }

    pub fn update_index(&self) {
        for volume_pack{volume, ..} in &self.volume_packs { 
            let volume = volume.clone();
            let _ = thread::spawn(move || {
                volume.lock().unwrap().update_index();
            });
        }
    }

    pub fn release_index(&self) {
        for volume_pack{volume, ..} in &self.volume_packs { 
            let volume = volume.clone();
            let _ = thread::spawn(move || {
                volume.lock().unwrap().release_index();
            });
        }
    }
}