use std::error::Error;
use std::ffi::{c_void, CString};
use std::sync::mpsc;
#[allow(unused_imports)]
use std::time::SystemTime;
use std::{fs, io};
use windows::Win32::Foundation;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem;
use windows::Win32::System::{Ioctl, IO};

use super::ntfs_file_map::FileMap;
use super::SearchResultItem;
#[allow(unused_imports)]
use crate::util::file_util;

pub struct Volume {
    pub drive: String,
    drive_frn: u64,
    ujd: Ioctl::USN_JOURNAL_DATA_V0,
    file_map: FileMap,
    stop_receiver: mpsc::Receiver<()>,
    last_query: String,
    last_search_num: usize,
}

impl Volume {
    pub fn new(drive: String, stop_receiver: mpsc::Receiver<()>) -> Volume {
        Volume {
            drive,
            drive_frn: 0x5000000000005,
            file_map: FileMap::new(),
            ujd: Ioctl::USN_JOURNAL_DATA_V0 {
                UsnJournalID: 0x0,
                FirstUsn: 0x0,
                NextUsn: 0x0,
                LowestValidUsn: 0x0,
                MaxUsn: 0x0,
                MaximumSize: 0x0,
                AllocationDelta: 0x0,
            },
            stop_receiver,
            last_query: String::new(),
            last_search_num: 0,
        }
    }

    // This is a helper function that opens a handle to the volume specified by the cDriveLetter parameter.
    fn open_drive(drive_letter: &str) -> Foundation::HANDLE {
        unsafe {
            if let Ok(c_str) = CString::new(format!("\\\\.\\{}:", drive_letter)) {
                FileSystem::CreateFileA(
                    windows::core::PCSTR(c_str.as_ptr() as *const u8),
                    Foundation::GENERIC_READ.0,
                    FileSystem::FILE_SHARE_READ | FileSystem::FILE_SHARE_WRITE,
                    None,
                    FileSystem::OPEN_EXISTING,
                    windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
                    None,
                )
                .unwrap_or_default()
            } else {
                HANDLE::default()
            }
        }
    }

    // This is a helper function that close a handle.
    fn close_drive(h_vol: Foundation::HANDLE) {
        unsafe {
            Foundation::CloseHandle(h_vol)
                .unwrap_or_else(|e| log::error!("Volume::close_drive, error: {:?}", e));
        }
    }

