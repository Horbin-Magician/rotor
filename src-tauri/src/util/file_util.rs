use std::error::Error;
use std::{env, fs};
use file_icon_provider::get_file_icon;
use image::{DynamicImage, RgbaImage, ImageFormat};
use std::io::Cursor;
use std::path::Path;
use base64::prelude::*;

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

pub fn open_file(file_path: String) -> Result<(), Box<dyn Error>> {
    let path = Path::new(&file_path);
    if !path.exists() {
        return Err(format!("File does not exist: {}", file_path).into());
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &file_path])
            .spawn()?;
        // Command::new("explorer.exe").arg(file_full_name).spawn()?; // Old use
    }
    
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&file_path)
            .spawn()?;
    }
    
    Ok(())
}

pub fn open_file_as_admin(file_path: String) -> Result<(), Box<dyn Error>> {
    #[cfg(target_os = "windows")]
    {
        let path = Path::new(&file_path);
        if !path.exists() {
            return Err(format!("File does not exist: {}", file_path).into());
        }

        std::process::Command::new("powershell")
            .args([
                "-Command",
                &format!("Start-Process -FilePath '{}' -Verb RunAs", file_path)
            ])
            .spawn()?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        open_file(file_path)?;
        return Err(format!("MacOS does not support, use normal open instead").into());
    }
}

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
