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

/// Sets the running application's icon, including launches outside an app bundle.
#[cfg(target_os = "macos")]
pub fn set_application_icon(bytes: &[u8]) -> Result<(), String> {
    let main = objc2_foundation::MainThreadMarker::new()
        .ok_or("Application icon must be changed on the main thread")?;
    let data = objc2_foundation::NSData::with_bytes(bytes);
    let image = objc2_app_kit::NSImage::initWithData(main.alloc(), &data)
        .ok_or("Could not decode the application icon")?;
    let application = objc2_app_kit::NSApplication::sharedApplication(main);
    // SAFETY: AppKit is accessed on the main thread with a valid, retained image.
    unsafe { application.setApplicationIconImage(Some(&image)) };
    Ok(())
}

pub fn configure_background_application() -> Result<(), String> {
    set_dock_visible(false)
}

/// Changes the macOS application policy; call from the desktop main thread.
pub fn set_dock_visible(visible: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let main = objc2_foundation::MainThreadMarker::new()
            .ok_or("Application policy must be changed on the main thread")?;
        let application = objc2_app_kit::NSApplication::sharedApplication(main);
        let policy = if visible {
            objc2_app_kit::NSApplicationActivationPolicy::Regular
        } else {
            objc2_app_kit::NSApplicationActivationPolicy::Accessory
        };
        if !application.setActivationPolicy(policy) {
            return Err("Could not change Dock icon visibility".into());
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = visible;
    Ok(())
}

pub fn show_startup_error(message: &str) {
    #[cfg(target_os = "windows")]
    {
        use windows::{
            core::PCWSTR,
            Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND},
        };
        let title: Vec<u16> = rotor_common::native_app::PRODUCT_NAME
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let message: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
        unsafe {
            MessageBoxW(
                None,
                PCWSTR(message.as_ptr()),
                PCWSTR(title.as_ptr()),
                MB_OK | MB_ICONERROR | MB_SETFOREGROUND,
            );
        }
    }
    #[cfg(target_os = "macos")]
    if let Some(main) = objc2_foundation::MainThreadMarker::new() {
        objc2_app_kit::NSApplication::sharedApplication(main);
        unsafe {
            let alert = objc2_app_kit::NSAlert::new(main);
            alert.setMessageText(&objc2_foundation::NSString::from_str(
                rotor_common::native_app::PRODUCT_NAME,
            ));
            alert.setInformativeText(&objc2_foundation::NSString::from_str(message));
            alert.setAlertStyle(objc2_app_kit::NSAlertStyle::Critical);
            alert.runModal();
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    eprintln!("Rotor: {message}");
}

#[cfg(target_os = "windows")]
pub fn is_elevated() -> bool {
    is_root::is_root()
}

#[cfg(target_os = "windows")]
pub fn launch_elevated(executable: &std::path::Path, arguments: &[String]) -> Result<(), String> {
    let parameters = arguments
        .iter()
        .map(|argument| crate::startup::quote_argument(argument))
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    launch_elevated_parameters(executable, &parameters)
}

#[cfg(target_os = "windows")]
pub fn launch_update_installer(
    executable: &std::path::Path,
    profile: &std::path::Path,
    flags: &[String],
) -> Result<(), String> {
    let parameters = update_installer_parameters(profile, std::process::id(), flags)?;
    launch_elevated_parameters(executable, &parameters)
}

#[cfg(target_os = "windows")]
pub fn rollback_failed_install(profile: &std::path::Path, flags: &[String]) -> Result<(), String> {
    use winreg::{
        enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY},
        RegKey,
    };
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let current = executable
        .parent()
        .ok_or("Executable has no installation directory")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let registry = if rotor_common::native_app::PRODUCTION {
        "Rotor"
    } else {
        "RotorGpuiDevelopment"
    };
    let key = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(format!("Software\\{registry}"), KEY_READ | KEY_WOW64_64KEY)
        .map_err(|e| e.to_string())?;
    let installed: String = key.get_value("InstallDir").map_err(|e| e.to_string())?;
    let previous: String = key
        .get_value("PreviousInstallLocation")
        .map_err(|e| e.to_string())?;
    let previous = std::path::PathBuf::from(previous)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if std::path::Path::new(&installed)
        .canonicalize()
        .map_err(|e| e.to_string())?
        != current
        || previous == current
        || previous.parent() != current.parent()
        || !previous
            .join(format!("{}.exe", rotor_common::native_app::EXECUTABLE_NAME))
            .is_file()
    {
        return Err("No matching previous installation is available for rollback".into());
    }
    let uninstaller = current.join("uninstall.exe");
    if !uninstaller.is_file() {
        return Err("Rollback helper is missing".into());
    }
    let arguments = update_installer_parameters(profile, std::process::id(), flags)?.replacen(
        "/UPDATE",
        "/S /ROLLBACK",
        1,
    );
    launch_elevated_parameters(&uninstaller, &arguments)
}

#[cfg(target_os = "windows")]
fn update_installer_parameters(
    profile: &std::path::Path,
    parent: u32,
    flags: &[String],
) -> Result<String, String> {
    let profile = profile.to_str().ok_or("Profile path is not Unicode")?;
    if parent == 0 || profile.contains(['\0', '"']) {
        return Err("Invalid installer arguments".into());
    }
    // NSIS GetOptions scans raw parameters and ignores switches inside quotes.
    // This is deliberately distinct from CommandLineToArgvW argument quoting.
    let mut parameters = format!("/UPDATE /PARENT={parent} /PROFILE=\"{profile}\"");
    for flag in flags {
        let switch = match flag.as_str() {
            "--no-elevate" => " /NOELEVATE",
            "--no-index" => " /NOINDEX",
            "--no-hotkeys" => " /NOHOTKEYS",
            "--production-shortcuts" => " /PRODUCTIONSHORTCUTS",
            "--background" => " /BACKGROUND",
            _ => "",
        };
        parameters.push_str(switch);
    }
    Ok(parameters)
}

#[cfg(target_os = "windows")]
fn launch_elevated_parameters(
    executable: &std::path::Path,
    parameters: &str,
) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        core::{w, PCWSTR},
        Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
    };
    let path: Vec<u16> = dunce::simplified(executable)
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
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
    #[cfg(target_os = "windows")]
    #[test]
    fn installer_switches_remain_outside_quotes() {
        let value = super::update_installer_parameters(
            std::path::Path::new(r"C:\Profiles\李 & test"),
            42,
            &["--no-elevate".into(), "--no-index".into()],
        )
        .unwrap();
        assert_eq!(
            value,
            "/UPDATE /PARENT=42 /PROFILE=\"C:\\Profiles\\李 & test\" /NOELEVATE /NOINDEX"
        );
        assert!(
            super::update_installer_parameters(std::path::Path::new("bad\"path"), 42, &[]).is_err()
        );
        assert!(
            super::update_installer_parameters(std::path::Path::new("C:\\profile"), 0, &[])
                .is_err()
        );
    }

    #[test]
    fn external_link_handler_rejects_non_web_schemes_before_launch() {
        assert!(super::open_url("javascript:alert(1)").is_err());
        assert!(super::open_url("file:///C:/Windows/system.ini").is_err());
    }
}
