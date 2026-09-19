//! File launching, bundle metadata and icon helpers shared by search and the shell.

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
use std::collections::HashMap;
use std::error::Error;
use std::fs;
use std::path::Path;

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
                    let (cow, _encoding, _had_errors) = encoding_rs::UTF_16LE.decode(&s);
                    let content = cow.to_string();

                    // 解析 key="value" 对（简单实现）
                    for line in content.lines() {
                        let line = line.trim();
                        if line.starts_with("CFBundleDisplayName")
                            || line.starts_with("\"CFBundleDisplayName\"")
                        {
                            if let Some(idx) = line.find('=') {
                                let val = line[idx + 1..].trim().trim_matches(';').trim();
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
