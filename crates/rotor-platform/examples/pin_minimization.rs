//! Run on macOS's main thread: cargo run -p rotor-platform --example pin_minimization --locked
//! Uses only a synthetic native panel and does not initialize a Rotor profile.
#[cfg(target_os = "macos")]
fn main() {
    use objc2_app_kit::{NSApplication, NSBackingStoreType, NSPanel, NSWindowStyleMask};
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};
    use raw_window_handle::{AppKitWindowHandle, RawWindowHandle, WindowHandle};
    use rotor_platform::overlay::{enable_pin_minimization, window_minimized};

    let main = MainThreadMarker::new().expect("Run on the main thread");
    let application = NSApplication::sharedApplication(main);
    rotor_platform::desktop::set_dock_visible(true).unwrap();
    unsafe { application.finishLaunching() };
    // Match GPUI's titlebar-less, nonactivating pin panel.
    let style = NSWindowStyleMask::Titled
        | NSWindowStyleMask::FullSizeContentView
        | NSWindowStyleMask::NonactivatingPanel;
    let panel = unsafe {
        NSPanel::initWithContentRect_styleMask_backing_defer(
            main.alloc(),
            NSRect::new(NSPoint::new(100., 100.), NSSize::new(64., 64.)),
            style,
            NSBackingStoreType::NSBackingStoreBuffered,
            false,
        )
    };
    unsafe { panel.setReleasedWhenClosed(false) };
    let view = panel.contentView().expect("Panel content view");
    let raw = AppKitWindowHandle::new(std::ptr::NonNull::from(&*view).cast());
    // The panel and content view stay alive for the entire borrowed handle use.
    let handle = unsafe { WindowHandle::borrow_raw(RawWindowHandle::AppKit(raw)) };
    assert_eq!(window_minimized(handle), None);
    enable_pin_minimization(handle).unwrap();
    assert_eq!(panel.styleMask(), style | NSWindowStyleMask::Miniaturizable);
    enable_pin_minimization(handle).unwrap();
    assert_eq!(panel.styleMask(), style | NSWindowStyleMask::Miniaturizable);
    for kind in [
        objc2_app_kit::NSWindowButton::NSWindowCloseButton,
        objc2_app_kit::NSWindowButton::NSWindowMiniaturizeButton,
        objc2_app_kit::NSWindowButton::NSWindowZoomButton,
    ] {
        if let Some(button) = panel.standardWindowButton(kind) {
            assert!(unsafe { button.isHidden() });
        }
    }
    panel.orderFront(None);
    assert_eq!(window_minimized(handle), Some(false));
    panel.miniaturize(None);
    settle();
    assert_eq!(window_minimized(handle), Some(true));
    unsafe { panel.deminiaturize(None) };
    settle();
    assert_eq!(window_minimized(handle), Some(false));
    panel.orderOut(None);
    assert_eq!(window_minimized(handle), None);
    // Restoring a saved minimized pin starts from an unshown native window.
    panel.miniaturize(None);
    settle();
    assert_eq!(window_minimized(handle), Some(true));
    unsafe { panel.deminiaturize(None) };
    settle();
    assert_eq!(window_minimized(handle), Some(false));
    // Rotor can also run as a tray-only accessory before Settings is opened.
    rotor_platform::desktop::set_dock_visible(false).unwrap();
    panel.orderFront(None);
    panel.miniaturize(None);
    settle();
    assert_eq!(window_minimized(handle), Some(true));
    unsafe { panel.deminiaturize(None) };
    settle();
    assert_eq!(window_minimized(handle), Some(false));
    panel.orderOut(None);
    println!("Synthetic pin minimization and restoration passed");
}

#[cfg(target_os = "macos")]
fn settle() {
    use objc2_foundation::{NSDate, NSRunLoop};
    unsafe {
        NSRunLoop::mainRunLoop().runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(1.));
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This native check requires macOS");
}
