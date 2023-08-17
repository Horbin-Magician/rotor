use std::ffi::{CString, c_void};
use std::time::SystemTime;
use std::sync::mpsc;

use windows_sys::Win32::{
    Storage::FileSystem,
    System::IO,
    System::Ioctl,
    Foundation,
};

struct File {
    parent_index: u64,
    file_name: String,
    filter: u32,
    rank: i8,
}

#[derive(Debug)]
pub struct SearchResultItem {
    pub path: String,
    pub file_name: String,
    rank: i8,
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

type FileMap = std::collections::HashMap<u64, File>;

pub struct Volume {
    state: u8,
    drive: char,
    drive_frn: u64,
    file_map: FileMap,
    start_usn: u64,
    ujd: Ioctl::USN_JOURNAL_DATA_V0,
    h_vol: isize,
    stop_receiver: mpsc::Receiver<()>,
}

impl Volume {
    pub fn new(drive: char, stop_receiver: mpsc::Receiver<()>) -> Volume {
        let h_vol = Self::open(drive);
        Volume {
            state: 0,
            drive,
            drive_frn: 0x5000000000005,
            file_map: FileMap::new(),
            start_usn: 0x0,
            ujd: Ioctl::USN_JOURNAL_DATA_V0{ UsnJournalID: 0x0, FirstUsn: 0x0, NextUsn: 0x0, LowestValidUsn: 0x0, MaxUsn: 0x0, MaximumSize: 0x0, AllocationDelta: 0x0 },
            h_vol,
            stop_receiver,
        }
    }

    // This is a helper function that opens a handle to the volume specified by the cDriveLetter parameter.
    fn open(drive_letter: char) -> isize {
        unsafe{
            let c_str: CString = CString::new(format!("\\\\.\\{}:", drive_letter)).unwrap();
            FileSystem::CreateFileA(
                c_str.as_ptr() as *const u8, 
                Foundation::GENERIC_READ,
                FileSystem::FILE_SHARE_READ | FileSystem::FILE_SHARE_WRITE, 
                0 as *const windows_sys::Win32::Security::SECURITY_ATTRIBUTES, 
                FileSystem::OPEN_EXISTING, 
                0, 
                0)
        }
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
        if len <= 0 { return 0;}
        let mut address: u32 = 0;
        let str_lower = str.to_lowercase();

        for c in str_lower.chars() {
            if c >= 'a' && c <= 'z' {
                address |= 1 << (c as u32 - 97);
            } else if c >= '0' && c <= '9' {
                address |= 1 << 26;
            } else if c > 127 as char {
                address |= 1 << 27;
            } else {
                address |= 1 << 28;
            }
        }
        address
    }

    // return rank by filename
    fn get_file_rank(file_name: &String) -> i8 {
        let mut rank: i8 = 0;
        if file_name.to_lowercase().ends_with(".exe") { rank += 10; }
        else if file_name.to_lowercase().ends_with(".lnk") { rank += 30; }
        rank
    }

    // Adds a file to the database
    fn add_file(&mut self, index: u64, file_name: String, parent_index: u64) {
        let filter = Self::make_filter(&file_name);
        let rank = Self::get_file_rank(&file_name);
        self.file_map.insert(index, File {
            parent_index,
            file_name,
            filter,
            rank,
        });
    }

