use super::super::excluded_dirs::ExcludedDirs;
use super::cache::check_cancel;
use super::ntfs_file_map::FileMap;
use super::{cache, metadata_modified_at, usn, SearchCursor, SearchPage, VolumeIndexStatus};
use std::error::Error;
use std::ffi::CString;
use std::io;
use std::sync::{atomic::AtomicBool, Arc};
use windows::Win32::{
    Foundation,
    Storage::FileSystem,
    System::{Ioctl, IO},
};

struct DriveHandle(Foundation::HANDLE);
impl Drop for DriveHandle {
    fn drop(&mut self) {
        let _ = unsafe { Foundation::CloseHandle(self.0) };
    }
}

/// Sizes for a change journal this process creates when a volume has none.
const JOURNAL_MAXIMUM_SIZE: u64 = 32 * 1024 * 1024;
const JOURNAL_ALLOCATION_DELTA: u64 = 4 * 1024 * 1024;

fn os_error(error: windows::core::Error) -> io::Error {
    io::Error::from_raw_os_error((error.code().0 as u32 & 0xffff) as i32)
}
fn is_journal_inactive(error: &io::Error) -> bool {
    error.raw_os_error() == Some(Foundation::ERROR_JOURNAL_NOT_ACTIVE.0 as i32)
}

pub struct Volume {
    pub drive: String,
    cache_path: std::path::PathBuf,
    drive_frn: u64,
    ujd: Ioctl::USN_JOURNAL_DATA_V0,
    file_map: FileMap,
    saved_item_count: usize,
    dirty: bool,
    /// The volume has no change journal and none could be created: every
    /// refresh enumerates the MFT instead of replaying journal records.
    mft_only: bool,
    excluded_dirs: ExcludedDirs,
}

impl Volume {
    pub fn new(drive: String) -> Volume {
        #[cfg(not(test))]
        let excluded_dirs = ExcludedDirs::from_config();
        #[cfg(test)]
        let excluded_dirs = ExcludedDirs::default();
        Self {
            cache_path: cache::index_path(&drive, &excluded_dirs),
            drive,
            drive_frn: 0x5000000000005,
            ujd: Ioctl::USN_JOURNAL_DATA_V0::default(),
            file_map: FileMap::new(),
            saved_item_count: 0,
            dirty: false,
            mft_only: false,
            excluded_dirs,
        }
    }

    fn open_drive(&self) -> io::Result<DriveHandle> {
        self.open_drive_with(Foundation::GENERIC_READ.0)
    }

