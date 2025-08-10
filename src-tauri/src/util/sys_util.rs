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
    pub use windows::Win32::Storage::FileSystem;
    pub use windows::Win32::Foundation::HWND;
    pub use windows::Win32::Foundation;
    use std::ffi::{CStr, CString};
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

// Check whether the disk represented by a drive letter is in ntfs format
#[cfg(target_os = "windows")]
fn is_ntfs(vol: char) -> bool {
    if let Ok(root_path_name) = CString::new(format!("{}:\\", vol)) {
        let mut volume_name_buffer = vec![0u8; Foundation::MAX_PATH as usize];
        let mut volume_serial_number: u32 = 0;
        let mut maximum_component_length: u32 = 0;
        let mut file_system_flags: u32 = 0;
        let mut file_system_name_buffer = vec![0u8; Foundation::MAX_PATH as usize];

        unsafe {
            if FileSystem::GetVolumeInformationA(
                    windows::core::PCSTR(root_path_name.as_ptr() as *const u8),
                    Some(&mut volume_name_buffer),
                    Some(&mut volume_serial_number),
                    Some(&mut maximum_component_length),
                    Some(&mut file_system_flags),
                    Some(&mut file_system_name_buffer),
                ).is_ok() {

                let result = CStr::from_ptr(file_system_name_buffer.as_ptr() as *const i8);
                return result.to_string_lossy() == "NTFS";
            }
        }
    }
    false
}

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
