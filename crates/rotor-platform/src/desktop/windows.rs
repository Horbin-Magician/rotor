use windows::{
    core::{w, PCWSTR},
    Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
};

pub(super) fn open_url(url: &url::Url) -> Result<(), String> {
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
    Ok(())
}

/// The Dock is a macOS concept; Windows taskbar presence follows the window styles.
pub(super) fn set_dock_visible(_visible: bool) -> Result<(), String> {
    Ok(())
}

pub(super) fn show_startup_error(message: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND,
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

pub(super) fn suppress_error_dialogs() {
    use windows::Win32::System::Diagnostics::Debug::{
        SetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX,
    };
    unsafe {
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
    }
}

pub(super) fn is_elevated() -> bool {
    is_root::is_root()
}

pub(super) fn launch_elevated(
    executable: &std::path::Path,
    arguments: &[String],
) -> Result<(), String> {
    let parameters = arguments
        .iter()
        .map(|argument| crate::startup::quote_argument(argument))
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    launch_elevated_parameters(executable, &parameters)
}

pub(super) fn launch_update_installer(
    executable: &std::path::Path,
    profile: &std::path::Path,
    flags: &[String],
) -> Result<(), String> {
    let parameters = update_installer_parameters(profile, std::process::id(), flags)?;
    launch_elevated_parameters(executable, &parameters)
}

pub(super) fn rollback_failed_install(
    profile: &std::path::Path,
    flags: &[String],
) -> Result<(), String> {
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
    let registry = rotor_common::native_app::IDENTIFIER;
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

fn launch_elevated_parameters(
    executable: &std::path::Path,
    parameters: &str,
) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
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
}
