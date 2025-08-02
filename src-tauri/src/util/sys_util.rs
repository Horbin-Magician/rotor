use xcap;

// #[cfg(target_os = "windows")]
// mod win_imports {
//     use crate::util::log_util;
//     pub use i_slint_backend_winit::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
//     pub use i_slint_backend_winit::WinitWindowAccessor;
//     pub use is_root::is_root;
//     use std::env;
//     use std::error::Error;
//     pub use windows::core::PCWSTR;
//     pub use windows::Win32::Globalization::GetUserDefaultLocaleName;
//     pub use windows::Win32::Graphics::Dwm::{
//         DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED,
//     };
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
// }
// #[cfg(target_os = "windows")]
// use win_imports::*;

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

pub fn get_all_window_rect() -> Result<Vec<(i32, i32, u32, u32)>, Box<dyn std::error::Error>> {
    let mut res = Vec::new();

    let windows = xcap::Window::all()?;
    for window in windows {
        let (x, y, width, height) = (window.x()?, window.y()?, window.width()?, window.height()?);
        res.push((x, y, width, height));
    }

    Ok(res)
}

// #[cfg(target_os = "windows")]
// pub fn forbid_window_animation(window: &slint::Window) {
//     window.with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
//         if let Ok(handle) = winit_win.window_handle() {
//             if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
//                 let disable: i32 = 1;
//                 unsafe {
//                     DwmSetWindowAttribute(
//                         HWND(win32_handle.hwnd.get() as *mut _),
//                         DWMWA_TRANSITIONS_FORCEDISABLED,
//                         &disable as *const _ as *const _,
//                         std::mem::size_of_val(&disable) as u32,
//                     )
//                     .unwrap_or_else(|e| {
//                         log_util::log_error(format!("DwmSetWindowAttribute error: {:?}", e))
//                     });
//                 }
//             }
//         }
//     });
// }
