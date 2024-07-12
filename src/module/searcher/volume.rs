use std::{
    collections::{BTreeMap, HashMap}, env, error::Error, ffi::{c_void, CString}, fs, io::{self, Write}, sync::mpsc, time::SystemTime
};

use windows_sys::Win32::{
    Storage::FileSystem,
    System::IO,
    System::Ioctl,
    Foundation,
};

use crate::core::util::log_util::{log_error, log_info};

struct File {
    parent_index: u64,
    file_name: String,
    filter: u32,
    rank: i8,
}

pub struct SearchResultItem {
    pub path: String,
    pub file_name: String,
    pub rank: i8,
}

impl Clone for SearchResultItem {
    fn clone(&self) -> Self {
        SearchResultItem {
            path: self.path.clone(),
            file_name: self.file_name.clone(),
            rank: self.rank,
        }
    }
}

pub struct SearchResult {
    pub items: Vec<SearchResultItem>,
    pub query: String,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct FileKey {
    rank: i8,
    index: u64,
}

struct FileMap {
    main_map: BTreeMap<FileKey, File>,
    rank_map: HashMap<u64, i8>,
}

impl FileMap {
    pub fn new() -> FileMap{
        FileMap {
            main_map: BTreeMap::new(),
            rank_map: HashMap::new(),
        }
    }

    // insert a file to the database by index, file name and parent index
    pub fn insert(&mut self, index: u64, file_name: String, parent_index: u64) {
        let filter = Volume::make_filter(&file_name);
        let rank = Self::get_file_rank(&file_name);
        self.insert_simple(index, File {
            parent_index,
            file_name,
            filter,
            rank,
        });
    }

    // insert a file to the database by index and file struct
    pub fn insert_simple(&mut self, index: u64, file: File) {
        let file_key = FileKey {
            rank: file.rank,
            index,
        };
        self.rank_map.insert(index, file.rank);
        self.main_map.insert(file_key, file);
    }

    // get a File by index
    pub fn get(&self, index: &u64) -> Option<&File> {
        if let Some(rank) = self.rank_map.get(index) {
            let file_key = FileKey {
                rank: *rank,
                index: *index,
            };
            return self.main_map.get(&file_key);
        }
        None
    }

    pub fn remove(&mut self, index: &u64) {
        if self.rank_map.contains_key(index) {
            let file_key = FileKey {
                rank: self.rank_map[index],
                index: *index,
            };
            self.main_map.remove(&file_key);
            self.rank_map.remove(index);
        }
    }

    pub fn iter(&self) -> std::collections::btree_map::Iter<'_, FileKey, File> {
        self.main_map.iter()
    }

    pub fn clear(&mut self) {
        self.main_map.clear();
        self.rank_map.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.main_map.is_empty()
    }

    // return rank by filename
    fn get_file_rank(file_name: &String) -> i8 {
        let mut rank: i8 = 0;

        if file_name.to_lowercase().ends_with(".exe") { rank += 10; }
        else if file_name.to_lowercase().ends_with(".lnk") { rank += 25; }

        let tmp = 40i16 - file_name.len() as i16;
        if tmp > 0 { rank += tmp as i8; }

        rank
    }

    // Constructs a path for a directory
    fn get_path(&self, index: &u64) -> Option<String> {
        let mut path = String::new();
        let mut loop_index = *index;
        while loop_index != 0 {
            let file_op = self.get(&loop_index);
            if let Some(file) = file_op {
                path.insert_str(0, (file.file_name.clone() + "\\").as_str());
                loop_index = file.parent_index;
            } else {
                return None;
            }
        }
        Some(path)
    }
}

pub struct Volume {
    pub drive: char,
    drive_frn: u64,
    ujd: Ioctl::USN_JOURNAL_DATA_V0,
    start_usn: i64,
    file_map: FileMap,
    stop_receiver: mpsc::Receiver<()>,
    last_query: String,
    last_search_num: usize,
}

