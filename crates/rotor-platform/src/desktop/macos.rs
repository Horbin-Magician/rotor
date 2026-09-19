use objc2_app_kit::{NSAlert, NSAlertStyle, NSApplication, NSApplicationActivationPolicy, NSImage};
use objc2_foundation::{MainThreadMarker, NSData, NSString};

pub(super) fn open_url(url: &url::Url) -> Result<(), String> {
    std::process::Command::new("open")
        .arg(url.as_str())
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(super) fn set_application_icon(bytes: &[u8]) -> Result<(), String> {
    let main =
        MainThreadMarker::new().ok_or("Application icon must be changed on the main thread")?;
    let data = NSData::with_bytes(bytes);
    let image = NSImage::initWithData(main.alloc(), &data)
        .ok_or("Could not decode the application icon")?;
    let application = NSApplication::sharedApplication(main);
    // SAFETY: AppKit is accessed on the main thread with a valid, retained image.
    unsafe { application.setApplicationIconImage(Some(&image)) };
    Ok(())
}

pub(super) fn set_dock_visible(visible: bool) -> Result<(), String> {
    let main =
        MainThreadMarker::new().ok_or("Application policy must be changed on the main thread")?;
    let application = NSApplication::sharedApplication(main);
    let policy = if visible {
        NSApplicationActivationPolicy::Regular
    } else {
        NSApplicationActivationPolicy::Accessory
    };
    if !application.setActivationPolicy(policy) {
        return Err("Could not change Dock icon visibility".into());
    }
    Ok(())
}

pub(super) fn show_startup_error(message: &str) {
    if let Some(main) = MainThreadMarker::new() {
        NSApplication::sharedApplication(main);
        unsafe {
            let alert = NSAlert::new(main);
            alert.setMessageText(&NSString::from_str(rotor_common::native_app::PRODUCT_NAME));
            alert.setInformativeText(&NSString::from_str(message));
            alert.setAlertStyle(NSAlertStyle::Critical);
            alert.runModal();
        }
    }
}