    // Enumerate the MFT for all entries. Store the file reference numbers of any directories in the database.
    pub fn build_index(&mut self) {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();
        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::build_index", self.drive);

        self.release_index();

        let h_vol = Self::open_drive(&self.drive);

        // Query, Return statistics about the journal on the current volume
        let mut cd: u32 = 0;
        unsafe {
            IO::DeviceIoControl(
                h_vol,
                Ioctl::FSCTL_QUERY_USN_JOURNAL,
                None,
                0,
                Some(&mut self.ujd as *mut Ioctl::USN_JOURNAL_DATA_V0 as *mut c_void),
                std::mem::size_of::<Ioctl::USN_JOURNAL_DATA_V0>() as u32,
                Some(&mut cd),
                None,
            )
            .unwrap_or_else(|e| log::error!("{} Volume::build_index, error: {:?}", self.drive, e));
        };

        self.file_map.start_usn = self.ujd.NextUsn;

        // add the root directory
        let sz_root = format!("{}:", self.drive);
        self.file_map.insert(self.drive_frn, sz_root, 0);

        let mut med: Ioctl::MFT_ENUM_DATA_V0 = Ioctl::MFT_ENUM_DATA_V0 {
            StartFileReferenceNumber: 0,
            LowUsn: 0,
            HighUsn: self.ujd.NextUsn,
        };
        let mut data = [0u64; 0x10000];
        let mut cb: u32 = 0;

        unsafe {
            while IO::DeviceIoControl(
                h_vol,
                Ioctl::FSCTL_ENUM_USN_DATA,
                Some(&med as *const _ as *const c_void),
                std::mem::size_of::<Ioctl::MFT_ENUM_DATA_V0>() as u32,
                Some(data.as_mut_ptr() as *mut c_void),
                std::mem::size_of::<[u8; std::mem::size_of::<u64>() * 0x10000]>() as u32,
                Some(&mut cb as *mut u32),
                None,
            )
            .is_ok()
            {
                let mut record_ptr = data.as_ptr().offset(1) as *const Ioctl::USN_RECORD_V2;
                let data_end = data.as_ptr() as usize + cb as usize;

                while (record_ptr as usize) < data_end {
                    let record = &*record_ptr;

                    let file_name_begin_ptr =
                        (record_ptr as usize + record.FileNameOffset as usize) as *const u16;
                    let file_name_length =
                        record.FileNameLength as usize / std::mem::size_of::<u16>();
                    let file_name_list =
                        std::slice::from_raw_parts(file_name_begin_ptr, file_name_length);
                    let file_name =
                        String::from_utf16(file_name_list).unwrap_or(String::from("unknown"));

                    self.file_map.insert(
                        record.FileReferenceNumber,
                        file_name,
                        record.ParentFileReferenceNumber,
                    );
                    record_ptr = (record_ptr as usize + record.RecordLength as usize)
                        as *mut Ioctl::USN_RECORD_V2;
                }

                med.StartFileReferenceNumber = data[0];
            }
        }

        #[cfg(debug_assertions)]
        log::info!(
            "{} End Volume::build_index, use time: {:?} ms",
            self.drive,
            sys_time.elapsed().unwrap_or_default().as_millis()
        );

        Self::close_drive(h_vol);
        self.serialization_write().unwrap_or_else(|e| {
            log::error!("{} Volume::serialization_write, error: {:?}", self.drive, e)
        });
    }

    // Clears the database
    pub fn release_index(&mut self) {
        if self.file_map.is_empty() {
            return;
        }

        self.last_query = String::new();
        self.last_search_num = 0;

        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::release_index", self.drive);

        self.file_map.clear();
    }

    // searching
    pub fn find(
        &mut self,
        query: String,
        batch: u8,
        sender: mpsc::Sender<Option<Vec<SearchResultItem>>>,
    ) {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();

        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::Find {query}", self.drive);

        if query.is_empty() {
            let _ = sender.send(None);
            return;
        }

        if self.last_query != query {
            self.last_search_num = 0;
            self.last_query = query.clone();
        }

        if self.file_map.is_empty() {
            self.serialization_read().unwrap_or_else(|e| {
                log::error!("{} Volume::serialization_write, error: {:?}", self.drive, e);
                self.build_index();
            });
        };

        while self.stop_receiver.try_recv().is_ok() {} // clear channel before find
        let (result, search_num) =
            self.file_map
                .search(&query, self.last_search_num, batch, &self.stop_receiver);

        #[cfg(debug_assertions)]
        log::info!(
            "{} End Volume::Find {query}, use time: {:?} ms",
            self.drive,
            sys_time.elapsed().unwrap_or_default().as_millis()
        );

        self.last_search_num += search_num;

        let _ = sender.send(result);
    }

