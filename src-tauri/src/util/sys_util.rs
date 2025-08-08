use xcap;

#[cfg(target_os = "windows")]
mod win_imports {
//     use crate::util::log_util;
//     pub use i_slint_backend_winit::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
//     pub use i_slint_backend_winit::WinitWindowAccessor;
//     pub use is_root::is_root;
//     use std::env;
//     use std::error::Error;
//     pub use windows::core::PCWSTR;
//     pub use windows::Win32::Globalization::GetUserDefaultLocaleName;
    pub use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED,
    };
    pub use windows::Win32::Foundation::HWND;
//     pub use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
//     pub use windows::Win32::UI::Shell::ShellExecuteW;
//     pub use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
//     pub use windows::Win32::{
//         Foundation::{HWND, POINT, RECT},
//         Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS},
//         UI::WindowsAndMessaging::{
//             ChildWindowFromPointEx, GetDesktopWindow, CWP_SKIPDISABLED, CWP_SKIPINVISIBLE,
//             CWP_SKIPTRANSPARENT,
//         },
//     };
//     pub use windows::Win32::{
//         Graphics::Gdi::HMONITOR,
//         UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
//     };
//     pub use winreg::enums::*;
//     pub use winreg::RegKey;
}
#[cfg(target_os = "windows")]
use win_imports::*;

// #[cfg(target_os = "windows")]
// pub fn run_as_admin() -> Result<bool, Box<dyn Error>> {
//     if is_root() {
//         return Ok(false);
//     }
//     let file_path: Vec<u16> = env::current_exe()?
//         .to_str()
//         .ok_or("Failed to_str")?
//         .encode_utf16()
//         .chain(std::iter::once(0))
//         .collect();
//     let runas_str: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();
//     let ins = unsafe {
//         // TODO use the method of file_util
//         ShellExecuteW(
//             HWND(std::ptr::null_mut()),
//             PCWSTR(runas_str.as_ptr()),
//             PCWSTR(file_path.as_ptr()),
//             PCWSTR::null(),
//             PCWSTR::null(),
//             SW_SHOWNORMAL,
//         )
//     };
//     Ok(!ins.is_invalid()) // return true if programe run success
// }

// #[cfg(target_os = "windows")] // TODO: enable for macOS
// pub fn get_user_default_locale_name() -> String {
//     const LOCALE_NAME_MAX_LENGTH: usize = 85;
//     let mut buffer = vec![0u16; LOCALE_NAME_MAX_LENGTH];
//     let length = unsafe { GetUserDefaultLocaleName(&mut buffer) };

//     let locale_name = if length > 0 {
//         String::from_utf16_lossy(&buffer[..(length as usize - 1)])
//     } else {
//         String::new()
//     };

//     return locale_name;
// }

// #[cfg(target_os = "windows")]
// pub fn enable_window(window: &slint::Window, enable: bool) {
//     window.with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
//         if let Ok(handle) = winit_win.window_handle() {
//             if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
//                 unsafe {
//                     let _ = EnableWindow(HWND(win32_handle.hwnd.get() as *mut _), enable);
//                 }
//             }
//         }
//     });
// }

pub fn get_all_window_rect() -> Result<Vec<(i32, i32, i32, u32, u32)>, Box<dyn std::error::Error>> {
    let mut res = Vec::new();

    let windows = xcap::Window::all()?;
    for window in windows {
        let (x, y, width, height) = (window.x()?, window.y()?, window.width()?, window.height()?);
        let z = window.z()?;
        res.push((x, y, z, width, height));
    }

    Ok(res)
}

#[cfg(target_os = "windows")]
pub fn forbid_window_animation(handle: HWND) {
    let disable: i32 = 1;
    unsafe {
        DwmSetWindowAttribute(
            handle,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            &disable as *const _ as *const _,
            std::mem::size_of_val(&disable) as u32,
        )
        .unwrap_or_else(|e| {
            log::error!("DwmSetWindowAttribute error: {:?}", e)
        });
    }
}

// use directories::ProjectDirs;
// #[cfg(windows)]
// use std::ffi::CString;

// extern crate chrono;
// use chrono::Local;

// use log::LevelFilter;
// use log4rs::append::console::ConsoleAppender;
// use log4rs::append::file::FileAppender;
// use log4rs::config::{Appender, Config, Logger, Root};
// use log4rs::encode::pattern::PatternEncoder;

// pub fn subs(str: &str) -> Vec<String> {
//   if let Ok(paths) = std::fs::read_dir(str) {
//     return paths
//       .into_iter()
//       .map(|x| x.unwrap().path().to_str().unwrap().to_string())
//       .collect();
//   }
//   vec![]
// }
// pub fn open_file_path(path: &str) {
//   let curr_path = std::path::Path::new(path);
//   let arg;
//   if curr_path.is_dir() {
//     arg = curr_path.to_str().unwrap();
//   } else {
//     arg = curr_path.parent().unwrap().to_str().unwrap();
//   }

