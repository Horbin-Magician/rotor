use base64::prelude::*;
use file_icon_provider::get_file_icon;
use image::{DynamicImage, ImageFormat, RgbaImage};
use std::collections::HashMap;
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

pub fn get_app_trans_names(app_path: &Path) -> Result<HashMap<String, String>, Box<dyn Error>> {
    let contents_path = app_path.join("Contents");

    // Traverse *.lproj under Resources
    let resources_path = contents_path.join("Resources");
    let mut translations: HashMap<String, String> = HashMap::new();

    for entry in fs::read_dir(&resources_path)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        if file_name_str.ends_with(".lproj") {
            let lang = file_name_str.trim_end_matches(".lproj");
            let strings_path = entry.path().join("InfoPlist.strings");
            if strings_path.exists() {
                if let Ok(s) = fs::read(&strings_path) {
                    // 旧版 .strings 是 UTF-16, 转成 UTF-8
                    let (cow, _encoding, _had_errors) =
                        encoding_rs::UTF_16LE.decode(&s);
                    let content = cow.to_string();

                    // 解析 key="value" 对（简单实现）
                    for line in content.lines() {
                        let line = line.trim();
                        if line.starts_with("CFBundleDisplayName") || line.starts_with("\"CFBundleDisplayName\"")
                        {
                            if let Some(idx) = line.find('=') {
                                let val = line[idx+1..].trim().trim_matches(';').trim();
                                let val = val.trim_matches('"');
                                translations.insert(lang.to_string(), val.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(translations)
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
