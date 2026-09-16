#[cfg(test)]
use notify::event::{ModifyKind, RenameMode};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::error::Error;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::time::SystemTime;
use std::{fs, io};
use walkdir::{DirEntry, WalkDir};

use super::super::excluded_dirs::ExcludedDirs;
use super::default_file_map::FileMap;
use super::{cache, metadata_modified_at, SearchCursor, SearchPage, VolumeIndexStatus};

const EVENT_CAPACITY: usize = 1024;
const MAX_EVENT_PATHS: usize = 128;

#[derive(Debug, Clone, Copy)]
enum FileAction {
    Insert,
    Remove,
}

pub struct Volume {
    pub drive: String,
    cache_path: std::path::PathBuf,
    file_map: FileMap,
    watcher: Option<RecommendedWatcher>,
    event_receiver: Option<mpsc::Receiver<notify::Result<Event>>>,
    rescan_required: Arc<AtomicBool>,
    saved_item_count: usize,
    excluded_dirs: ExcludedDirs,
}

impl Volume {
    pub fn new(drive: String) -> Volume {
        #[cfg(not(test))]
        let excluded_dirs = ExcludedDirs::from_config();
        #[cfg(test)]
        let excluded_dirs = ExcludedDirs::default();
        Volume {
            cache_path: cache::index_path(&drive, &excluded_dirs),
            drive,
            file_map: FileMap::new(),
            watcher: None,
            event_receiver: None,
            rescan_required: Arc::new(AtomicBool::new(false)),
            saved_item_count: 0,
            excluded_dirs,
        }
    }

