use base64::prelude::*;
use file_icon_provider::get_file_icon;
use image::{DynamicImage, ImageFormat, RgbaImage};
use std::error::Error;
use std::io::Cursor;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::{env, fs};

pub fn del_useless_files() -> Result<(), Box<dyn std::error::Error>> {
    let userdata_path = get_userdata_path().unwrap();
    for entry in fs::read_dir(userdata_path)? {
        let entry = entry?;
        let entry_path = entry.path();
        if entry_path.is_file() {
            if let Some(ext) = entry_path.extension() {
                if ext == "fd" { // TODO del this
                    fs::remove_file(&entry_path)?;
                }
            }
        }
    }
    Ok(())
}

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
        std::process::Command::new("explorer.exe")
            .arg(file_path)
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&file_path).spawn()?;
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
                &format!("Start-Process -FilePath '{}' -Verb RunAs", file_path),
            ])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()?;
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        open_file(file_path)?;
        Err("MacOS does not support, use normal open instead".into())
    }
}

// Get file icon as base64 encoded PNG data
pub fn get_file_icon_data(file_path: &str) -> Option<String> {
    let path = Path::new(file_path);
    if !path.exists() {
        return None;
    }

    // Get icon with 64x64 size
    match get_file_icon(path, 64) {
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