impl Volume {
    pub fn new(drive: char, stop_receiver: mpsc::Receiver<()>) -> Volume {
        Volume {
            drive,
            drive_frn: 0x5000000000005,
            file_map: FileMap::new(),
            start_usn: 0x0,
            ujd: Ioctl::USN_JOURNAL_DATA_V0{ UsnJournalID: 0x0, FirstUsn: 0x0, NextUsn: 0x0, LowestValidUsn: 0x0, MaxUsn: 0x0, MaximumSize: 0x0, AllocationDelta: 0x0 },
            stop_receiver,
            last_query: String::new(),
            last_search_num: 0,
        }
    }

    // This is a helper function that opens a handle to the volume specified by the cDriveLetter parameter.
    fn open_drive(drive_letter: char) -> isize {
        unsafe{
            let c_str: CString = CString::new(format!("\\\\.\\{}:", drive_letter)).unwrap();
            FileSystem::CreateFileA(
                c_str.as_ptr() as *const u8, 
                Foundation::GENERIC_READ,
                FileSystem::FILE_SHARE_READ | FileSystem::FILE_SHARE_WRITE, 
                std::ptr::null::<windows_sys::Win32::Security::SECURITY_ATTRIBUTES>(), 
                FileSystem::OPEN_EXISTING, 
                0, 
                0)
        }
    }

    // This is a helper function that close a handle.
    fn close_drive(h_vol: isize) {
        unsafe { Foundation::CloseHandle(h_vol); }
    }

    // Calculates a 32bit value that is used to filter out many files before comparing their filenames
    fn make_filter(str: &String) -> u32 {
        /*
        Creates an address that is used to filter out strings that don't contain the queried characters
        Explanation of the meaning of the single bits:
        0-25 a-z
        26 0-9
        27 other ASCII
        28 not in ASCII
        */
        let len = str.len();
        if len == 0 { return 0;}
        let mut address: u32 = 0;
        let str_lower = str.to_lowercase();

        for c in str_lower.chars() {
            if ('a'..='z').contains(&c) {
                address |= 1 << (c as u32 - 97);
            } else if ('0'..'9').contains(&c) {
                address |= 1 << 26;
            } else if c > 127 as char {
                address |= 1 << 27;
            } else {
                address |= 1 << 28;
            }
        }
        address
    }