    // Enumerate the MFT for all entries. Store the file reference numbers of any directories in the database.
    pub fn build_index(&mut self) {
        let sys_time = SystemTime::now();
        println!("[info] {} Begin Volume::build_index", self.drive);

        self.release_index();

        // Query, Return statistics about the journal on the current volume
        let mut cd: u32 = 0;
        unsafe { 
            IO::DeviceIoControl(
                self.h_vol, 
                Ioctl::FSCTL_QUERY_USN_JOURNAL, 
                std::ptr::null(), 
                0, 
                &mut self.ujd as *mut Ioctl::USN_JOURNAL_DATA_V0 as *mut c_void, 
                std::mem::size_of::<Ioctl::USN_JOURNAL_DATA_V0>().try_into().unwrap(), 
                &mut cd, 
                std::ptr::null::<IO::OVERLAPPED>() as *mut IO::OVERLAPPED
            )
        };

        self.start_usn = self.ujd.NextUsn as u64;

        // add the root directory
        let sz_root = format!("{}:", self.drive);
        self.add_file(self.drive_frn, sz_root, 0);

        let mut med: Ioctl::MFT_ENUM_DATA_V0 = Ioctl::MFT_ENUM_DATA_V0 {
            StartFileReferenceNumber: 0,
            LowUsn: 0,
            HighUsn: self.ujd.NextUsn,
        };
        let mut data: [u64; 0x10000] = [0; 0x10000];
        let mut cb: u32 = 0;
        
        unsafe{
            while IO::DeviceIoControl(
                self.h_vol, 
                Ioctl::FSCTL_ENUM_USN_DATA, 
                &med as *const _ as *const c_void, 
                std::mem::size_of::<Ioctl::MFT_ENUM_DATA_V0>() as u32, 
                &mut data as *mut _ as *mut c_void, 
                std::mem::size_of::<[u8; std::mem::size_of::<u64>() * 0x10000]>() as u32, 
                &mut cb as *mut u32, 
                0 as *mut IO::OVERLAPPED
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
                    let file_name = String::from_utf16(&file_name_list).unwrap();
                    self.add_file((*record_ptr).FileReferenceNumber, file_name, (*record_ptr).ParentFileReferenceNumber);
                    record_ptr = (record_ptr as usize + (*record_ptr).RecordLength as usize) as *mut Ioctl::USN_RECORD_V2;
                }
                med.StartFileReferenceNumber = *(&(data[0]) as *const u64);
            }
        }
        println!("[info] {} End Volume::build_index, use tiem: {:?} ms", self.drive, sys_time.elapsed().unwrap().as_millis());
    }


    pub fn build_index_with_sender(&mut self, sender: mpsc::Sender<bool>) {
        self.build_index();
        sender.send(true).unwrap();
    }

    // Clears the database
    pub fn release_index(&mut self) {
        self.file_map.clear();
    }

    fn match_str(contain: &String, query: &String) -> i8 {
        let mut i = 0;
        let query_list = query.to_lowercase().chars().collect::<Vec<char>>();
        for c in contain.to_lowercase().chars() {
            if query_list[i] == c { i += 1; }
            if i >= query_list.len() {
                let rank: i8 = 10i8.wrapping_sub((contain.len() as i8).wrapping_sub(query_list.len() as i8));
                return if rank < 0 { 0 } else { rank };
            }
        }
        -1
    }

    // Constructs a path for a directory
    fn get_path(&self, index: &u64) -> Option<String> {
        let mut path = String::new();
        let mut loop_index = index.clone();
        while loop_index != 0 {
            if self.file_map.contains_key(&loop_index) == false { return None;}
            let file = &self.file_map[&loop_index];
            path.insert_str(0, (file.file_name.clone() + "\\").as_str());
            loop_index = file.parent_index;
        }
        Some(path)
    }

    // searching
    pub fn find(&mut self, query: String, sender: mpsc::Sender<Vec<SearchResultItem>>) {
        let sys_time = SystemTime::now();
        println!("[info] {} Begin Volume::Find", self.drive);

        let mut result = Vec::new();

        if query.len() == 0 { sender.send(result).unwrap(); return; }
        if self.file_map.is_empty() { self.serialization_read() };

        let query_lower = query.to_lowercase();
        let query_filter = Self::make_filter(&query_lower);

        for (index, file) in self.file_map.iter() {
            match self.stop_receiver.try_recv() {
                Ok(_) => { sender.send(Vec::new()).unwrap(); return; },
                Err(_) => {},
            }
                
            if (file.filter & query_filter) == query_filter {
                let file_name = file.file_name.clone();
                let rank = Self::match_str(&file_name, &query_lower);
                if rank >= 0 {
                    if let Some(path) = self.get_path(index){
                        let item: SearchResultItem = SearchResultItem {
                            path,
                            file_name,
                            rank: file.rank + rank,
                        };
                        result.push(item);
                    }
                }
            }
        }
        println!("[info] {} End Volume::find, use tiem: {:?} ms", self.drive, sys_time.elapsed().unwrap().as_millis());
        sender.send(result).unwrap();
    }