    // update index, add new file, remove deleted file
    pub fn update_index(&mut self) {
        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::update_index", self.drive);

        if self.file_map.is_empty() {
            self.serialization_read()
                .unwrap_or_else(|e: Box<dyn Error>| {
                    log::error!("{} Volume::serialization_write, error: {:?}", self.drive, e);
                    self.build_index();
                });
        };

        let mut data = [0i64; 0x10000];
        let mut cb: u32 = 0;
        let mut rujd: Ioctl::READ_USN_JOURNAL_DATA_V0 = Ioctl::READ_USN_JOURNAL_DATA_V0 {
            StartUsn: self.file_map.start_usn,
            ReasonMask: Ioctl::USN_REASON_FILE_CREATE
                | Ioctl::USN_REASON_FILE_DELETE
                | Ioctl::USN_REASON_RENAME_NEW_NAME
                | Ioctl::USN_REASON_RENAME_OLD_NAME,
            ReturnOnlyOnClose: 0,
            Timeout: 0,
            BytesToWaitFor: 0,
            UsnJournalID: self.ujd.UsnJournalID,
        };

        let h_vol = Self::open_drive(&self.drive);

        unsafe {
            while IO::DeviceIoControl(
                h_vol,
                Ioctl::FSCTL_READ_USN_JOURNAL,
                Some(&rujd as *const _ as *const c_void),
                std::mem::size_of::<Ioctl::READ_USN_JOURNAL_DATA_V0>() as u32,
                Some(data.as_mut_ptr() as *mut c_void),
                std::mem::size_of::<[u8; std::mem::size_of::<u64>() * 0x10000]>() as u32,
                Some(&mut cb as *mut u32),
                None,
            )
            .is_ok()
            {
                if cb == 8 {
                    break;
                };
                let mut record_ptr = data.as_ptr().offset(1) as *const Ioctl::USN_RECORD_V2;
                let data_end = data.as_ptr() as usize + cb as usize;

                while (record_ptr as usize) < data_end {
                    let record = &*record_ptr;
                    let file_name_begin_ptr =
                        (record_ptr as usize + record.FileNameOffset as usize) as *const u16;
                    let file_name_length =
                        record.FileNameLength as usize / std::mem::size_of::<u16>();
                    let file_name_list =
                        std::slice::from_raw_parts(file_name_begin_ptr, file_name_length);
                    let file_name =
                        String::from_utf16(file_name_list).unwrap_or(String::from("unknown"));

                    if record.Reason
                        & (Ioctl::USN_REASON_FILE_CREATE | Ioctl::USN_REASON_RENAME_NEW_NAME)
                        != 0
                    {
                        self.file_map.insert(
                            record.FileReferenceNumber,
                            file_name,
                            record.ParentFileReferenceNumber,
                        );
                    } else {
                        // Ioctl::USN_REASON_FILE_DELETE | Ioctl::USN_REASON_RENAME_OLD_NAME
                        self.file_map.remove(&record.FileReferenceNumber);
                    }

                    record_ptr = (record_ptr as usize + record.RecordLength as usize)
                        as *mut Ioctl::USN_RECORD_V2;
                }

                rujd.StartUsn = data[0];
            }
        }
        self.file_map.start_usn = rujd.StartUsn;
        Self::close_drive(h_vol);
    }

    // serializate file_map to reduce memory usage
    fn serialization_write(&mut self) -> Result<(), io::Error> {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();
        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::serialization_write", self.drive);

        if self.file_map.is_empty() {
            return Ok(());
        };

        let file_path = file_util::get_tmp_path();
        if !file_path.exists() {
            fs::create_dir(&file_path)?;
        }
        let file_name = format!("{}/{}.fd", file_path.to_str().unwrap_or("."), self.drive);

        self.file_map.save(&file_name)?;

        self.release_index();

        #[cfg(debug_assertions)]
        log::info!(
            "{} End Volume::serialization_write, use time: {:?} ms",
            self.drive,
            sys_time.elapsed().unwrap_or_default().as_millis()
        );

        Ok(())
    }

    // deserializate file_map from file
    fn serialization_read(&mut self) -> Result<(), Box<dyn Error>> {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();
        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::serialization_read", self.drive);

        let file_path = file_util::get_tmp_path();
        let file_name = format!("{}/{}.fd", file_path.to_str().unwrap_or("."), self.drive);
        self.file_map.read(&file_name)?;

        #[cfg(debug_assertions)]
        log::info!(
            "{} End Volume::serialization_read, use time: {:?} ms",
            self.drive,
            sys_time.elapsed().unwrap_or_default().as_millis()
        );

        Ok(())
    }
}
