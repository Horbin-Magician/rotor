use chrono::Local;
use std::io::{self, Write};
use std::{fs, fs::OpenOptions};

use crate::util::file_util;

#[cfg(target_os = "windows")]
mod win_imports {
    pub use windows::core::PCWSTR;
    pub use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_OK};
}
#[cfg(target_os = "windows")]
use win_imports::*;

#[cfg(target_os = "macos")]
use std::process::Command;

#[cfg(target_os = "windows")]
fn log_error_with_message_box(content: &str) {
    fn wide_null(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    let title = wide_null("Error");
    let content = wide_null(content);
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(content.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK,
        );
    }
}

#[cfg(target_os = "macos")]
fn log_error_with_message_box(content: &str) {
    // Use osascript to display a native macOS alert dialog
    let script = format!(
        "osascript -e 'display alert \"Error\" message \"{}\" buttons \"OK\" default button \"OK\"'",
        content.replace("\"", "\\\"")
    );

    let _ = Command::new("sh").arg("-c").arg(script).output();
}

fn write_log(message: &str) -> std::io::Result<()> {
    if let Some(userdata_path) = file_util::get_userdata_path() {
        if !userdata_path.exists() {
            fs::create_dir(&userdata_path)?;
        }
        let log_path = userdata_path.join("log");
        // Open the log file
        let mut file = OpenOptions::new()
            .append(true) // set it to append mode
            .create(true) // create it if the file does not exist
            .open(log_path)?;
        writeln!(file, "{}", message)?;
    } else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Can't get userdata path.",
        ));
    }
    Ok(())
}

#[allow(dead_code)]
pub fn log_error(message: String) {
    let now_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let error_content = format!("{now_time} [Error] {message}");
    write_log(&error_content)
        .unwrap_or_else(|e: io::Error| log_error_with_message_box(&format!("{:?}", e)));
}

#[allow(dead_code)]
pub fn log_warn(message: String) {
    let now_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let error_content = format!("{now_time} [warn] {message}");
    write_log(&error_content)
        .unwrap_or_else(|e: io::Error| log_error_with_message_box(&format!("{:?}", e)));
    println!("{}", error_content);
}

#[allow(dead_code)]
pub fn log_info(message: String) {
    let now_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let error_content = format!("{now_time} [info] {message}");
    write_log(&error_content)
        .unwrap_or_else(|e: io::Error| log_error_with_message_box(&format!("{:?}", e)));
    println!("{}", error_content);
}
