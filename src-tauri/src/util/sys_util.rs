#[cfg(target_os = "windows")]
mod win_imports {
    use crate::util::log_util;
    pub use i_slint_backend_winit::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    pub use i_slint_backend_winit::WinitWindowAccessor;
    pub use is_root::is_root;
    use std::env;
    use std::error::Error;
    pub use windows::core::PCWSTR;
    pub use windows::Win32::Globalization::GetUserDefaultLocaleName;
    pub use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED,
    };
    pub use windows::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
    pub use windows::Win32::UI::Shell::ShellExecuteW;
    pub use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    pub use windows::Win32::{
        Foundation::{HWND, POINT, RECT},
        Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS},
        UI::WindowsAndMessaging::{
            ChildWindowFromPointEx, GetDesktopWindow, CWP_SKIPDISABLED, CWP_SKIPINVISIBLE,
            CWP_SKIPTRANSPARENT,
        },
    };
    pub use windows::Win32::{
        Graphics::Gdi::HMONITOR,
        UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
    };
    pub use winreg::enums::*;
    pub use winreg::RegKey;
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

// get point(x, y) return the window rect(x, y, width, height)
#[cfg(target_os = "windows")]
pub fn get_point_window_rect(x: i32, y: i32) -> (i32, i32, i32, i32) {
    let point: POINT = POINT { x, y };
    let mut temp_window = RECT::default();
    unsafe {
        let hwnd: HWND = ChildWindowFromPointEx(
            GetDesktopWindow(),
            point,
            CWP_SKIPDISABLED | CWP_SKIPINVISIBLE | CWP_SKIPTRANSPARENT,
        );
        let _ = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut temp_window as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        );
    }
    return (
        temp_window.left,
        temp_window.top,
        temp_window.right - temp_window.left,
        temp_window.bottom - temp_window.top,
    );
}

#[cfg(target_os = "macos")]
pub fn get_point_window_rect(x: i32, y: i32) -> (i32, i32, i32, i32) {
    use core_graphics::geometry::CGPoint;
    use core_graphics::window::{
        kCGWindowListOptionOnScreenOnly, 
        kCGWindowListExcludeDesktopElements,
        CGWindowListCopyWindowInfo
    };
    use core_foundation::array::CFArray;
    use core_foundation::base::TCFType;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    
    let _point = CGPoint::new(x as f64, y as f64);
    
    unsafe {
        // Get list of all on-screen windows
        let window_list_info = CGWindowListCopyWindowInfo(
            kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
            0
        );
        
        if window_list_info.is_null() {
            return (0, 0, 0, 0);
        }
        
        let windows: CFArray<CFDictionary> = CFArray::wrap_under_create_rule(window_list_info);
        
        // Iterate through windows to find the one containing the point
        for window_info in windows.iter() {
            // Get window bounds using string keys directly
            let bounds_key = CFString::from_static_string("kCGWindowBounds");
            
            // Use the raw CFDictionary API
            let bounds_value = core_foundation::dictionary::CFDictionaryGetValue(
                window_info.as_concrete_TypeRef(),
                bounds_key.as_concrete_TypeRef() as *const _
            );
            
            if !bounds_value.is_null() {
                let bounds_dict: CFDictionary = CFDictionary::wrap_under_get_rule(bounds_value as *const core_foundation::dictionary::__CFDictionary);
                
                // Extract X, Y, Width, Height from bounds dictionary
                let x_key = CFString::from_static_string("X");
                let y_key = CFString::from_static_string("Y");
                let width_key = CFString::from_static_string("Width");
                let height_key = CFString::from_static_string("Height");
                
                let x_value = core_foundation::dictionary::CFDictionaryGetValue(
                    bounds_dict.as_concrete_TypeRef(),
                    x_key.as_concrete_TypeRef() as *const _
                );
                let y_value = core_foundation::dictionary::CFDictionaryGetValue(
                    bounds_dict.as_concrete_TypeRef(),
                    y_key.as_concrete_TypeRef() as *const _
                );
                let width_value = core_foundation::dictionary::CFDictionaryGetValue(
                    bounds_dict.as_concrete_TypeRef(),
                    width_key.as_concrete_TypeRef() as *const _
                );
                let height_value = core_foundation::dictionary::CFDictionaryGetValue(
                    bounds_dict.as_concrete_TypeRef(),
                    height_key.as_concrete_TypeRef() as *const _
                );
                
                if !x_value.is_null() && !y_value.is_null() && !width_value.is_null() && !height_value.is_null() {
                    let x_num = CFNumber::wrap_under_get_rule(x_value as *const _);
                    let y_num = CFNumber::wrap_under_get_rule(y_value as *const _);
                    let width_num = CFNumber::wrap_under_get_rule(width_value as *const _);
                    let height_num = CFNumber::wrap_under_get_rule(height_value as *const _);
                    
                    if let (Some(wx), Some(wy), Some(ww), Some(wh)) = (
                        x_num.to_i32(),
                        y_num.to_i32(),
                        width_num.to_i32(),
                        height_num.to_i32()
                    ) {
                        // Check if point is within window bounds
                        if x >= wx && x < wx + ww && y >= wy && y < wy + wh {
                            return (wx, wy, ww, wh);
                        }
                    }
                }
            }
        }
    }
    
    // Return empty rect if no window found
    (0, 0, 0, 0)
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
