use std::collections::HashSet;
use std::error::Error;
use std::ffi::{c_void, CString};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
#[cfg(debug_assertions)]
use std::time::SystemTime;
use std::{fs, io};
use windows::Win32::Foundation;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem;
use windows::Win32::System::{Ioctl, IO};

use super::super::excluded_dirs::ExcludedDirs;
use super::ntfs_file_map::FileMap;
use super::{index_file_stem, metadata_modified_at, SearchResultItem, VolumeIndexStatus};
use rotor_platform::file_util;

pub struct Volume {
    pub drive: String,
    drive_frn: u64,
    ujd: Ioctl::USN_JOURNAL_DATA_V0,
    file_map: FileMap,
    last_query: String,
    last_search_num: usize,
    saved_item_count: usize,
    excluded_dirs: ExcludedDirs,
}

impl Volume {
    pub fn new(drive: String) -> Volume {
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
            last_query: String::new(),
            last_search_num: 0,
            saved_item_count: 0,
            excluded_dirs: ExcludedDirs::from_config(),
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

    pub fn index_status(&self) -> VolumeIndexStatus {
        let index_file_path = self.index_file_path();
        let index_file_metadata = index_file_path.metadata().ok();
        let loaded_item_count = self.file_map.len();
        let index_item_count = match loaded_item_count.max(self.saved_item_count) {
            0 => None,
            count => Some(count),
        };

        VolumeIndexStatus {
            name: self.drive.clone(),
            indexed: loaded_item_count > 0 || index_file_metadata.is_some(),
            index_item_count,
            index_file_size_bytes: index_file_metadata
                .as_ref()
                .map(|metadata| metadata.len())
                .unwrap_or(0),
            index_file_modified_at: index_file_metadata.as_ref().and_then(metadata_modified_at),
        }
    }

    // Enumerate the MFT for all entries. Store the file reference numbers of any directories in the database.
    pub fn build_index(&mut self) {
        self.build_index_with_cancel(None);
    }

    fn build_index_with_cancel(&mut self, cancel: Option<&AtomicBool>) {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();
        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::build_index", self.drive);

        self.excluded_dirs = ExcludedDirs::from_config();
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
        let excluded_dirs = self.excluded_dirs.clone();
        let mut excluded_indexes = HashSet::new();

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
                    if cancel
                        .map(|cancel| cancel.load(Ordering::Relaxed))
                        .unwrap_or(false)
                    {
                        log::info!("{} Volume::build_index cancelled by user", self.drive);
                        Self::close_drive(h_vol);
                        return;
                    }

                    let record = &*record_ptr;

                    let file_name_begin_ptr =
                        (record_ptr as usize + record.FileNameOffset as usize) as *const u16;
                    let file_name_length =
                        record.FileNameLength as usize / std::mem::size_of::<u16>();
                    let file_name_list =
                        std::slice::from_raw_parts(file_name_begin_ptr, file_name_length);
                    let file_name =
                        String::from_utf16(file_name_list).unwrap_or(String::from("unknown"));

                    if excluded_indexes.contains(&record.ParentFileReferenceNumber)
                        || excluded_dirs.is_excluded_name(&file_name)
                    {
                        excluded_indexes.insert(record.FileReferenceNumber);
                    } else {
                        self.file_map.insert(
                            record.FileReferenceNumber,
                            file_name,
                            record.ParentFileReferenceNumber,
                        );
                    }
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
        cancel: Arc<AtomicBool>,
        sender: mpsc::Sender<Option<Vec<SearchResultItem>>>,
    ) {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();

        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::Find {query}", self.drive);

        if query.is_empty() || cancel.load(Ordering::Relaxed) {
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
                self.build_index_with_cancel(Some(&cancel));
            });
        };

        let (result, search_num) = self.file_map.search(
            &query,
            self.last_search_num,
            batch,
            &cancel,
            &self.excluded_dirs,
        );

        #[cfg(debug_assertions)]
        log::info!(
            "{} End Volume::Find {query}, use time: {:?} ms",
            self.drive,
            sys_time.elapsed().unwrap_or_default().as_millis()
        );

        if cancel.load(Ordering::Relaxed) {
            let _ = sender.send(None);
            return;
        }

        if result.is_some() {
            self.last_search_num += search_num;
        }

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
                        if self.is_excluded_record(&file_name, record.ParentFileReferenceNumber) {
                            self.file_map.remove(&record.FileReferenceNumber);
                        } else {
                            self.file_map.insert(
                                record.FileReferenceNumber,
                                file_name,
                                record.ParentFileReferenceNumber,
                            );
                        }
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

        let index_dir = file_util::get_tmp_path();
        fs::create_dir_all(index_dir)?;
        self.saved_item_count = self.file_map.len();
        self.file_map
            .save(&self.index_file_path().to_string_lossy())?;

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

        self.file_map
            .read(&self.index_file_path().to_string_lossy())?;
        self.saved_item_count = self.file_map.len();

        #[cfg(debug_assertions)]
        log::info!(
            "{} End Volume::serialization_read, use time: {:?} ms",
            self.drive,
            sys_time.elapsed().unwrap_or_default().as_millis()
        );

        Ok(())
    }

    fn index_file_path(&self) -> std::path::PathBuf {
        file_util::get_tmp_path().join(format!("{}.fd", index_file_stem(&self.drive)))
    }

    fn is_excluded_record(&self, file_name: &str, parent_index: u64) -> bool {
        if self.excluded_dirs.is_excluded_name(file_name) {
            return true;
        }

        if parent_index == 0 || parent_index == self.drive_frn {
            return false;
        }

        !self.file_map.contains_index(&parent_index)
    }
}