//   if cfg!(target_os = "windows") {
//     std::process::Command::new("explorer")
//       .args([win_norm4explorer(arg)])
//       .output()
//       .expect("failed to execute process");
//   } else if cfg!(target_os = "linux") {
//     std::process::Command::new("xdg-open")
//       .args([arg])
//       .output()
//       .expect("failed to execute process");
//   } else {
//     //mac os
//     std::process::Command::new("open")
//       .args([arg])
//       .output()
//       .expect("failed to execute process");
//   }
// }

// pub fn open_file_path_in_terminal(path: &str) {
//   let curr_path = std::path::Path::new(path);
//   let arg;
//   if curr_path.is_dir() {
//     arg = curr_path.to_str().unwrap();
//   } else {
//     arg = curr_path.parent().unwrap().to_str().unwrap();
//   }

//   if cfg!(target_os = "windows") {
//     //cmd /K "cd C:\Windows\"
//     std::process::Command::new("cmd")
//       .args([
//         "/c",
//         "start",
//         "cmd",
//         "/K",
//         "pushd",
//         &format!("{}", win_norm4explorer(arg)),
//       ])
//       .output()
//       .expect("failed to execute process");
//   } else if cfg!(target_os = "linux") {
//     // gnome-terminal -e "bash -c command;bash"
//     std::process::Command::new("gnome-terminal")
//       .args(["-e", &format!("bash -c 'cd {}';bash", arg)])
//       .output()
//       .expect("failed to execute process");
//   } else {
//     //mac os
//     //open -a Terminal "/Library"
//     std::process::Command::new("open")
//       .args(["-a", "Terminal", arg])
//       .output()
//       .expect("failed to execute process");
//   }
// }

// pub fn data_dir() -> String {
//   let project_dir = ProjectDirs::from("com", "github", "Orange").unwrap();
//   project_dir.data_dir().to_str().unwrap().to_string()
// }

// pub fn path2name(x: String) -> String {
//   norm(&x)
//     .as_str()
//     .split("/")
//     .into_iter()
//     .last()
//     .map(|x| x.to_string())
//     .unwrap_or("".to_string())
// }
// pub fn file_ext(file_name: &str) -> &str {
//   if !file_name.contains(".") {
//     return "";
//   }
//   file_name.split(".").last().unwrap_or("")
// }

// pub fn norm(path: &str) -> String {
//   str::replace(path, "\\", "/")
// }

// pub fn today() -> String {
//   let date = Local::now();
//   date.format("%Y-%m-%d").to_string()
// }

// pub fn win_norm4explorer(path: &str) -> String {
//   str::replace(path, "/", "\\")
// }
// #[cfg(windows)]
// pub unsafe fn get_win32_ready_drives() -> Vec<String> {
//   let mut logical_drives = Vec::with_capacity(5);
//   let mut bitfield = kernel32::GetLogicalDrives();
//   let mut drive = 'A';

//   while bitfield != 0 {
//     if bitfield & 1 == 1 {
//       let strfulldl = drive.to_string() + ":/";
//       let cstrfulldl = CString::new(strfulldl.clone()).unwrap();
//       let x = kernel32::GetDriveTypeA(cstrfulldl.as_ptr());
//       if x == 3 || x == 2 {
//         logical_drives.push(strfulldl);
//         // println!("drive {0} is {1}", strfdl, x);
//       }
//     }
//     drive = std::char::from_u32((drive as u32) + 1).unwrap();
//     bitfield >>= 1;
//   }
//   logical_drives.sort_by(|x1, x2| x2.cmp(x1));
//   logical_drives
// }

// pub fn is_ascii_alphanumeric(raw: &str) -> bool {
//   raw.chars().all(|x| x.is_ascii())
// }

// #[cfg(windows)]
// pub unsafe fn get_win32_ready_drive_nos() -> Vec<String> {
//   let vec = get_win32_ready_drives();
//   let mut res = vec![];
//   for x in vec {
//     let s = str::replace(x.as_str(), ":/", "");
//     res.push(s);
//   }
//   res.sort();
//   res
// }
// pub fn win_norm4exclude_path(x: String) -> String {
//   let (x1, x2) = x.split_at(1);
//   let mut up = x1.to_uppercase();
//   up.push_str(x2);
//   up.replace("//", "/")
// }

// #[cfg(windows)]
// pub unsafe fn build_volume_path(str: &str) -> String {
//   str::replace("\\\\?\\$:", "$", str)
// }
