pub fn open_url(value: &str) -> Result<(), String> {
    let url = url::Url::parse(value).map_err(|error| error.to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Only HTTP and HTTPS links can be opened".into());
    }
    #[cfg(target_os = "windows")]
    {
        use windows::{
            core::{w, PCWSTR},
            Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        };
        let value: Vec<u16> = url.as_str().encode_utf16().chain(Some(0)).collect();
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(value.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize <= 32 {
            return Err(format!(
                "Cannot open link (Windows code {})",
                result.0 as isize
            ));
        }
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url.as_str())
            .spawn()
            .map_err(|error| error.to_string())?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        return Err("Opening links is unsupported".into());
    }
    Ok(())
}

pub fn configure_background_application() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let main = objc2_foundation::MainThreadMarker::new()
            .ok_or("Application policy must be changed on the main thread")?;
        let application = objc2_app_kit::NSApplication::sharedApplication(main);
        if !application.setActivationPolicy(objc2_app_kit::NSApplicationActivationPolicy::Accessory)
        {
            return Err("Could not hide the Dock icon".into());
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn is_elevated() -> bool {
    is_root::is_root()
}

#[cfg(target_os = "windows")]
pub fn launch_elevated(executable: &std::path::Path, arguments: &[String]) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        core::{w, PCWSTR},
        Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    };
    let path: Vec<u16> = executable
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let parameters = arguments
        .iter()
        .map(|argument| crate::startup::quote_argument(argument))
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    let parameters: Vec<u16> = parameters.encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            w!("runas"),
            PCWSTR(path.as_ptr()),
            PCWSTR(parameters.as_ptr()),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize <= 32 {
        return Err(format!(
            "Elevation was cancelled or failed (Windows code {})",
            result.0 as isize
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn external_link_handler_rejects_non_web_schemes_before_launch() {
        assert!(super::open_url("javascript:alert(1)").is_err());
        assert!(super::open_url("file:///C:/Windows/system.ini").is_err());
    }
}