    // Enumerate the MFT for all entries. Store the file reference numbers of any directories in the database.
    pub fn build_index(&mut self, sender: Option<mpsc::Sender<bool>>) {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();
        #[cfg(debug_assertions)]
        log_info(format!("{} Begin Volume::build_index", self.drive));

        self.release_index();

        let h_vol = Self::open_drive(self.drive);

        // Query, Return statistics about the journal on the current volume
        let mut cd: u32 = 0;
        unsafe { 
            IO::DeviceIoControl(
                h_vol, 
                Ioctl::FSCTL_QUERY_USN_JOURNAL, 
                std::ptr::null(), 
                0, 
                &mut self.ujd as *mut Ioctl::USN_JOURNAL_DATA_V0 as *mut c_void, 
                std::mem::size_of::<Ioctl::USN_JOURNAL_DATA_V0>().try_into().unwrap(), 
                &mut cd, 
                std::ptr::null::<IO::OVERLAPPED>() as *mut IO::OVERLAPPED
            )
        };

        self.start_usn = self.ujd.NextUsn;

        // add the root directory
        let sz_root = format!("{}:", self.drive);
        self.file_map.insert(self.drive_frn, sz_root, 0);

        let mut med: Ioctl::MFT_ENUM_DATA_V0 = Ioctl::MFT_ENUM_DATA_V0 {
            StartFileReferenceNumber: 0,
            LowUsn: 0,
            HighUsn: self.ujd.NextUsn,
        };
        let mut data: [u64; 0x10000] = [0; 0x10000];
        let mut cb: u32 = 0;
        
        unsafe{
            while IO::DeviceIoControl(
                h_vol, 
                Ioctl::FSCTL_ENUM_USN_DATA, 
                &med as *const _ as *const c_void, 
                std::mem::size_of::<Ioctl::MFT_ENUM_DATA_V0>() as u32, 
                &mut data as *mut _ as *mut c_void, 
                std::mem::size_of::<[u8; std::mem::size_of::<u64>() * 0x10000]>() as u32, 
                &mut cb as *mut u32, 
                std::ptr::null_mut::<IO::OVERLAPPED>()
            ) != 0 {
                let mut record_ptr: *const Ioctl::USN_RECORD_V2 = &(data[1]) as *const u64 as *const Ioctl::USN_RECORD_V2;
                while (record_ptr as usize) < (&(data[0]) as *const u64 as usize + cb as usize) {
                    let file_name_begin_ptr = record_ptr as usize + (*record_ptr).FileNameOffset as usize;
                    let file_name_length = (*record_ptr).FileNameLength / (std::mem::size_of::<u16>() as u16);
                    let mut file_name_list: Vec<u16> = Vec::new();
                    for i in 0..file_name_length {
                        let c = *((file_name_begin_ptr + (i * 2) as usize) as *const u16);
                        file_name_list.push(c);
                    }
                    let file_name = String::from_utf16(&file_name_list).unwrap_or(String::from("unknown"));
                    self.file_map.insert((*record_ptr).FileReferenceNumber, file_name, (*record_ptr).ParentFileReferenceNumber);
                    record_ptr = (record_ptr as usize + (*record_ptr).RecordLength as usize) as *mut Ioctl::USN_RECORD_V2;
                }
                med.StartFileReferenceNumber = *(&(data[0]) as *const u64);
            }
        }

        #[cfg(debug_assertions)]
        log_info(format!("{} End Volume::build_index, use time: {:?} ms", self.drive, sys_time.elapsed().unwrap().as_millis()));
        
        Self::close_drive(h_vol);
        self.serialization_write().unwrap_or_else(|err: io::Error| {
            log_error(format!("{} Volume::serialization_write, error: {:?}", self.drive, err));
        });
        
        if let Some(sender) = sender {
            let _ = sender.send(true); // TODO return false when failed
        }
    }

    // Clears the database
    pub fn release_index(&mut self) {
        if self.file_map.is_empty() {return;}

        self.last_query = String::new();
        self.last_search_num = 0;

        #[cfg(debug_assertions)]
        log_info(format!("{} Begin Volume::release_index", self.drive));

        self.file_map.clear();
    }

    // return true if contain query
    fn match_str(contain: &str, query_lower: &String) -> bool {
        if contain.to_lowercase().contains(query_lower) {
            return true;
        }
        false
    }

    // searching
    pub fn find(&mut self, query: String, batch: u8, sender: mpsc::Sender<Option<Vec<SearchResultItem>>>) {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();
        #[cfg(debug_assertions)]
        log_info(format!("{} Begin Volume::Find {query}", self.drive));

        let mut result = Vec::new();

        if query.is_empty() { let _ = sender.send(None); return;}
        if self.file_map.is_empty() { 
            self.serialization_read().unwrap_or_else(|err: Box<dyn Error>| {
                log_error(format!("{} Volume::serialization_write, error: {:?}", self.drive, err));
                self.build_index(None);
            });
        };

        let query_lower = query.to_lowercase();
        let query_filter = Self::make_filter(&query_lower);
        
        // clear channel before find !!! TODO need to use a better way
        while self.stop_receiver.try_recv().is_ok() { }
        
        if self.last_query != query {
            self.last_search_num = 0;
            self.last_query = query.clone();
        }
        let file_map_iter = self.file_map.iter().rev().skip(self.last_search_num);

        let mut find_num = 0;
        let mut search_num: usize = 0;
        for (_, file) in file_map_iter {
            if self.stop_receiver.try_recv().is_ok() {
                #[cfg(debug_assertions)]
                log_info(format!("{} Stop Volume::Find", self.drive));
                let _ = sender.send(None);
                return;
            }
            search_num += 1;
            if (file.filter & query_filter) == query_filter {
                let file_name = file.file_name.clone();
                if Self::match_str(&file_name, &query_lower) {
                    if let Some(path) = self.file_map.get_path(&file.parent_index){
                        result.push(SearchResultItem {
                            path,
                            file_name,
                            rank: file.rank,
                        });
                        find_num += 1;
                        if find_num >= batch { break;}
                    }
                }
            }
        }

        #[cfg(debug_assertions)]
        log_info(format!("{} End Volume::Find {query}, use time: {:?} ms, get result num {}", self.drive, sys_time.elapsed().unwrap().as_millis(), result.len()));
        
        self.last_search_num += search_num;
        let _ = sender.send(Some(result));
    }

