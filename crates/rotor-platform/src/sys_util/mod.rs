//! Process, volume, cursor and window-list queries used by search and capture.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
use macos as native;
#[cfg(target_os = "windows")]
use windows as native;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryUsage {
    /// Private working set on Windows; total resident memory on macOS.
    pub resident_bytes: u64,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionStatus {
    pub key: String,
    pub name: String,
    pub granted: Option<bool>,
    pub detail: String,
}

// Check whether the disk represented by a drive letter is in ntfs format
#[cfg(target_os = "windows")]
pub fn is_ntfs(vol: char) -> bool {
    native::is_ntfs(vol)
}

pub type WindowRect = (i32, i32, i32, u32, u32);

/// Visible top-level windows of other processes, front to back, as
/// `(x, y, z, width, height)`. Larger `z` is closer to the front.
pub fn get_all_window_rect() -> Result<Vec<WindowRect>, Box<dyn std::error::Error>> {
    native::get_all_window_rect()
}

pub fn get_cursor_position() -> Result<(i32, i32), Box<dyn std::error::Error>> {
    native::get_cursor_position()
}

pub fn get_memory_usage() -> Result<MemoryUsage, Box<dyn std::error::Error>> {
    native::get_memory_usage()
}

pub fn get_permission_statuses() -> Vec<PermissionStatus> {
    native::get_permission_statuses()
}
