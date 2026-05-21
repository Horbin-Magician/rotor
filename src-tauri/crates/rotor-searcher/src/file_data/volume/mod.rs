pub mod default_file_map;
pub mod default_volume;

#[cfg(target_os = "windows")]
pub mod ntfs_file_map;
#[cfg(target_os = "windows")]
pub mod ntfs_volume;

#[derive(serde::Serialize)]
pub struct SearchResultItem {
    pub path: String,
    pub file_name: String,
    pub rank: i8,
    pub icon_data: Option<String>, // Base64 encoded icon data
    pub alias: Option<String>,
}

impl Clone for SearchResultItem {
    fn clone(&self) -> Self {
        SearchResultItem {
            path: self.path.clone(),
            file_name: self.file_name.clone(),
            rank: self.rank,
            icon_data: self.icon_data.clone(),
            alias: self.alias.clone(),
        }
    }
}