    pub fn start_watching(&mut self) -> Result<(), Box<dyn Error>> {
        if self.watcher.is_some() {
            return Ok(());
        }

        let root_path = self.drive.clone();

        if !std::path::Path::new(&root_path).exists() {
            return Err(format!("Root path {} does not exist", root_path).into());
        }

        let (tx, rx) = mpsc::sync_channel(EVENT_CAPACITY);
        let rescan = self.rescan_required.clone();
        let excluded = self.excluded_dirs.clone();
        let config = Config::default().with_poll_interval(std::time::Duration::from_secs(2));
        let mut watcher = notify::recommended_watcher(move |res| {
            enqueue_event(&tx, &rescan, &excluded, res);
        })?;

        watcher.configure(config)?;
        watcher.watch(root_path.as_ref(), RecursiveMode::Recursive)?;

        self.watcher = Some(watcher);
        self.event_receiver = Some(rx);

        log::info!(
            "{} File watching started for path: {}",
            self.drive,
            root_path
        );
        Ok(())
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

    pub fn stop_watching(&mut self) {
        if let Some(watcher) = self.watcher.take() {
            drop(watcher);
            log::info!("{} File watcher stopped", self.drive);
        }
        self.event_receiver = None;
    }

    #[cfg(test)]
    fn process_path(&mut self, path: &std::path::Path, action: FileAction) {
        self.process_path_with_cancel(path, action, None).unwrap();
    }

    fn process_path_with_cancel(
        &mut self,
        path: &std::path::Path,
        action: FileAction,
        cancel: Option<&AtomicBool>,
    ) -> io::Result<()> {
        cache::check_cancel(cancel)?;
        // Removed directories no longer have metadata; remove descendants by indexed path.
        if matches!(action, FileAction::Remove) {
            self.file_map.remove_subtree(path);
            return Ok(());
        }
        if self.is_ignored_event_path(path, action) {
            return Ok(());
        }
        // A directory arriving via rename/create may have no child notifications.
        let excluded = self.excluded_dirs.clone();
        for entry in WalkDir::new(path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| !is_ignored_walk_entry(entry, &excluded))
        {
            cache::check_cancel(cancel)?;
            match entry {
                Ok(entry) => {
                    if let (Some(name), Some(parent)) =
                        (entry.path().file_name(), entry.path().parent())
                    {
                        self.file_map.insert(
                            name.to_string_lossy().into_owned(),
                            parent.to_string_lossy().into_owned(),
                        );
                    }
                }
                Err(error) => {
                    self.rescan_required.store(true, Ordering::Release);
                    log::warn!("{} Failed to scan event path: {error}", self.drive);
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn handle_event(&mut self, event: Event) {
        use EventKind::*;
        use ModifyKind::Name;

        match event.kind {
            Create(_) => {
                for path in &event.paths {
                    self.process_path(path, FileAction::Insert);
                }
            }
            Remove(_) => {
                for path in &event.paths {
                    self.process_path(path, FileAction::Remove);
                }
            }
            Modify(Name(rename_mode)) => match rename_mode {
                RenameMode::From => {
                    for path in &event.paths {
                        self.process_path(path, FileAction::Remove);
                    }
                }
                RenameMode::To => {
                    for path in &event.paths {
                        self.process_path(path, FileAction::Insert);
                    }
                }
                RenameMode::Both => {
                    let mut paths = event.paths.iter();
                    if let (Some(from), Some(to)) = (paths.next(), paths.next()) {
                        self.process_path(from, FileAction::Remove);
                        self.process_path(to, FileAction::Insert);
                    }
                }
                _ => {
                    for path in &event.paths {
                        if path.exists() {
                            self.process_path(path, FileAction::Insert);
                        } else {
                            self.process_path(path, FileAction::Remove);
                        }
                    }
                }
            },
            _ => {}
        }
    }

    fn handle_file_events_with_cancel(&mut self, cancel: Option<&AtomicBool>) -> io::Result<()> {
        let Some(receiver) = self.event_receiver.take() else {
            return Ok(());
        };

        let mut paths = std::collections::HashSet::new();
        for event_result in receiver.try_iter().take(EVENT_CAPACITY) {
            match event_result {
                Ok(event) => paths.extend(event.paths),
                Err(_) => self.rescan_required.store(true, Ordering::Release),
            }
        }
        self.event_receiver = Some(receiver);
        // Reconcile final filesystem state once per path, regardless of event ordering.
        for path in paths {
            self.process_path_with_cancel(&path, FileAction::Remove, cancel)?;
            if path.exists() {
                self.process_path_with_cancel(&path, FileAction::Insert, cancel)?;
            }
        }

        Ok(())
    }

    // Enumerate the filesystem using walkdir. Store the file entries in the database.
    // Without a persistent filesystem event cursor, startup requires a scan.
    pub fn initialize_index(&mut self, cancel: &AtomicBool) -> io::Result<()> {
        self.build_index_with_cancel(Some(cancel))
    }

    #[cfg(all(test, unix))]
    pub fn build_index(&mut self) -> io::Result<()> {
        self.build_index_with_cancel(None)
    }

    pub fn build_index_with_cancel(&mut self, cancel: Option<&AtomicBool>) -> io::Result<()> {
        let result = self.scan_index_with_cancel(cancel);
        if result.is_err() {
            self.rescan_required.store(true, Ordering::Release);
            self.release_index_without_save();
        }
        result
    }

    fn scan_index_with_cancel(&mut self, cancel: Option<&AtomicBool>) -> io::Result<()> {
        cache::check_cancel(cancel)?;
        let sys_time = SystemTime::now();

        self.stop_watching();
        self.rescan_required.store(false, Ordering::Release);
        self.release_index_without_save();

        // Build the root path based on the drive letter
        let root_path = self.drive.clone();

        // Check if the root path exists
        if !std::path::Path::new(&root_path).exists() {
            log::error!(
                "{} Root path {} does not exist, skipping index build",
                self.drive,
                root_path
            );
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Root path {root_path} does not exist"),
            ));
        }

        self.start_watching()
            .map_err(|error| io::Error::other(error.to_string()))?;

        // Walk the directory tree using walkdir
        let walkdir = WalkDir::new(&root_path).follow_links(false); // don't follow symbolic links to avoid infinite loops
        let excluded_dirs = self.excluded_dirs.clone();

        let walker = walkdir
            .into_iter()
            .filter_entry(move |e| !is_ignored_walk_entry(e, &excluded_dirs));

        for entry in walker {
            if cancel
                .map(|cancel| cancel.load(Ordering::Relaxed))
                .unwrap_or(false)
            {
                log::info!("{} Volume::build_index cancelled by user", self.drive);
                self.release_index_without_save();
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "Index build cancelled",
                ));
            }

            let entry = match entry {
                Ok(entry) => entry,
                // A protected or vanished child must not discard the entire index.
                // Root traversal failures still make this volume unavailable.
                Err(error) if error.depth() > 0 => {
                    log::warn!("{} Skipping inaccessible index path: {error}", self.drive);
                    continue;
                }
                Err(error) => return Err(io::Error::other(error.to_string())),
            };

            let path = entry.path();

            // Get the file name
            let file_name = match entry.file_name().to_str() {
                Some(name) => name.to_string(),
                None => {
                    log::warn!("Invalid UTF-8 filename: {:?}", entry.file_name());
                    continue;
                }
            };

            // Get parent directory path
            let parent_path = match path.parent() {
                Some(parent) => parent.to_string_lossy().to_string(),
                None => root_path.clone(), // If no parent, use root
            };

            // Insert into file map
            self.file_map.insert(file_name, parent_path);
        }

        if cancel
            .map(|cancel| cancel.load(Ordering::Relaxed))
            .unwrap_or(false)
        {
            self.release_index_without_save();
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Index build cancelled",
            ));
        }