    // TODO
    pub fn update_index(&self) {
        println!("TODO Volume::update_index");
        if self.file_map.is_empty() {self.serialization_read()};

        // WCHAR szRoot[_MAX_PATH];
        // wsprintf(szRoot, TEXT("%c:"), m_drive);

        // BYTE pData[sizeof(DWORDLONG) * 0x10000];
        // DWORD cb;
        // DWORD reason_mask = USN_REASON_FILE_CREATE | USN_REASON_FILE_DELETE | USN_REASON_RENAME_NEW_NAME;
        // READ_USN_JOURNAL_DATA rujd = {m_StartUSN, reason_mask, 0, 0, 0, m_ujd.UsnJournalID};

        // m_FileMapMutex.lock();
        // while (DeviceIoControl(m_hVol, FSCTL_READ_USN_JOURNAL, &rujd, sizeof(rujd), pData, sizeof(pData), &cb, NULL)){
        //     if(cb == 8) break;
        //     PUSN_RECORD pRecord = (PUSN_RECORD) &pData[sizeof(USN)];
        //     while ((PBYTE) pRecord < (pData + cb)){
        //         wstring sz((LPCWSTR) ((PBYTE) pRecord + pRecord->FileNameOffset), pRecord->FileNameLength / sizeof(WCHAR));
        //         if ((pRecord->Reason & USN_REASON_FILE_CREATE) == USN_REASON_FILE_CREATE){
        //             AddFile(pRecord->FileReferenceNumber, sz, pRecord->ParentFileReferenceNumber);
        //         }
        //         else if ((pRecord->Reason & USN_REASON_FILE_DELETE) == USN_REASON_FILE_DELETE){
        //             m_FileMap.remove(pRecord->FileReferenceNumber);
        //         }
        //         else if ((pRecord->Reason & USN_REASON_RENAME_NEW_NAME) == USN_REASON_RENAME_NEW_NAME){
        //             AddFile(pRecord->FileReferenceNumber, sz, pRecord->ParentFileReferenceNumber);
        //         }
        //         pRecord = (PUSN_RECORD) ((PBYTE) pRecord + pRecord->RecordLength);
        //     }
        //     rujd.StartUsn = *(USN *)&pData;
        // }
        // m_FileMapMutex.unlock();

        // m_StartUSN = rujd.StartUsn;
    }

    // TODO
    fn serialization_write(&self) {
        println!("TODO Volume::SerializationWrite");
        if self.file_map.is_empty() {return;};

        // QString appPath = QApplication::applicationDirPath(); // get programe path
        // QFile file(appPath + "/userdata/" + m_drive + ".fd");
        // file.open(QIODevice::WriteOnly | QIODevice::Truncate);
        // QDataStream out(&file);

        // out<<m_StartUSN;

        // QMapIterator<DWORDLONG, File*> i(m_FileMap);
        // while (i.hasNext()){
        //     i.next();
        //     const File* filedata = i.value();
        //     out<<i.key()<<filedata->parentIndex<<filedata->fileName<<(quint32)filedata->filter<<filedata->rank;
        // }

        // this->ReleaseIndex(false);
        // file.close();
    }

    // TODO
    fn serialization_read(&self) {
        println!("TODO Volume::SerializationRead")
        // QString appPath = QApplication::applicationDirPath(); // get programe path
        // QFile file(appPath + "/userdata/" + m_drive + ".fd");
        // file.open(QIODevice::ReadOnly);
        // QDataStream in(&file);

        // DWORDLONG index;
        // DWORDLONG parentIndex;
        // QByteArray fileName;
        // quint32 filter;
        // char rank;

        // in>>m_StartUSN;

        // m_FileMapMutex.lock();
        // while(in.atEnd() == false){
        //     in>>index>>parentIndex>>fileName>>filter>>rank;
        //     m_FileMap[index] = new File(parentIndex, fileName, filter, rank);
        // }
        // m_FileMapMutex.unlock();

        // file.close();
    }

}