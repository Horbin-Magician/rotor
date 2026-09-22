use core_graphics::display::CGDisplay;
use objc2_app_kit::{
    NSFloatingWindowLevel, NSScreenSaverWindowLevel, NSView, NSViewLayerContentsPlacement,
    NSWindowAnimationBehavior, NSWindowButton, NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};
use raw_window_handle::{RawWindowHandle, WindowHandle};

/// The borrowed handle refers to a live NSView on the owning UI thread.
fn view(handle: WindowHandle<'_>) -> Result<&NSView, String> {
    let RawWindowHandle::AppKit(raw) = handle.as_raw() else {
        return Err("Expected an AppKit window".into());
    };
    Ok(unsafe { &*raw.ns_view.as_ptr().cast::<NSView>() })
}

pub(super) fn screen_cursor_position(scale: f32) -> Option<(f64, f64)> {
    use core_graphics::{
        event::CGEvent,
        event_source::{CGEventSource, CGEventSourceStateID},
    };
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let cursor = CGEvent::new(source).ok()?.location();
    // Quartz uses global logical points; retain fractional points on Retina.
    Some((cursor.x * scale as f64, cursor.y * scale as f64))
}

pub(super) fn configure_text_entry_panel(handle: WindowHandle<'_>) -> Result<(), String> {
    let window = view(handle)?
        .window()
        .ok_or("View is not attached to a window")?;
    // GPUI gives PopUp panels level 101, which can cover input-method windows.
    // Retain the panel's style and all-Spaces/fullscreen collection behavior.
    window.setLevel(NSFloatingWindowLevel);
    Ok(())
}

/// Taskbar entries are a Windows concept; the Dock tracks AppKit windows itself.
pub(super) fn enable_pin_taskbar(_handle: WindowHandle<'_>) -> Result<(), String> {
    Ok(())
}

pub(super) fn enable_pin_minimization(handle: WindowHandle<'_>) -> Result<(), String> {
    // Called on the owning UI thread before the pin is first shown. GPUI
    // ignores is_minimizable when titlebar is None; retain all other flags.
    let window = view(handle)?
        .window()
        .ok_or("View is not attached to a window")?;
    window.setStyleMask(window.styleMask() | NSWindowStyleMask::Miniaturizable);
    // AppKit can recreate the traffic lights when the style mask changes.
    // Hide them afterwards; the pin toolbar owns its window controls.
    for kind in [
        NSWindowButton::NSWindowCloseButton,
        NSWindowButton::NSWindowMiniaturizeButton,
        NSWindowButton::NSWindowZoomButton,
    ] {
        if let Some(button) = window.standardWindowButton(kind) {
            button.setHidden(true);
        }
    }
    Ok(())
}

pub(super) fn window_minimized(handle: WindowHandle<'_>) -> Option<bool> {
    let window = view(handle).ok()?.window()?;
    // Miniaturized windows are not visible. Only an unshown, non-miniaturized
    // window should retain its saved state during initialization.
    let minimized = window.isMiniaturized();
    (minimized || window.isVisible()).then_some(minimized)
}

/// DWM transitions do not exist on macOS; pins use AppKit's default behavior.
pub(super) fn disable_window_animation(_handle: WindowHandle<'_>) -> Result<(), String> {
    Ok(())
}

/// Corner preferences are a DWM attribute; AppKit windows are not rounded by the OS.
pub(super) fn disable_window_rounding(_handle: WindowHandle<'_>) -> Result<(), String> {
    Ok(())
}

/// Corner preferences are a DWM attribute; AppKit windows are not rounded by the OS.
pub(super) fn use_small_window_corners(_handle: WindowHandle<'_>) -> Result<(), String> {
    Ok(())
}

pub(super) fn settle_desktop() -> Result<(), String> {
    Ok(())
}

/// Submit a hidden capture window's backing layer before ordering it onscreen.
/// Call on the main thread outside UI framework borrows: the layer's display
/// delegate re-enters the renderer. Keep the window alive until this returns.
pub(super) fn paint_hidden_window(handle: RawWindowHandle) -> Result<(), String> {
    let RawWindowHandle::AppKit(raw) = handle else {
        return Err("Expected an AppKit window".into());
    };
    let _main_thread =
        MainThreadMarker::new().ok_or("Hidden paint must run on the owning UI thread")?;
    // The caller obtained this handle from a live window on this thread.
    let view = unsafe { &*raw.ns_view.as_ptr().cast::<NSView>() };
    let _window = view.window().ok_or("View is not attached to a window")?;
    let layer = unsafe { view.layer() }.ok_or("Capture window has no backing layer")?;
    // Hidden views are not serviced by ordinary window invalidation. CALayer's
    // display calls GPUI's displayLayer: delegate synchronously, which renders
    // and presents Metal content with the current Core Animation transaction.
    // Merely refreshing and then orderFront would expose the placeholder frame.
    layer.display();
    Ok(())
}