        log::info!(
            "{} End Volume::build_index, use time: {:?} ms",
            self.drive,
            sys_time.elapsed().unwrap_or_default().as_millis()
        );

        if let Err(e) = self.start_watching() {
            log::error!("{} Failed to start file watching: {:?}", self.drive, e);
        }

        let result = self.serialization_write();
        self.release_index_without_save();
        result
    }

    // searching
    pub fn find(
        &mut self,
        query: String,
        cursor: Option<SearchCursor>,
        batch: u8,
        cancel: Arc<AtomicBool>,
    ) -> Option<SearchPage> {
        if query.is_empty() || cancel.load(Ordering::Relaxed) {
            return None;
        }
        if self.file_map.is_empty() && self.serialization_read_with_cancel(Some(&cancel)).is_err() {
            if let Err(error) = self.build_index_with_cancel(Some(&cancel)) {
                log::error!("{} Rebuild index failed: {error}", self.drive);
                return None;
            }
            // Building persists and releases the index.
            if self.serialization_read_with_cancel(Some(&cancel)).is_err() {
                return None;
            }
        }
        self.file_map
            .search(&query, cursor.as_ref(), batch, &cancel)
    }

    // Clears the database
    pub fn release_index(&mut self) -> io::Result<()> {
        let result = self.serialization_write();
        self.release_index_without_save();
        result
    }

    pub fn release_index_without_save(&mut self) {
        self.file_map.clear();
    }

    // update index, add new file, remove deleted file
    #[cfg(test)]
    pub fn update_index(&mut self) -> io::Result<()> {
        self.update_index_with_cancel(None)
    }

    pub fn update_index_with_cancel(&mut self, cancel: Option<&AtomicBool>) -> io::Result<()> {
        let result = self.update_index_inner(cancel);
        if result.is_err() {
            self.rescan_required.store(true, Ordering::Release);
            self.release_index_without_save();
        }
        result.map_err(|error| io::Error::new(error.kind(), format!("{}: {error}", self.drive)))
    }

    fn update_index_inner(&mut self, cancel: Option<&AtomicBool>) -> io::Result<()> {
        cache::check_cancel(cancel)?;
        if self.file_map.is_empty() && self.serialization_read_with_cancel(cancel).is_err() {
            self.build_index_with_cancel(cancel)?;
            self.serialization_read_with_cancel(cancel)
                .map_err(|error| io::Error::other(error.to_string()))?;
        }
        self.start_watching()
            .map_err(|error| io::Error::other(error.to_string()))?;
        self.handle_file_events_with_cancel(cancel)?;
        if self.rescan_required.load(Ordering::Acquire) {
            if let Err(error) = self.build_index_with_cancel(cancel) {
                self.rescan_required.store(true, Ordering::Release);
                return Err(error);
            }
            self.serialization_read_with_cancel(cancel)
                .map_err(|error| io::Error::other(error.to_string()))?;
            if self.rescan_required.load(Ordering::Acquire) {
                return Err(io::Error::other(
                    "Filesystem changed too quickly during recovery scan; retry refresh",
                ));
            }
        }
        Ok(())
    }

    // serializate file_map to reduce memory usage
    fn serialization_write(&mut self) -> Result<(), io::Error> {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();
        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::serialization_write", self.drive);

        self.file_map
            .save(&self.index_file_path().to_string_lossy())?;
        self.saved_item_count = self.file_map.len();

        #[cfg(debug_assertions)]
        log::info!(
            "{} End Volume::serialization_write, use time: {:?} ms",
            self.drive,
            sys_time.elapsed().unwrap_or_default().as_millis()
        );

        Ok(())
    }

    // deserializate file_map from file
    #[cfg(test)]
    fn serialization_read(&mut self) -> Result<(), Box<dyn Error>> {
        self.serialization_read_with_cancel(None)
    }

    fn serialization_read_with_cancel(
        &mut self,
        cancel: Option<&AtomicBool>,
    ) -> Result<(), Box<dyn Error>> {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();
        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::serialization_read", self.drive);

        self.file_map
            .read_with_cancel(&self.index_file_path().to_string_lossy(), cancel)?;
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
        self.cache_path.clone()
    }

    fn is_ignored_event_path(&self, path: &std::path::Path, action: FileAction) -> bool {
        if has_hidden_component(path) {
            return true;
        }

        #[cfg(target_os = "macos")]
        if has_named_component(path, &["cache", "caches"]) {
            return true;
        }

        match action {
            FileAction::Insert => {
                self.excluded_dirs.is_excluded_parent_path(path)
                    || (path.is_dir() && self.excluded_dirs.is_excluded_path(path))
            }
            FileAction::Remove => self.excluded_dirs.is_excluded_parent_path(path),
        }
    }
}