    // update index, add new file, remove deleted file
    pub fn update_index(&mut self) {
        #[cfg(debug_assertions)]
        log_info(format!("{} Begin Volume::update_index", self.drive));

        if self.file_map.is_empty() { 
            self.serialization_read().unwrap_or_else(|err: Box<dyn Error>| {
                log_error(format!("{} Volume::serialization_write, error: {:?}", self.drive, err));
                self.build_index(None);
            });
        };

        let mut data: [i64; 0x10000] = [0; 0x10000];
        let mut cb: u32 = 0;
        let mut rujd: Ioctl::READ_USN_JOURNAL_DATA_V0 = Ioctl::READ_USN_JOURNAL_DATA_V0 {
                StartUsn: self.start_usn,
                ReasonMask: Ioctl::USN_REASON_FILE_CREATE | Ioctl::USN_REASON_FILE_DELETE | Ioctl::USN_REASON_RENAME_NEW_NAME,
                ReturnOnlyOnClose: 0,
                Timeout: 0,
                BytesToWaitFor: 0,
                UsnJournalID: self.ujd.UsnJournalID,
        };

        let h_vol = Self::open_drive(self.drive);

        unsafe{
            while IO::DeviceIoControl(
                h_vol, 
                Ioctl::FSCTL_READ_USN_JOURNAL, 
                &rujd as *const _ as *const c_void,
                std::mem::size_of::<Ioctl::READ_USN_JOURNAL_DATA_V0>().try_into().unwrap(), 
                &mut data as *mut _ as *mut c_void,
                std::mem::size_of::<[u8; std::mem::size_of::<u64>() * 0x10000]>() as u32, 
                &mut cb as *mut u32, 
                std::ptr::null_mut::<IO::OVERLAPPED>()
            ) != 0 {
                if cb == 8 { break };
                let mut record_ptr: *const Ioctl::USN_RECORD_V2 = &(data[1]) as *const i64 as *const Ioctl::USN_RECORD_V2;
                while (record_ptr as usize) < (&(data[0]) as *const i64 as usize + cb as usize) {
                    let file_name_begin_ptr = record_ptr as usize + (*record_ptr).FileNameOffset as usize;
                    let file_name_length = (*record_ptr).FileNameLength / (std::mem::size_of::<u16>() as u16);
                    let mut file_name_list: Vec<u16> = Vec::new();
                    for i in 0..file_name_length {
                        let c = *((file_name_begin_ptr + (i * 2) as usize) as *const u16);
                        file_name_list.push(c);
                    }
                    let file_name = String::from_utf16(&file_name_list).unwrap_or(String::from("unknown"));
                    if (*record_ptr).Reason & Ioctl::USN_REASON_FILE_CREATE == Ioctl::USN_REASON_FILE_CREATE {
                        self.file_map.insert((*record_ptr).FileReferenceNumber, file_name, (*record_ptr).ParentFileReferenceNumber);
                    }
                    else if (*record_ptr).Reason & Ioctl::USN_REASON_FILE_DELETE == Ioctl::USN_REASON_FILE_DELETE {
                        self.file_map.remove(&(*record_ptr).FileReferenceNumber);
                    }
                    else if (*record_ptr).Reason & Ioctl::USN_REASON_RENAME_NEW_NAME == Ioctl::USN_REASON_RENAME_NEW_NAME {
                        self.file_map.insert((*record_ptr).FileReferenceNumber, file_name, (*record_ptr).ParentFileReferenceNumber);
                    }
                    record_ptr = (record_ptr as usize + (*record_ptr).RecordLength as usize) as *mut Ioctl::USN_RECORD_V2;
                }
                rujd.StartUsn = *(&(data[0]) as *const i64);
            }
        }
        self.start_usn = rujd.StartUsn;
        Self::close_drive(h_vol);
    }

