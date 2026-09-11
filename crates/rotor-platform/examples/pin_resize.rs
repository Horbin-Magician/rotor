//! Native layer/geometry regression: cargo run -p rotor-platform --example pin_resize --locked
//! Uses an unshown synthetic panel; no profile, capture or user data is accessed.
#[cfg(target_os = "macos")]
fn main() {
    use objc2_app_kit::{
        NSApplication, NSBackingStoreType, NSPanel, NSViewLayerContentsPlacement as Placement,
        NSWindowStyleMask,
    };
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};
    use raw_window_handle::{AppKitWindowHandle, RawWindowHandle, WindowHandle};
    use rotor_platform::overlay::set_client_bounds;

    let main = MainThreadMarker::new().expect("Run on the main thread");
    let app = NSApplication::sharedApplication(main);
    unsafe { app.finishLaunching() };
    let panel = unsafe {
        NSPanel::initWithContentRect_styleMask_backing_defer(
            main.alloc(),
            NSRect::new(NSPoint::new(100., 100.), NSSize::new(200., 200.)),
            NSWindowStyleMask::Titled
                | NSWindowStyleMask::FullSizeContentView
                | NSWindowStyleMask::NonactivatingPanel,
            NSBackingStoreType::NSBackingStoreBuffered,
            false,
        )
    };
    unsafe { panel.setReleasedWhenClosed(false) };
    let view = panel.contentView().unwrap();
    view.setWantsLayer(true);
    let raw = AppKitWindowHandle::new(std::ptr::NonNull::from(&*view).cast());
    let handle = unsafe { WindowHandle::borrow_raw(RawWindowHandle::AppKit(raw)) };
    let scale = panel.backingScaleFactor() as f32;
    let primary_height = core_graphics::display::CGDisplay::main()
        .bounds()
        .size
        .height;
    set_client_bounds(handle, 100, 100, 300, 300, scale).unwrap();
    for (x, y, width, height, placement) in [
        (100, 100, 280, 280, Placement::TopLeft),
        (120, 100, 260, 280, Placement::TopRight),
        (120, 120, 260, 260, Placement::BottomLeft),
        (100, 100, 280, 280, Placement::BottomRight),
    ] {
        set_client_bounds(handle, x, y, width, height, scale).unwrap();
        let client = panel.convertRectToScreen(view.convertRect_toView(view.bounds(), None));
        let actual = (
            (client.origin.x * scale as f64).round() as i32,
            ((primary_height - client.origin.y - client.size.height) * scale as f64).round() as i32,
            (client.size.width * scale as f64).round() as u32,
            (client.size.height * scale as f64).round() as u32,
        );
        assert_eq!(actual, (x, y, width, height));
        unsafe {
            assert_eq!(view.layerContentsPlacement(), placement);
            let layer = view.layer().unwrap();
            assert_ne!(layer.contentsGravity().to_string(), "resize");
            assert_eq!(layer.contentsScale(), panel.backingScaleFactor());
        }
    }
    println!("Native pin resize geometry, unscaled layer placement and backing scale passed");
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("This native check requires macOS");
}
