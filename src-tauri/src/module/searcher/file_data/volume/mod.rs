pub mod default_file_map;
pub mod default_volume;

#[cfg(target_os = "windows")]
pub mod ntfs_file_map;
#[cfg(target_os = "windows")]
pub mod ntfs_volume;
