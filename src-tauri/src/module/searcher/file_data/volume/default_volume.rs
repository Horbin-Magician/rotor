use std::{fs, io};
use std::sync::mpsc;
use std::error::Error;
use std::time::SystemTime;
use walkdir::WalkDir;

use crate::util::file_util;
use super::default_file_map::{FileMap, SearchResultItem};

pub struct Volume {
    pub drive: String,
    file_map: FileMap,
    stop_receiver: mpsc::Receiver<()>,
    last_query: String,
    last_search_num: usize,
}

impl Volume {
    pub fn new(drive: String, stop_receiver: mpsc::Receiver<()>) -> Volume {
        Volume {
            drive,
            file_map: FileMap::new(),
            stop_receiver,
            last_query: String::new(),
            last_search_num: 0,
        }
    }

    // Enumerate the filesystem using walkdir. Store the file entries in the database.
    pub fn build_index(&mut self) {
        let sys_time = SystemTime::now();
        log::info!("{} Begin Volume::build_index", self.drive);

        self.release_index();

        // Build the root path based on the drive letter
        let root_path = if cfg!(target_os = "windows") {
            format!("{}:\\", self.drive)
        } else {
            // For Unix-like systems, use the drive char as a mount point identifier
            format!("{}", self.drive)
        };

        // Check if the root path exists
        if !std::path::Path::new(&root_path).exists() {
            log::error!("{} Root path {} does not exist, skipping index build", self.drive, root_path);
            return;
        }

        // Walk the directory tree using walkdir
        let walker = WalkDir::new(&root_path)
            .follow_links(false) // don't follow symbolic links to avoid infinite loops
            .into_iter()
            .filter_map(|e| e.ok()); // skit no permission

        for entry in walker {
            // Check if we should stop (user requested cancellation)
            if self.stop_receiver.try_recv().is_ok() {
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

        log::info!("{} End Volume::build_index, use time: {:?} ms", self.drive, sys_time.elapsed().unwrap_or_default().as_millis());

        self.serialization_write()
            .unwrap_or_else(|e| log::error!("{} Volume::serialization_write, error: {:?}", self.drive, e));
    }

    // Clears the database
    pub fn release_index(&mut self) {
        if self.file_map.is_empty() {return;}

        self.last_query = String::new();
        self.last_search_num = 0;

        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::release_index", self.drive);

        self.file_map.clear();
    }

    // searching
    pub fn find(&mut self, query: String, batch: u8, sender: mpsc::Sender<Option<Vec<SearchResultItem>>>) {
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
            self.serialization_read()
                .unwrap_or_else(|e| {
                    log::error!("{} Volume::serialization_write, error: {:?}", self.drive, e);
                    self.build_index();
                });
        };

        while self.stop_receiver.try_recv().is_ok() { } // clear channel before find
        let (result, search_num) = self.file_map.search(&query, self.last_search_num, batch, &self.stop_receiver);

        #[cfg(debug_assertions)]
        log::info!("{} End Volume::Find {query}, use time: {:?} ms", self.drive, sys_time.elapsed().unwrap_or_default().as_millis());
        
        self.last_search_num += search_num;

        let _ = sender.send(result);
    }

    // update index, add new file, remove deleted file TODO
    pub fn update_index(&mut self) {
        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::update_index", self.drive);

        // TODO

        #[cfg(debug_assertions)]
        log::info!("{} End Volume::update_index", self.drive);
    }

    // serializate file_map to reduce memory usage
    fn serialization_write(&mut self) -> Result<(), io::Error> {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();
        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::serialization_write", self.drive);

        if self.file_map.is_empty() {return Ok(())};
        
        let file_path = file_util::get_userdata_path();
        if let Some(file_path) = file_path {
            if !file_path.exists() { fs::create_dir(&file_path)?; }
            let safe_drive = self.drive[1..].replace("/", "_");
            let file_name = format!("{}/{}.fd", file_path.to_str().unwrap_or("."), safe_drive);
            self.file_map.save(&file_name)?;
        }

        self.release_index();

        #[cfg(debug_assertions)]
        log::info!("{} End Volume::serialization_write, use time: {:?} ms", self.drive, sys_time.elapsed().unwrap_or_default().as_millis());

        Ok(())
    }

    // deserializate file_map from file
    fn serialization_read(&mut self) -> Result<(), Box<dyn Error>> {
        #[cfg(debug_assertions)]
        let sys_time = SystemTime::now();
        #[cfg(debug_assertions)]
        log::info!("{} Begin Volume::serialization_read", self.drive);
        
        let file_path = file_util::get_userdata_path();
        if let Some(file_path) = file_path {
            let safe_drive = self.drive[1..].replace("/", "_");
            let file_name = format!("{}/{}.fd", file_path.to_str().unwrap_or("."), safe_drive);
            self.file_map.read(&file_name)?;
        }

        #[cfg(debug_assertions)]
        log::info!("{} End Volume::serialization_read, use time: {:?} ms", self.drive, sys_time.elapsed().unwrap_or_default().as_millis());

        Ok(())
    }
}
