//! File launching, bundle metadata and icon helpers shared by search and the shell.

mod bundle_names;
pub use bundle_names::get_app_trans_names;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
use macos as native;
#[cfg(target_os = "windows")]
use windows as native;

use file_icon_provider::get_file_icon;
use image::RgbaImage;
use std::error::Error;
use std::path::Path;

pub fn open_file(file_path: String) -> Result<(), Box<dyn Error>> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("File does not exist: {}", file_path).into());
    }
    native::open_file(&file_path)
}

pub fn open_file_as_admin(file_path: String) -> Result<(), Box<dyn Error>> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("File does not exist: {}", file_path).into());
    }
    native::open_file_as_admin(&file_path)
}

/// Native RGBA pixels; callers can share them without PNG/Base64 round trips.
pub fn file_icon(file_path: &str) -> Option<RgbaImage> {
    let path = Path::new(file_path);
    if !path.exists() {
        return None;
    }
    let icon = get_file_icon(path, 64).ok()?;
    RgbaImage::from_raw(icon.width, icon.height, icon.pixels)
}