fn enqueue_event(
    sender: &mpsc::SyncSender<notify::Result<Event>>,
    rescan: &AtomicBool,
    excluded: &ExcludedDirs,
    result: notify::Result<Event>,
) {
    let Ok(mut event) = result else {
        rescan.store(true, Ordering::Release);
        return;
    };
    if event.need_rescan() || event.paths.len() > MAX_EVENT_PATHS {
        rescan.store(true, Ordering::Release);
        return;
    }
    if !matches!(
        event.kind,
        EventKind::Create(_)
            | EventKind::Remove(_)
            | EventKind::Modify(notify::event::ModifyKind::Name(_))
    ) {
        return;
    }
    event.paths.retain(|path| {
        !has_hidden_component(path)
            && !excluded.is_excluded_path(path)
            && !has_named_component(path, &["cache", "caches"])
    });
    if !event.paths.is_empty() && sender.try_send(Ok(event)).is_err() {
        rescan.store(true, Ordering::Release);
    }
}

fn is_ignored_walk_entry(entry: &DirEntry, excluded_dirs: &ExcludedDirs) -> bool {
    let Some(file_name) = entry.file_name().to_str().map(|name| name.to_lowercase()) else {
        return false;
    };

    if file_name.starts_with('.') {
        return true;
    }

    #[cfg(target_os = "macos")]
    {
        let ignore_names = ["cache", "caches"];
        if ignore_names.contains(&file_name.as_str()) {
            return true;
        }
    }

    if entry.file_type().is_dir() {
        return excluded_dirs.is_excluded_path(entry.path());
    }

    false
}

fn has_hidden_component(path: &std::path::Path) -> bool {
    path.components().any(|component| match component {
        std::path::Component::Normal(segment) => segment
            .to_str()
            .is_some_and(|segment| segment.starts_with('.')),
        _ => false,
    })
}

fn has_named_component(path: &std::path::Path, names: &[&str]) -> bool {
    path.components().any(|component| match component {
        std::path::Component::Normal(segment) => segment
            .to_str()
            .map(|segment| segment.to_lowercase())
            .is_some_and(|segment| names.contains(&segment.as_str())),
        _ => false,
    })
}

#[cfg(test)]
mod event_tests {
    use super::*;

    #[test]
    fn portable_backend_reloads_pages_and_cancels_without_retaining_index() {
        let temp = tempfile::Builder::new()
            .prefix("rotor-portable-")
            .tempdir()
            .unwrap();
        let root = temp.path().join("files");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("report.txt"), b"fixture").unwrap();
        let mut volume = Volume::new(root.to_string_lossy().into_owned());
        volume.cache_path = temp.path().join("cache.idx");
        let cancel = Arc::new(AtomicBool::new(false));
        volume.initialize_index(&cancel).unwrap();
        assert!(volume.index_status().indexed);
        volume.serialization_read().unwrap();
        let page = volume
            .find("report".into(), None, 20, cancel.clone())
            .unwrap();
        assert_eq!(page.items.len(), 1);
        volume.release_index().unwrap();
        assert!(volume.file_map.is_empty());
        cancel.store(true, Ordering::Release);
        assert!(volume.update_index_with_cancel(Some(&cancel)).is_err());
        assert!(volume.file_map.is_empty());
        volume.stop_watching();
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_child_preserves_searchable_siblings_but_root_failure_is_fatal() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::Builder::new()
            .prefix("rotor-permissions-")
            .tempdir()
            .unwrap();
        let root = temp.path().canonicalize().unwrap();
        let blocked = root.join("a-blocked");
        fs::create_dir(&blocked).unwrap();
        fs::write(blocked.join("private.txt"), b"test").unwrap();
        fs::write(root.join("z-searchable.txt"), b"test").unwrap();
        let permissions = fs::metadata(&blocked).unwrap().permissions();
        fs::set_permissions(&blocked, fs::Permissions::from_mode(0o000)).unwrap();
        let denied = fs::read_dir(&blocked).is_err();
        let mut volume = Volume::new(root.to_string_lossy().into_owned());
        let result = volume.build_index();
        fs::set_permissions(&blocked, permissions).unwrap();
        assert!(denied, "test requires an unprivileged user");
        result.unwrap();
        volume.serialization_read().unwrap();
        let items = volume
            .file_map
            .search("z-searchable", None, 1, &AtomicBool::new(false));
        assert_eq!(items.unwrap().items.len(), 1);
        let items = volume
            .file_map
            .search("private.txt", None, 1, &AtomicBool::new(false));
        assert!(items.is_none_or(|page| page.items.is_empty()));
        volume.stop_watching();
        fs::remove_file(volume.index_file_path()).unwrap();

