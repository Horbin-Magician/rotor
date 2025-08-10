use std::{env, fs};
use file_icon_provider::get_file_icon;
use image::{DynamicImage, RgbaImage, ImageFormat};
use std::io::Cursor;
use std::path::Path;
use base64::prelude::*;

// #[cfg(target_os = "windows")]
// mod win_imports {
//     pub use crate::util::log_util;
//     pub use std::error::Error;
//     pub use std::ffi::c_void;
//     pub use std::process::Command;
//     pub use std::{io, mem, ptr};
//     pub use windows::core::PCWSTR;
//     pub use windows::Win32::Foundation::{BOOL, HWND};
//     pub use windows::Win32::Graphics::Gdi::{
//         self, DeleteObject, GetBitmapBits, BITMAP, BITMAPINFOHEADER, HBITMAP, HGDIOBJ,
//     };
//     pub use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
//     pub use windows::Win32::UI::Shell::{SHGetFileInfoW, ShellExecuteW, SHFILEINFOW, SHGFI_ICON};
//     pub use windows::Win32::UI::WindowsAndMessaging::{
//         DestroyIcon, GetIconInfo, HICON, ICONINFO, SW_SHOWNORMAL,
//     };
// }
// #[cfg(target_os = "windows")]
// use win_imports::*;

#[allow(dead_code)]
pub fn file_exists(path: &str) -> bool {
    fs::metadata(path).is_ok()
}

#[allow(dead_code)]
pub fn get_app_path() -> std::path::PathBuf {
    if let Ok(exe_path) = env::current_exe() {
        let result = exe_path.parent().unwrap_or(std::path::Path::new("."));
        result.to_path_buf()
    } else {
        std::path::Path::new(".").to_path_buf()
    }
}

#[allow(dead_code)]
pub fn get_tmp_path() -> std::path::PathBuf {
    env::temp_dir()
}

pub fn get_userdata_path() -> Option<std::path::PathBuf> {
    if let Some(home_path) = env::home_dir() {
        return Some(home_path.join(".rotor"));
    }
    None
}

// #[cfg(target_os = "windows")]
// pub fn open_file(file_full_name: String) -> Result<(), Box<dyn Error>> {
//     Command::new("explorer.exe").arg(file_full_name).spawn()?;
//     Ok(())
// }

// #[cfg(target_os = "windows")]
// pub fn open_file_admin(file_full_name: String) {
//     let file_path: Vec<u16> = file_full_name
//         .as_str()
//         .encode_utf16()
//         .chain(std::iter::once(0))
//         .collect();
//     let runas_str: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();
//     unsafe {
//         ShellExecuteW(
//             HWND(std::ptr::null_mut()),
//             PCWSTR(runas_str.as_ptr()),
//             PCWSTR(file_path.as_ptr()),
//             PCWSTR::null(),
//             PCWSTR::null(),
//             SW_SHOWNORMAL,
//         )
//     };
// }

// Get file icon as base64 encoded PNG data
pub fn get_file_icon_data(file_path: &str) -> Option<String> {
    let path = Path::new(file_path);
    if !path.exists() {
        return None;
    }
    
    // Get icon with 32x32 size
    match get_file_icon(path.to_path_buf(), 32) {
        Ok(icon) => {
            // Convert Icon to Image
            match RgbaImage::from_raw(icon.width, icon.height, icon.pixels) {
                Some(img) => {
                    let dynamic_img = DynamicImage::ImageRgba8(img);
                    
                    // Convert to PNG bytes
                    let mut png_bytes = Vec::new();
                    let mut cursor = Cursor::new(&mut png_bytes);
                    
                    match dynamic_img.write_to(&mut cursor, ImageFormat::Png) {
                        Ok(()) => {
                            // Encode as base64
                            Some(BASE64_STANDARD.encode(&png_bytes))
                        }
                        Err(_) => None,
                    }
                }
                None => None,
            }
        }
        Err(_) => None,
    }
}
