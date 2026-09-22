use std::error::Error;
use std::os::windows::process::CommandExt;

pub(super) fn open_file(file_path: &str) -> Result<(), Box<dyn Error>> {
    let trimmed_path = file_path.trim_end_matches(&['/', '\\'][..]);
    std::process::Command::new("explorer.exe")
        .arg(trimmed_path)
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .spawn()?;
    Ok(())
}

pub(super) fn open_file_as_admin(file_path: &str) -> Result<(), Box<dyn Error>> {
    use windows::core::{w, PCWSTR};
    use windows::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    if file_path.contains('\0') {
        return Err("File path contains a null character".into());
    }
    let path: Vec<u16> = file_path.encode_utf16().chain(Some(0)).collect();
    // Pass the path as data, never as a PowerShell program. Apostrophes and
    // other valid filename characters must not become command syntax.
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("runas"),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        return Err(format!(
            "Windows could not open the file as administrator (code {})",
            result.0 as isize
        )
        .into());
    }
    Ok(())
}