/// Pointer capture is a Win32 concept; AppKit tracks drags on the receiving view.
pub(super) fn pointer_capture(_handle: WindowHandle<'_>, _capture: bool) -> Result<(), String> {
    Ok(())
}

pub(super) fn set_client_bounds(
    handle: WindowHandle<'_>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f32,
) -> Result<(), String> {
    let view = view(handle)?;
    let window = view.window().ok_or("View is not attached to a window")?;
    let client = window.convertRectToScreen(view.convertRect_toView(view.bounds(), None));
    let mut frame = window.frame();
    let primary_height = CGDisplay::main().bounds().size.height;
    let width = width as f64 / scale as f64;
    let height = height as f64 / scale as f64;
    // Until GPUI submits the matching drawable, AppKit otherwise stretches
    // the previous Metal frame to the new bounds. Preserve its pixel scale
    // and anchor it to the crop's stationary edges instead. NSView maps the
    // placement to layer gravity using the view's coordinate orientation.
    let (right, bottom) = super::resize_content_anchor(
        (
            client.origin.x * scale as f64,
            (primary_height - client.origin.y - client.size.height) * scale as f64,
        ),
        (x, y),
    );
    use NSViewLayerContentsPlacement as Placement;
    let placement = match (right, bottom) {
        (false, false) => Placement::TopLeft,
        (true, false) => Placement::TopRight,
        (false, true) => Placement::BottomLeft,
        (true, true) => Placement::BottomRight,
    };
    // The handle borrows a live view on its owning UI thread.
    unsafe {
        view.setLayerContentsPlacement(placement);
        if let Some(layer) = view.layer() {
            layer.setContentsScale(window.backingScaleFactor());
        }
    }
    frame.origin.x += x as f64 / scale as f64 - client.origin.x;
    frame.origin.y += primary_height - y as f64 / scale as f64 - height - client.origin.y;
    frame.size.width += width - client.size.width;
    frame.size.height += height - client.size.height;
    // Do not synchronously display the old scene while PinView/Window are
    // borrowed. The caller synchronizes GPUI bounds and invalidates its
    // updated crop before the next frame is drawn.
    window.setFrame_display(frame, false);
    Ok(())
}

pub(super) fn fit_capture_screen(handle: WindowHandle<'_>, display_id: u32) -> Result<(), String> {
    let view = view(handle)?;
    let bounds = CGDisplay::new(display_id).bounds();
    if bounds.size.width <= 0. || bounds.size.height <= 0. {
        return Err("Captured display is no longer available".into());
    }
    let window = view.window().ok_or("View is not attached to a window")?;
    // A capture mask must replace the desktop in one step, without AppKit's
    // popup fade changing the apparent brightness of the frozen screenshot.
    unsafe { window.setAnimationBehavior(NSWindowAnimationBehavior::None) };
    // GPUI's titlebar: None still uses Titled | FullSizeContentView on macOS.
    // Remove those flags so AppKit does not constrain the mask below the menu
    // bar. Preserve the popup's nonactivating panel behavior.
    window.setStyleMask(
        window.styleMask() & !(NSWindowStyleMask::Titled | NSWindowStyleMask::FullSizeContentView),
    );
    window.setLevel(NSScreenSaverWindowLevel);
    // Quartz display bounds are global logical points with a top-left origin;
    // AppKit uses a bottom-left origin relative to the primary display.
    let primary_height = CGDisplay::main().bounds().size.height;
    window.setFrame_display(
        NSRect::new(
            NSPoint::new(
                bounds.origin.x,
                primary_height - bounds.origin.y - bounds.size.height,
            ),
            NSSize::new(bounds.size.width, bounds.size.height),
        ),
        true,
    );
    Ok(())
}

pub(super) fn hide_window(handle: WindowHandle<'_>) -> Result<(), String> {
    // The borrowed AppKit handle owns a live NSView on this UI thread.
    view(handle)?
        .window()
        .ok_or("View is not attached to a window")?
        .orderOut(None);
    Ok(())
}

pub(super) fn show_window(handle: WindowHandle<'_>) -> Result<(), String> {
    view(handle)?
        .window()
        .ok_or("View is not attached to a window")?
        .orderFront(None);
    Ok(())
}
