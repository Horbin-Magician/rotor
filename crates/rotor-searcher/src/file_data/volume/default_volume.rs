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
use super::{index_file_stem, metadata_modified_at, SearchResultItem, VolumeIndexStatus};
use rotor_platform::file_util;

#[derive(Debug, Clone, Copy)]
enum FileAction {
    Insert,
    Remove,
}

pub struct Volume {
    pub drive: String,
    file_map: FileMap,
    last_query: String,
    last_search_num: usize,
    watcher: Option<RecommendedWatcher>,
    event_receiver: Option<mpsc::Receiver<notify::Result<Event>>>,
    saved_item_count: usize,
    excluded_dirs: ExcludedDirs,
}

impl Volume {
    pub fn new(drive: String) -> Volume {
        Volume {
            drive,
            file_map: FileMap::new(),
            last_query: String::new(),
            last_search_num: 0,
            watcher: None,
            event_receiver: None,
            saved_item_count: 0,
            excluded_dirs: ExcludedDirs::from_config(),
        }
    }

    pub fn start_watching(&mut self) -> Result<(), Box<dyn Error>> {
        if self.watcher.is_some() {
            return Ok(());
        }

        let root_path = if cfg!(target_os = "windows") {
            format!("{}:\\", self.drive)
        } else {
            self.drive.to_string()
        };

        if !std::path::Path::new(&root_path).exists() {
            return Err(format!("Root path {} does not exist", root_path).into());
        }

        let (tx, rx) = mpsc::channel();
        let config = Config::default().with_poll_interval(std::time::Duration::from_secs(2));
        let mut watcher = notify::recommended_watcher(move |res| {
            if let Err(e) = tx.send(res) {
                log::error!("Failed to send file event: {:?}", e);
            }
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

    fn process_path(&mut self, path: &std::path::Path, action: FileAction) {
        if self.is_ignored_event_path(path, action) {
            return;
        }

        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
            if let Some(parent) = path.parent() {
                let parent_path = parent.to_string_lossy().to_string();
                let file_name = file_name.to_string();

                match action {
                    FileAction::Insert => self.file_map.insert(file_name, parent_path),
                    FileAction::Remove => self.file_map.remove(file_name, parent_path),
                }
            }
        }
    }

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

    fn handle_file_events(&mut self) {
        let Some(receiver) = self.event_receiver.take() else {
            return;
        };

        while let Ok(event_result) = receiver.try_recv() {
            match event_result {
                Ok(event) => {
                    self.handle_event(event);
                }
                Err(e) => {
                    log::error!("{} File watcher error: {:?}", self.drive, e);
                }
            }
        }

        self.event_receiver = Some(receiver);
    }

    // Enumerate the filesystem using walkdir. Store the file entries in the database.
    pub fn build_index(&mut self) {
        self.build_index_with_cancel(None);
    }

    fn build_index_with_cancel(&mut self, cancel: Option<&AtomicBool>) {
        let sys_time = SystemTime::now();

        self.excluded_dirs = ExcludedDirs::from_config();
        self.stop_watching();
        self.release_index_without_save();

        // Build the root path based on the drive letter
        let root_path = if cfg!(target_os = "windows") {
            format!("{}:\\", self.drive)
        } else {
            self.drive.to_string()
        };

        // Check if the root path exists
        if !std::path::Path::new(&root_path).exists() {
            log::error!(
                "{} Root path {} does not exist, skipping index build",
                self.drive,
                root_path
            );
            return;
        }

        // Walk the directory tree using walkdir
        let walkdir = WalkDir::new(&root_path).follow_links(false); // don't follow symbolic links to avoid infinite loops
        let excluded_dirs = self.excluded_dirs.clone();

        let walker = walkdir
            .into_iter()
            .filter_entry(move |e| !is_ignored_walk_entry(e, &excluded_dirs))
            .filter_map(|e| e.ok()); // skit no permission

        for entry in walker {
            if cancel
                .map(|cancel| cancel.load(Ordering::Relaxed))
                .unwrap_or(false)
            {
                log::info!("{} Volume::build_index cancelled by user", self.drive);
                return;
            }

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

        log::info!(
            "{} End Volume::build_index, use time: {:?} ms",
            self.drive,
            sys_time.elapsed().unwrap_or_default().as_millis()
        );

        if let Err(e) = self.start_watching() {
            log::error!("{} Failed to start file watching: {:?}", self.drive, e);
        }

        self.release_index();
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
                log::error!("{} Volume::serialization_read, error: {:?}", self.drive, e);
                self.build_index_with_cancel(Some(&cancel));
            });
        };

        let (result, search_num) =
            self.file_map
                .search(&query, self.last_search_num, batch, &cancel);

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

    // Clears the database
    pub fn release_index(&mut self) {
        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::release_index", self.drive);

        if self.file_map.is_empty() {
            return;
        }

        self.serialization_write().unwrap_or_else(|e| {
            log::error!("{} Volume::serialization_write, error: {:?}", self.drive, e)
        });

        self.release_index_without_save();
    }

    pub fn release_index_without_save(&mut self) {
        self.last_query = String::new();
        self.last_search_num = 0;
        self.file_map.clear();
    }

    // update index, add new file, remove deleted file
    pub fn update_index(&mut self) {
        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::update_index", self.drive);

        if self.file_map.is_empty() {
            self.serialization_read().unwrap_or_else(|e| {
                log::error!("{} Volume::serialization_read, error: {:?}", self.drive, e);
                self.build_index();
            });
        };

        self.handle_file_events();

        #[cfg(debug_assertions)]
        log::info!("{} End Volume::update_index", self.drive);
    }

    // serializate file_map to reduce memory usage
    fn serialization_write(&mut self) -> Result<(), io::Error> {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();
        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::serialization_write", self.drive);

        if self.file_map.is_empty() {
            return Ok(());
        }

        let index_dir = file_util::get_tmp_path();
        fs::create_dir_all(index_dir)?;
        self.saved_item_count = self.file_map.len();
        self.file_map
            .save(&self.index_file_path().to_string_lossy())?;

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

#[cfg(target_os = "macos")]
fn has_named_component(path: &std::path::Path, names: &[&str]) -> bool {
    path.components().any(|component| match component {
        std::path::Component::Normal(segment) => segment
            .to_str()
            .map(|segment| segment.to_lowercase())
            .is_some_and(|segment| names.contains(&segment.as_str())),
        _ => false,
    })
}
