use std::{
    fs::OpenOptions, fs,
};
use std::io::{self, Write};
use chrono::Local;

use windows::core::PCWSTR;
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_OK};
use windows::Win32::Foundation::HWND;

use crate::util::file_util;


fn wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn write_log(message: &String) -> std::io::Result<()> {
    let file_path = file_util::get_userdata_path();
    if !file_path.exists() { fs::create_dir(&file_path)?; }
    let log_path = file_path.join("log");

    // Open the log file
    let mut file = OpenOptions::new()
        .append(true) // set it to append mode
        .create(true) // create it if the file does not exist
        .open(log_path)?;

    writeln!(file, "{}", message)?;
    Ok(())
}

fn log_error_without_log(content: &str) {
    let title = wide_null("Error");
    let content = wide_null(content);
    unsafe {
        MessageBoxW(HWND(std::ptr::null_mut()), PCWSTR(content.as_ptr()), PCWSTR(title.as_ptr()), MB_OK);
    }
}

pub fn log_error(message: String)  {
    let now_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let error_content = format!("{now_time} [Error] {message}");
    write_log(&error_content).unwrap_or_else(|err: io::Error| {
        log_error_without_log(&format!("{:?}", err));
    });
    log_error_without_log(&error_content);
}

#[allow(dead_code)]
pub fn log_info(message: String)  {
    let now_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let error_content = format!("{now_time} [info] {message}");
    write_log(&error_content).unwrap_or_else(|err: io::Error| {
        log_error_without_log(&format!("{:?}", err));
    });
    println!("{}", error_content);
}