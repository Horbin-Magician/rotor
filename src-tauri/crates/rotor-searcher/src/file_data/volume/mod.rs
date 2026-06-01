pub mod default_file_map;
pub mod default_volume;

use std::fs::Metadata;
use std::time::UNIX_EPOCH;

#[cfg(target_os = "windows")]
pub mod ntfs_file_map;
#[cfg(target_os = "windows")]
pub mod ntfs_volume;

#[derive(serde::Serialize)]
pub struct SearchResultItem {
    pub path: String,
    pub file_path: String,
    pub file_name: String,
    pub rank: i8,
    pub icon_data: Option<String>, // Base64 encoded icon data
    pub alias: Option<String>,
}

impl Clone for SearchResultItem {
    fn clone(&self) -> Self {
        SearchResultItem {
            path: self.path.clone(),
            file_path: self.file_path.clone(),
            file_name: self.file_name.clone(),
            rank: self.rank,
            icon_data: self.icon_data.clone(),
            alias: self.alias.clone(),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeIndexStatus {
    pub name: String,
    pub indexed: bool,
    pub index_item_count: Option<usize>,
    pub index_file_size_bytes: u64,
    pub index_file_modified_at: Option<u64>,
}

pub fn metadata_modified_at(metadata: &Metadata) -> Option<u64> {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
}

pub fn index_file_stem(name: &str) -> String {
    let stem = name
        .trim_start_matches('/')
        .replace('/', "_")
        .replace('\\', "_");

    if stem.is_empty() {
        "root".to_string()
    } else {
        stem
    }
}
