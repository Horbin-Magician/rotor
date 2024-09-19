use std::env;
use std::io;

use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use is_root::is_root;
use winreg::enums::*;
use winreg::RegKey;

pub fn run_as_admin() -> bool {
    if is_root() { return false; }
    let file_path: Vec<u16> = env::current_exe().unwrap().to_str().unwrap().encode_utf16().chain(std::iter::once(0)).collect();
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
    return !ins.is_invalid(); // return true if programe run success
}

pub fn set_power_boot(if_power_boot: bool) -> io::Result<()> {
    let programe_key = "Rotor";
    let file_path = env::current_exe()?;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let key = hkcu.open_subkey_with_flags("Software\\Microsoft\\Windows\\CurrentVersion\\Run", KEY_ALL_ACCESS)?;

    if if_power_boot {
        key.set_value(programe_key, &file_path.to_str().unwrap())?;
    } else {
        key.delete_value(programe_key)?;
    }
    
    Ok(())
}