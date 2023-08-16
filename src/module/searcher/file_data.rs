use std::ffi::CString;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::rc::Rc;

use windows_sys::Win32::Storage::FileSystem;
use windows_sys::Win32::Foundation;

use crate::module::searcher::volume;
use super::volume::Volume;

pub struct FileData {
    vols: Vec<char>,
    volumes: Vec<Arc<Mutex<volume::Volume>>>,
    finding_name: String,
    finding_result: volume::SearchResult,
    waiting_init: u8,
    waiting_finder: u8,
    stop_senders: Vec<mpsc::Sender<()>>,
}

impl FileData {
    pub fn new() -> FileData {
        FileData {
            vols: Vec::new(),
            volumes: Vec::new(),
            finding_name: String::new(),
            finding_result: volume::SearchResult{items: Vec::new(), query: String::new()},
            waiting_finder: 0,
            waiting_init: 0,
            stop_senders: vec![],
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

        let (sender, receiver) = mpsc::channel::<bool>();

        for c in &self.vols {
            let (stop_sender, stop_receiver) = mpsc::channel::<()>();
            let volume = Arc::new(Mutex::new(Volume::new(c.clone(), stop_receiver)));
            self.stop_senders.push(stop_sender);
            self.volumes.push(volume.clone());

            let sender_clone = sender.clone();
            let _ = thread::spawn(move || {
                volume.lock().unwrap().build_index_with_sender(sender_clone);
            });
        }

        loop{
            let result = receiver.recv().unwrap();
            if result {
                self.waiting_init -= 1;
                if self.waiting_init == 0 { return true; }
            }
            else { return false; }
        }
    }

    pub fn find(&mut self, filename: String) -> &volume::SearchResult {
        println!("Begin FileData::find_file Filename: {filename}");

        if self.finding_name == filename { return &self.finding_result; }
        self.finding_name = filename.clone();

        for sender in &self.stop_senders { let _ = sender.send(()); }
        self.waiting_finder = self.volumes.len() as u8;

        self.finding_result.items.clear();
        self.finding_result.query = filename.clone();

        let (find_result_sender, find_result_receiver) = mpsc::channel::<Vec<volume::SearchResultItem>>();

        for volume in &mut self.volumes {
            let find_result_sender: mpsc::Sender<Vec<volume::SearchResultItem>> = find_result_sender.clone();
            let volume = volume.clone();
            let filename = filename.clone();
            let _ = thread::spawn(move || {
                let mut volume = volume.lock().unwrap();
                volume.find(filename.clone(), find_result_sender);
            });
        }

        loop{
            // TODO 出现错误时终止循环
            // if(result == nullptr || filename != this->m_findingName) return;
            let mut result = find_result_receiver.recv().unwrap();
            self.finding_result.items.append(&mut result);
            self.waiting_finder -= 1;
            if self.waiting_finder == 0 {
                // TODO 排序
                // sort(m_findingResult->begin(), m_findingResult->end());
                println!("End FileData::find_file");
                return &self.finding_result;
            }
        }
    }

    pub fn update_index(&self) {
        for volume in &self.volumes { 
            let volume = volume.clone();
            let _ = thread::spawn(move || {
                volume.lock().unwrap().update_index();
            });
        }
    }

    pub fn release_index(&self) {
        for volume in &self.volumes { 
            let volume = volume.clone();
            let _ = thread::spawn(move || {
                volume.lock().unwrap().release_index();
            });
        }
    }
}