    // serializate file_map to reduce memory usage
    fn serialization_write(&mut self) -> Result<(), io::Error> {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();
        #[cfg(debug_assertions)]
        log_info(format!("{} Begin Volume::serialization_write", self.drive));

        if self.file_map.is_empty() {return Ok(())};
        
        let file_path = env::current_exe().unwrap().parent().unwrap().join("userdata");
        if !file_path.exists() { fs::create_dir(&file_path)?; }

        let mut save_file = fs::File::create(format!("{}/{}.fd", file_path.to_str().unwrap(), self.drive))?;

        let mut buf = Vec::new();
        buf.write(&self.start_usn.to_be_bytes())?;
        for (file_key, file) in self.file_map.iter() {
            buf.write(&file_key.index.to_be_bytes())?;
            buf.write(&file.parent_index.to_be_bytes())?;
            buf.write(&(file.file_name.len() as u16).to_be_bytes())?;
            buf.write(file.file_name.as_bytes())?;
            buf.write(&file.filter.to_be_bytes())?;
            buf.write(&file.rank.to_be_bytes())?;
        }
        let _ = save_file.write(&buf.to_vec());
        self.release_index();

        #[cfg(debug_assertions)]
        log_info(format!("{} End Volume::serialization_write, use time: {:?} ms", self.drive, sys_time.elapsed().unwrap().as_millis()));

        Ok(())
    }

    // deserializate file_map from file
    fn serialization_read(&mut self) -> Result<(), Box<dyn Error>> {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();
        #[cfg(debug_assertions)]
        log_info(format!("{} Begin Volume::serialization_read", self.drive));
        
        let file_path = env::current_exe().unwrap().parent().unwrap().join("userdata");
        let file_path_str = file_path.to_str().unwrap();
        
        let file_data = fs::read(format!("{}/{}.fd", file_path_str, self.drive))?;

        if file_data.len() < 8 { return Err(io::Error::new(io::ErrorKind::InvalidData, "File data too short.").into()); }

        self.start_usn = i64::from_be_bytes(file_data[0..8].try_into()?);
        let mut ptr_index = 8;

        while ptr_index < file_data.len() {
            if ptr_index + 18 > file_data.len() { return Err(io::Error::new(io::ErrorKind::InvalidData, "File data size error.").into()); }
            
            let index = u64::from_be_bytes(file_data[ptr_index..ptr_index+8].try_into()?);
            ptr_index += 8;
            let parent_index = usize::from_be_bytes(file_data[ptr_index..ptr_index+8].try_into()?) as u64;
            ptr_index += 8;
            let file_name_len = u16::from_be_bytes(file_data[ptr_index..ptr_index+2].try_into()?) as u16;
            ptr_index += 2;

            if ptr_index + (file_name_len as usize) + 5 > file_data.len() { return Err(io::Error::new(io::ErrorKind::InvalidData, "File data size error.").into()); }

            let file_name = String::from_utf8(file_data[ptr_index..(ptr_index + file_name_len as usize)].to_vec())?;
            ptr_index += file_name_len as usize;
            let filter = u32::from_be_bytes(file_data[ptr_index..ptr_index+4].try_into()?);
            ptr_index += 4;
            let rank = i8::from_be_bytes(file_data[ptr_index..ptr_index+1].try_into()?);
            ptr_index += 1;
            self.file_map.insert_simple(index, File { parent_index, file_name, filter, rank });
        }

        #[cfg(debug_assertions)]
        log_info(format!("{} End Volume::serialization_read, use time: {:?} ms", self.drive, sys_time.elapsed().unwrap().as_millis()));

        Ok(())
    }
}