    fn open_drive_with(&self, access: u32) -> io::Result<DriveHandle> {
        let name = CString::new(format!("\\\\.\\{}:", self.drive)).map_err(io::Error::other)?;
        unsafe {
            FileSystem::CreateFileA(
                windows::core::PCSTR(name.as_ptr().cast()),
                access,
                FileSystem::FILE_SHARE_READ | FileSystem::FILE_SHARE_WRITE,
                None,
                FileSystem::OPEN_EXISTING,
                FileSystem::FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
            .map(DriveHandle)
            .map_err(os_error)
        }
    }

    fn query_journal(&mut self, drive: &DriveHandle) -> io::Result<()> {
        let mut returned = 0;
        unsafe {
            IO::DeviceIoControl(
                drive.0,
                Ioctl::FSCTL_QUERY_USN_JOURNAL,
                None,
                0,
                Some((&mut self.ujd as *mut Ioctl::USN_JOURNAL_DATA_V0).cast()),
                std::mem::size_of_val(&self.ujd) as u32,
                Some(&mut returned),
                None,
            )
        }
        .map_err(os_error)?;
        if returned < std::mem::size_of_val(&self.ujd) as u32 {
            return Err(cache::invalid("Truncated USN journal metadata"));
        }
        Ok(())
    }

    /// Creating a journal writes volume metadata, so it needs a write handle;
    /// an unprivileged process fails here and falls back to MFT-only refreshes.
    fn create_journal(&self) -> io::Result<()> {
        let drive =
            self.open_drive_with(Foundation::GENERIC_READ.0 | Foundation::GENERIC_WRITE.0)?;
        let request = Ioctl::CREATE_USN_JOURNAL_DATA {
            MaximumSize: JOURNAL_MAXIMUM_SIZE,
            AllocationDelta: JOURNAL_ALLOCATION_DELTA,
        };
        let mut returned = 0;
        unsafe {
            IO::DeviceIoControl(
                drive.0,
                Ioctl::FSCTL_CREATE_USN_JOURNAL,
                Some((&request as *const Ioctl::CREATE_USN_JOURNAL_DATA).cast()),
                std::mem::size_of_val(&request) as u32,
                None,
                0,
                Some(&mut returned),
                None,
            )
        }
        .map_err(os_error)
    }

    /// Query the journal, creating one when the volume has none. Returns whether
    /// journal replay is possible; otherwise the volume stays searchable through
    /// full MFT enumeration on every refresh.
    fn ensure_journal(&mut self, drive: &DriveHandle) -> io::Result<bool> {
        let inactive = match self.query_journal(drive) {
            Ok(()) => {
                if self.mft_only {
                    log::info!("{} USN journal is active again", self.drive);
                }
                self.mft_only = false;
                return Ok(true);
            }
            Err(error) if is_journal_inactive(&error) => error,
            Err(error) => return Err(error),
        };
        match self
            .create_journal()
            .and_then(|()| self.query_journal(drive))
        {
            Ok(()) => {
                log::info!("{} Created USN journal", self.drive);
                self.mft_only = false;
                Ok(true)
            }
            Err(error) => {
                if !self.mft_only {
                    log::warn!(
                        "{} {inactive}; creating one failed ({error}); refreshing by full MFT enumeration only",
                        self.drive
                    );
                }
                self.ujd = Ioctl::USN_JOURNAL_DATA_V0::default();
                self.mft_only = true;
                Ok(false)
            }
        }
    }

    pub fn index_status(&self) -> VolumeIndexStatus {
        let metadata = self.cache_path.metadata().ok();
        let count = if self.file_map.is_empty() {
            self.saved_item_count
        } else {
            self.file_map.len()
        };
        VolumeIndexStatus {
            name: self.drive.clone(),
            indexed: !self.file_map.is_empty() || metadata.is_some(),
            index_item_count: (count > 0).then_some(count),
            index_file_size_bytes: metadata.as_ref().map_or(0, |metadata| metadata.len()),
            index_file_modified_at: metadata.as_ref().and_then(metadata_modified_at),
        }
    }

    /// Startup can reuse a verified snapshot; an explicit rebuild still enumerates MFT.
    /// The caller labels the volume, so this refresh leaves the error unprefixed.
    pub fn initialize_index(&mut self, cancel: &AtomicBool) -> io::Result<()> {
        let result = self.refresh_index(Some(cancel)).and_then(|_| {
            check_cancel(Some(cancel))?;
            self.release_index()
        });
        if result.is_err() {
            self.clear_index();
        }
        result
    }

    pub fn build_index_with_cancel(&mut self, cancel: Option<&AtomicBool>) -> io::Result<()> {
        let result = self.scan_index(cancel).and_then(|_| {
            check_cancel(cancel)?;
            self.serialization_write()
        });
        if result.is_err() {
            self.clear_index();
        }
        result
    }

    fn scan_index(&mut self, cancel: Option<&AtomicBool>) -> io::Result<()> {
        check_cancel(cancel)?;
        self.clear_index();
        let drive = self.open_drive()?;
        // Without a journal the position is zero and the snapshot never resumes.
        self.ensure_journal(&drive)?;
        self.file_map.start_usn = self.ujd.NextUsn;
        self.file_map.journal_id = self.ujd.UsnJournalID;
        self.file_map
            .insert(self.drive_frn, format!("{}:", self.drive), 0);
        let mut request = Ioctl::MFT_ENUM_DATA_V0 {
            StartFileReferenceNumber: 0,
            LowUsn: 0,
            // MFT filtering uses each file's most recent USN. Capping this at
            // the initial journal position would omit existing files whose
            // contents change during enumeration (outside our rename mask).
            HighUsn: i64::MAX,
        };
        let mut data = vec![0u8; 512 * 1024];
        // MFT order is not directory order. Keep descendants whose parents have
        // not been seen yet; path filtering rejects excluded ancestors at search.
        let mut excluded = std::collections::HashSet::new();
        loop {
            check_cancel(cancel)?;
            let mut returned = 0;
            let result = unsafe {
                IO::DeviceIoControl(
                    drive.0,
                    Ioctl::FSCTL_ENUM_USN_DATA,
                    Some((&request as *const Ioctl::MFT_ENUM_DATA_V0).cast()),
                    std::mem::size_of_val(&request) as u32,
                    Some(data.as_mut_ptr().cast()),
                    data.len() as u32,
                    Some(&mut returned),
                    None,
                )
            };
            if let Err(error) = result {
                if error.code()
                    == windows::core::HRESULT::from_win32(Foundation::ERROR_HANDLE_EOF.0)
                {
                    break;
                }
                return Err(os_error(error));
            }
            let bytes = data
                .get(..returned as usize)
                .ok_or_else(|| cache::invalid("Invalid MFT buffer length"))?;
            let (next, records) = usn::records(bytes)?;
            for record in records {
                check_cancel(cancel)?;
                let record = record?;
                if record.index == self.drive_frn {
                    continue;
                }
                if excluded.contains(&record.parent)
                    || self.excluded_dirs.is_excluded_name(&record.name)
                {
                    excluded.insert(record.index);
                } else {
                    self.file_map
                        .insert(record.index, record.name, record.parent);
                }
            }
            if next <= request.StartFileReferenceNumber {
                return Err(cache::invalid("MFT enumeration did not advance"));
            }
            request.StartFileReferenceNumber = next;
        }
        self.dirty = true;
        Ok(())
    }

    fn clear_index(&mut self) {
        self.file_map.clear();
        self.dirty = false;
    }

    pub fn release_index(&mut self) -> io::Result<()> {
        let result = if self.dirty {
            self.checkpoint()
        } else {
            Ok(())
        };
        self.clear_index();
        result
    }

    pub fn find(
        &mut self,
        query: String,
        cursor: Option<SearchCursor>,
        batch: u8,
        cancel: Arc<AtomicBool>,
    ) -> Option<SearchPage> {
        if query.is_empty() || check_cancel(Some(&cancel)).is_err() {
            return None;
        }
        if self.file_map.is_empty() && self.update_index_with_cancel(Some(&cancel)).is_err() {
            return None;
        }
        self.file_map
            .search(&query, cursor.as_ref(), batch, &cancel, &self.excluded_dirs)
    }

    pub fn update_index_with_cancel(&mut self, cancel: Option<&AtomicBool>) -> io::Result<()> {
        self.refresh_index(cancel)
            .map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", self.drive)))
    }

    /// Refresh the index, dropping it on failure. The error is not labelled with
    /// the drive: callers that report it unlabelled add the prefix themselves.
    fn refresh_index(&mut self, cancel: Option<&AtomicBool>) -> io::Result<()> {
        let result = self.update_index_inner(cancel);
        if result.is_err() {
            self.clear_index();
        }
        result
    }

    fn update_index_inner(&mut self, cancel: Option<&AtomicBool>) -> io::Result<()> {
        check_cancel(cancel)?;
        if self.file_map.is_empty() && self.serialization_read_with_cancel(cancel).is_err() {
            self.scan_index(cancel)?;
            if self.mft_only {
                return Ok(());
            }
        }
        for attempt in 0..2 {
            check_cancel(cancel)?;
            let drive = self.open_drive()?;
            let result = if !self.ensure_journal(&drive)? {
                Err(cache::invalid("USN journal is not active"))
            } else if self.can_resume_journal() {
                self.replay_journal(&drive, cancel)
            } else {
                Err(cache::invalid("USN snapshot expired or journal replaced"))
            };
            match result {
                Ok(()) => return Ok(()),
                Err(error)
                    if attempt == 0
                        && (error.kind() == io::ErrorKind::InvalidData
                            || matches!(error.raw_os_error(), Some(code) if code == Foundation::ERROR_JOURNAL_ENTRY_DELETED.0 as i32
                        || code == Foundation::ERROR_JOURNAL_DELETE_IN_PROGRESS.0 as i32)) =>
                {
                    drop(drive);
                    log::info!("{} Rebuilding invalid USN snapshot: {error}", self.drive);
                    self.scan_index(cancel)?;
                    // Without a journal the fresh enumeration is the whole refresh.
                    if self.mft_only {
                        return Ok(());
                    }
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!("second attempt returns")
    }

    fn can_resume_journal(&self) -> bool {
        !self.mft_only
            && self.file_map.journal_id == self.ujd.UsnJournalID
            && self.file_map.start_usn >= self.ujd.FirstUsn.max(self.ujd.LowestValidUsn)
            && self.file_map.start_usn <= self.ujd.NextUsn
    }

    fn replay_journal(
        &mut self,
        drive: &DriveHandle,
        cancel: Option<&AtomicBool>,
    ) -> io::Result<()> {
        let mut request = Ioctl::READ_USN_JOURNAL_DATA_V0 {
            StartUsn: self.file_map.start_usn,
            ReasonMask: Ioctl::USN_REASON_FILE_CREATE
                | Ioctl::USN_REASON_FILE_DELETE
                | Ioctl::USN_REASON_RENAME_NEW_NAME
                | Ioctl::USN_REASON_RENAME_OLD_NAME,
            ReturnOnlyOnClose: 0,
            Timeout: 0,
            BytesToWaitFor: 0,
            UsnJournalID: self.file_map.journal_id,
        };
        let mut data = vec![0u8; 512 * 1024];
        while request.StartUsn < self.ujd.NextUsn {
            check_cancel(cancel)?;
            let mut returned = 0;
            unsafe {
                IO::DeviceIoControl(
                    drive.0,
                    Ioctl::FSCTL_READ_USN_JOURNAL,
                    Some((&request as *const Ioctl::READ_USN_JOURNAL_DATA_V0).cast()),
                    std::mem::size_of_val(&request) as u32,
                    Some(data.as_mut_ptr().cast()),
                    data.len() as u32,
                    Some(&mut returned),
                    None,
                )
            }
            .map_err(os_error)?;
            let bytes = data
                .get(..returned as usize)
                .ok_or_else(|| cache::invalid("Invalid USN buffer length"))?;
            let (next, records) = usn::records(bytes)?;
            for record in records {
                check_cancel(cancel)?;
                self.apply_record(record?);
            }
            let next =
                i64::try_from(next).map_err(|_| cache::invalid("Invalid USN continuation"))?;
            if next <= request.StartUsn {
                return Err(cache::invalid("USN journal did not advance"));
            }
            request.StartUsn = next;
        }
        self.dirty |= self.file_map.start_usn != request.StartUsn;
        self.file_map.start_usn = request.StartUsn;
        Ok(())
    }

    fn apply_record(&mut self, record: usn::Record) {
        if record.index == self.drive_frn {
            return;
        }
        // Reasons accumulate: deletion must win over an earlier creation.
        if record.reason & Ioctl::USN_REASON_FILE_DELETE != 0 {
            self.file_map.remove(&record.index);
        } else if record.reason
            & (Ioctl::USN_REASON_FILE_CREATE | Ioctl::USN_REASON_RENAME_NEW_NAME)
            != 0
        {
            if self.excluded_dirs.is_excluded_name(&record.name) {
                self.file_map.remove(&record.index);
            } else {
                self.file_map
                    .insert(record.index, record.name, record.parent);
            }
        } else if record.reason & Ioctl::USN_REASON_RENAME_OLD_NAME != 0 {
            self.file_map.remove(&record.index);
        }
    }

    fn checkpoint(&mut self) -> io::Result<()> {
        self.file_map.save(&self.cache_path.to_string_lossy())?;
        self.saved_item_count = self.file_map.len();
        self.dirty = false;
        Ok(())
    }

    fn serialization_write(&mut self) -> io::Result<()> {
        self.serialization_write_to(&self.index_file_path())
    }

    fn serialization_write_to(&mut self, path: &std::path::Path) -> io::Result<()> {
        let result = (|| {
            if self.file_map.is_empty() {
                return Ok(());
            }
            // `cache::write` creates the parent directory before writing.
            self.file_map.save(&path.to_string_lossy())?;
            self.saved_item_count = self.file_map.len();
            Ok(())
        })();
        self.clear_index();
        result
    }

    #[cfg(test)]
    fn serialization_read(&mut self) -> Result<(), Box<dyn Error>> {
        self.serialization_read_with_cancel(None)
    }

    fn serialization_read_with_cancel(
        &mut self,
        cancel: Option<&AtomicBool>,
    ) -> Result<(), Box<dyn Error>> {
        self.file_map
            .read_with_cancel(&self.index_file_path().to_string_lossy(), cancel)?;
        self.saved_item_count = self.file_map.len();
        self.dirty = false;
        Ok(())
    }

    fn index_file_path(&self) -> std::path::PathBuf {
        self.cache_path.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::super::release_tests::IndexFile;
    use super::*;
    use std::fs;

    fn synthetic_volume() -> Volume {
        // Construct directly to avoid configuration/profile reads and drive I/O.
        let mut volume = Volume {
            drive: "synthetic".into(),
            cache_path: std::path::PathBuf::new(),
            drive_frn: 42,
            ujd: Ioctl::USN_JOURNAL_DATA_V0 {
                UsnJournalID: 123,
                NextUsn: 456,
                ..Default::default()
            },
            file_map: FileMap::new(),
            saved_item_count: 128,
            dirty: false,
            mft_only: false,
            excluded_dirs: ExcludedDirs::default(),
        };
        volume.file_map.start_usn = 456;
        volume.file_map.journal_id = 123;
        volume
    }

    fn populate(volume: &mut Volume) {
        volume.file_map.insert(1, "fixture-a.txt".into(), 0);
        volume.file_map.insert(2, "fixture-b.txt".into(), 0);
    }

    fn assert_released(volume: &Volume) {
        assert!(volume.file_map.is_empty());

        assert_eq!(volume.file_map.start_usn, 456);
    }

    #[test]
    fn resume_requires_matching_journal_and_available_usn_range() {
        let mut volume = synthetic_volume();
        volume.ujd.FirstUsn = 100;
        volume.ujd.LowestValidUsn = 200;
        volume.ujd.NextUsn = 500;
        assert!(volume.can_resume_journal());
        for (journal, usn) in [(122, 456), (123, 199), (123, 501)] {
            volume.file_map.journal_id = journal;
            volume.file_map.start_usn = usn;
            assert!(!volume.can_resume_journal());
        }
    }

    #[test]
    fn volumes_without_a_journal_never_resume_from_snapshots() {
        let mut volume = synthetic_volume();
        assert!(volume.can_resume_journal());
        volume.mft_only = true;
        assert!(!volume.can_resume_journal());
        // An MFT-only snapshot records a zero position; it must not match an
        // inactive journal's zeroed metadata either.
        volume.ujd = Ioctl::USN_JOURNAL_DATA_V0::default();
        volume.file_map.journal_id = 0;
        volume.file_map.start_usn = 0;
        assert!(!volume.can_resume_journal());
        assert!(is_journal_inactive(&io::Error::from_raw_os_error(
            Foundation::ERROR_JOURNAL_NOT_ACTIVE.0 as i32
        )));
        assert!(!is_journal_inactive(&io::Error::from_raw_os_error(
            Foundation::ERROR_JOURNAL_ENTRY_DELETED.0 as i32
        )));
        assert!(!is_journal_inactive(&cache::invalid("not an OS error")));
    }

    #[test]
    fn release_checkpoints_latest_changes_and_cursor() {
        let index = IndexFile::new();
        let mut volume = synthetic_volume();
        volume.cache_path = index.path().into();
        populate(&mut volume);
        volume.file_map.start_usn = 789;
        volume.dirty = true;
        volume.release_index().unwrap();
        assert!(volume.file_map.is_empty());
        assert!(!volume.dirty);
        volume.serialization_read().unwrap();
        assert_eq!(volume.file_map.len(), 2);
        assert_eq!(volume.file_map.start_usn, 789);
        assert_eq!(volume.file_map.journal_id, 123);
    }

    #[test]
    fn accumulated_delete_reason_wins_over_creation() {
        let mut volume = synthetic_volume();
        populate(&mut volume);
        volume.apply_record(usn::Record {
            index: 1,
            parent: 0,
            name: "fixture-a.txt".into(),
            reason: Ioctl::USN_REASON_FILE_CREATE | Ioctl::USN_REASON_FILE_DELETE,
        });
        assert!(!volume.file_map.contains_index(&1));
        assert!(volume.file_map.contains_index(&2));
    }

    #[test]
    fn persistence_success_releases_memory_and_can_reload() {
        let index = IndexFile::new();
        let mut volume = synthetic_volume();
        populate(&mut volume);
        volume
            .serialization_write_to(std::path::Path::new(index.path()))
            .unwrap();
        assert_released(&volume);
        assert_eq!(volume.saved_item_count, 2);

        volume.file_map.read(index.path()).unwrap();
        assert_eq!(volume.file_map.len(), 2);
        assert!(volume.file_map.contains_index(&1));
        assert!(volume.file_map.contains_index(&2));
        assert_eq!(volume.file_map.start_usn, 456);
    }

    #[test]
    fn directory_creation_failure_releases_memory_and_reports_error() {
        let blocked_directory = IndexFile::new();
        let mut volume = synthetic_volume();
        populate(&mut volume);
        let result = volume.serialization_write_to(
            &std::path::Path::new(blocked_directory.path()).join("nested/index.fd"),
        );
        assert!(result.is_err());
        assert_released(&volume);
        assert_eq!(volume.saved_item_count, 128);
    }

    #[test]
    fn file_write_failure_releases_memory_and_allows_retry() {
        use std::os::windows::fs::OpenOptionsExt;

        let index = IndexFile::new();
        let path = std::path::Path::new(index.path());
        fs::write(path, b"existing synthetic index").unwrap();
        let locked_file = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(path)
            .unwrap();
        let mut volume = synthetic_volume();
        populate(&mut volume);

        assert!(volume.serialization_write_to(path).is_err());
        assert_released(&volume);
        assert_eq!(volume.saved_item_count, 128);
        drop(locked_file);
        assert_eq!(fs::read(path).unwrap(), b"existing synthetic index");

        // A subsequent rebuild can persist and release normally.
        populate(&mut volume);
        volume.serialization_write_to(path).unwrap();
        assert_released(&volume);
        assert_eq!(volume.saved_item_count, 2);
        volume.file_map.read(index.path()).unwrap();
        assert_eq!(volume.file_map.len(), 2);
    }

    #[test]
    fn persistence_of_empty_index_resets_paging_without_writing() {
        let index = IndexFile::new();
        let mut volume = synthetic_volume();
        populate(&mut volume);
        volume.file_map.remove(&1);
        volume.file_map.remove(&2);
        volume
            .serialization_write_to(std::path::Path::new(index.path()))
            .unwrap();
        assert_released(&volume);
        assert_eq!(volume.saved_item_count, 128);
        assert_eq!(fs::metadata(index.path()).unwrap().len(), 0);
    }

    #[test]
    fn release_resets_paging_even_when_empty_and_keeps_volume_metadata() {
        let mut volume = synthetic_volume();

        for remove_all in [false, true] {
            volume.file_map.insert(1, "fixture-a.txt".into(), 0);
            volume.file_map.insert(2, "fixture-b.txt".into(), 0);

            if remove_all {
                volume.file_map.remove(&1);
                volume.file_map.remove(&2);
            }
            volume.release_index().unwrap();

            assert!(volume.file_map.is_empty());
            assert_eq!(volume.file_map.start_usn, 456);
            assert_eq!(volume.drive, "synthetic");
            assert_eq!(volume.drive_frn, 42);
            assert_eq!(volume.ujd.UsnJournalID, 123);
            assert_eq!(volume.ujd.NextUsn, 456);
            assert_eq!(volume.saved_item_count, 128);

            // Reload synthetically; find must restart the same query at page 1.
            volume.file_map.insert(1, "fixture-a.txt".into(), 0);
            volume.file_map.insert(2, "fixture-b.txt".into(), 0);
            let page = volume
                .find("fixture".into(), None, 1, Arc::new(AtomicBool::new(false)))
                .unwrap();
            assert_eq!(page.items.len(), 1);
            assert_eq!(page.items[0].file_name, "fixture-b.txt");

            volume.release_index().unwrap();
        }
    }
}
