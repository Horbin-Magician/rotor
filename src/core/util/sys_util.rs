use std::env;
use std::io;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::Foundation::HMODULE;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use is_root::is_root;
use winreg::enums::*;
use winreg::RegKey;

pub fn run_as_admin() -> bool {
    if is_root() { return false; }
    let file_path: Vec<u16> = env::current_exe().unwrap().to_str().unwrap().encode_utf16().chain(std::iter::once(0)).collect();
    let runas_str: Vec<u16> = "runas".encode_utf16().chain(std::iter::once(0)).collect();
    let ins:HMODULE = unsafe {
        ShellExecuteW(
            0,
            runas_str.as_ptr(),
            file_path.as_ptr(),
            runas_str.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL
        )
    };
    if ins > 32 as HMODULE { return true; } // return true if programe run successfully
    // TODO: MessageBoxW(NULL, L"该软件需要在管理员权限下建立索引。", L"请以管理员身份运行",MB_OK);
    false
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