        let permissions = fs::metadata(&root).unwrap().permissions();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();
        let result = volume.build_index();
        fs::set_permissions(&root, permissions).unwrap();
        assert!(result.is_err());
        assert!(volume.file_map.is_empty());
    }

    #[test]
    fn missing_volume_refresh_returns_identity_and_stays_failed() {
        let temp = tempfile::Builder::new()
            .prefix("rotor-missing-")
            .tempdir()
            .unwrap();
        let drive = temp.path().join("missing").to_string_lossy().into_owned();
        let mut volume = Volume::new(drive.clone());
        for _ in 0..2 {
            let error = volume.update_index().unwrap_err();
            assert!(error.to_string().contains(&drive));
            assert!(volume.file_map.is_empty());
        }
    }

    #[test]
    fn watcher_queue_is_bounded_and_overflow_requests_rescan() {
        let (tx, rx) = mpsc::sync_channel(2);
        let rescan = AtomicBool::new(false);
        let excluded = ExcludedDirs::default();
        for _ in 0..100 {
            enqueue_event(
                &tx,
                &rescan,
                &excluded,
                Ok(
                    Event::new(EventKind::Create(notify::event::CreateKind::File))
                        .add_path("/tmp/.hidden/file".into()),
                ),
            );
        }
        assert_eq!(rx.try_iter().count(), 0);
        assert!(!rescan.load(Ordering::Acquire));
        for _ in 0..100 {
            enqueue_event(
                &tx,
                &rescan,
                &excluded,
                Ok(
                    Event::new(EventKind::Create(notify::event::CreateKind::File))
                        .add_path("/tmp/visible".into()),
                ),
            );
        }
        assert!(rescan.load(Ordering::Acquire));
        assert_eq!(rx.try_iter().count(), 2);
    }

    #[test]
    fn directory_events_reconcile_descendants_and_exclusion_boundaries() {
        // Avoid hidden tempfile parents, which are deliberately excluded by the watcher.
        let temp = tempfile::Builder::new()
            .prefix("rotor-events-")
            .tempdir()
            .unwrap();
        let root = temp.path().canonicalize().unwrap();
        let a = root.join("A");
        let b = root.join("B");
        fs::create_dir_all(a.join("nested")).unwrap();
        fs::write(a.join("nested/report.pdf"), b"test").unwrap();
        let mut volume = Volume::new(root.to_string_lossy().into_owned());
        volume.process_path(&a, FileAction::Insert);
        assert_eq!(volume.file_map.len(), 3);
        fs::rename(&a, &b).unwrap();
        volume.handle_event(
            Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
                .add_path(a)
                .add_path(b.clone()),
        );
        let items = volume
            .file_map
            .search("report", None, 1, &AtomicBool::new(false));
        assert!(items.unwrap().items[0].path.contains("B"));
        volume.excluded_dirs =
            crate::file_data::excluded_dirs::parse_excluded_dirs("excluded", None);
        let hidden = root.join("excluded");
        fs::rename(&b, &hidden).unwrap();
        volume.handle_event(
            Event::new(EventKind::Modify(ModifyKind::Name(RenameMode::Both)))
                .add_path(b.clone())
                .add_path(hidden.clone()),
        );
        assert_eq!(volume.file_map.len(), 0);
        fs::rename(&hidden, &b).unwrap();
        volume.process_path(&b, FileAction::Insert);
        assert_eq!(volume.file_map.len(), 3);
        fs::remove_dir_all(&b).unwrap();
        volume.process_path(&b, FileAction::Remove);
        assert_eq!(volume.file_map.len(), 0);
    }
}
