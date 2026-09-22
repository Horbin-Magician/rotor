//! Desktop shell integration: link handling, Dock/application policy, elevation
//! and startup diagnostics.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
use macos as native;
#[cfg(target_os = "windows")]
use windows as native;

pub fn open_url(value: &str) -> Result<(), String> {
    let url = url::Url::parse(value).map_err(|error| error.to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Only HTTP and HTTPS links can be opened".into());
    }
    native::open_url(&url)
}

/// Sets the running application's icon, including launches outside an app bundle.
#[cfg(target_os = "macos")]
pub fn set_application_icon(bytes: &[u8]) -> Result<(), String> {
    native::set_application_icon(bytes)
}

pub fn configure_background_application() -> Result<(), String> {
    set_dock_visible(false)
}

/// Changes the macOS application policy; call from the desktop main thread.
pub fn set_dock_visible(visible: bool) -> Result<(), String> {
    native::set_dock_visible(visible)
}

pub fn show_startup_error(message: &str) {
    native::show_startup_error(message)
}

/// Keep the OS from blocking this process in a modal error dialog when a
/// corrupt executable or missing file is encountered. Intended for the
/// unattended recovery launcher, which reports failures itself.
#[cfg(target_os = "windows")]
pub fn suppress_error_dialogs() {
    native::suppress_error_dialogs()
}

#[cfg(target_os = "windows")]
pub fn is_elevated() -> bool {
    native::is_elevated()
}

#[cfg(target_os = "windows")]
pub fn launch_elevated(executable: &std::path::Path, arguments: &[String]) -> Result<(), String> {
    native::launch_elevated(executable, arguments)
}

#[cfg(target_os = "windows")]
pub fn launch_update_installer(
    executable: &std::path::Path,
    profile: &std::path::Path,
    flags: &[String],
) -> Result<(), String> {
    native::launch_update_installer(executable, profile, flags)
}

#[cfg(target_os = "windows")]
pub fn rollback_failed_install(profile: &std::path::Path, flags: &[String]) -> Result<(), String> {
    native::rollback_failed_install(profile, flags)
}

#[cfg(test)]
mod tests {
    #[test]
    fn external_link_handler_rejects_non_web_schemes_before_launch() {
        assert!(super::open_url("javascript:alert(1)").is_err());
        assert!(super::open_url("file:///C:/Windows/system.ini").is_err());
    }
}
