use std::env;
use std::error::Error;
use i_slint_backend_winit::WinitWindowAccessor;
use i_slint_backend_winit::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED};
use windows::Win32::{Graphics::Gdi::HMONITOR, UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI}};
use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use is_root::is_root;
use winreg::enums::*;
use winreg::RegKey;

use crate::util::log_util;


pub fn run_as_admin() -> Result<bool, Box<dyn Error>> {
    if is_root() { return Ok(false); }
    let file_path: Vec<u16> = env::current_exe()?.to_str().ok_or("Failed to_str")?.encode_utf16().chain(std::iter::once(0)).collect();
    let runas_str: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();
    let ins = unsafe { // TODO use the method of file_util
        ShellExecuteW(
            HWND(std::ptr::null_mut()),
            PCWSTR(runas_str.as_ptr()),
            PCWSTR(file_path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL
        )
    };
    Ok(!ins.is_invalid()) // return true if programe run success
}

pub fn set_power_boot(if_power_boot: bool) -> Result<(), Box<dyn Error>> {
    let programe_key = "Rotor";
    let file_path = env::current_exe()?;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let key = hkcu.open_subkey_with_flags("Software\\Microsoft\\Windows\\CurrentVersion\\Run", KEY_ALL_ACCESS)?;

    if if_power_boot {
        key.set_value(programe_key, &file_path.to_str().ok_or("Failed to_str")?)?;
    } else {
        key.delete_value(programe_key)?;
    }
    
    Ok(())
}

pub fn forbid_window_animation(window: &slint::Window) {
    window.with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
        if let Ok(handle) = winit_win.window_handle(){
            if let RawWindowHandle::Win32(win32_handle) = handle.as_raw() {
                let disable: i32 = 1;
                unsafe {
                    DwmSetWindowAttribute(
                        HWND(win32_handle.hwnd.get() as *mut _),
                        DWMWA_TRANSITIONS_FORCEDISABLED,
                        &disable as *const _ as *const _,
                        std::mem::size_of_val(&disable) as u32,
                    ).unwrap_or_else(|e| log_util::log_error(format!("DwmSetWindowAttribute error: {:?}", e)));
                }
            }
        }
    });
}

pub fn get_scale_factor(monitor_id: u32) -> f32 {
    let mut dpi_x: u32 = 0;
    let mut dpi_y: u32 = 0;
    unsafe{ 
        GetDpiForMonitor(HMONITOR(monitor_id as *mut _), MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y)
            .unwrap_or_else(|e| log_util::log_error(format!("GetDpiForMonitor error: {:?}", e)));
    }
    dpi_x as f32 / 96.0
}