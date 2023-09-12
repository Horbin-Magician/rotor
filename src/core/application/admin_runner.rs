use std::env;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::Foundation::HMODULE;
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use is_root::is_root;
pub struct AdminRunner;

impl AdminRunner {
    // run a programe as admin
    // return: true, if run successfully
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
}