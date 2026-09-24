//! Native window geometry and lifecycle for capture masks and pins.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
use macos as native;
#[cfg(target_os = "windows")]
use windows as native;

use raw_window_handle::{RawWindowHandle, WindowHandle};

/// Current cursor in the same top-left, physical coordinate space as pin bounds.
/// Read independently of window geometry: drag events can be replayed after a
/// window has moved, making their window-relative coordinates stale.
pub fn screen_cursor_position(scale: f32) -> Option<(f64, f64)> {
    if !scale.is_finite() || scale <= 0. {
        return None;
    }
    native::screen_cursor_position(scale)
}

/// Keep a text-entry panel above ordinary windows without covering IME candidates.
/// macOS only; a no-op on Windows.
pub fn configure_text_entry_panel(handle: WindowHandle<'_>) -> Result<(), String> {
    native::configure_text_entry_panel(handle)
}

/// Give a hidden pin a taskbar entry while preserving its borderless, topmost style.
/// Windows only; a no-op on macOS.
pub fn enable_pin_taskbar(handle: WindowHandle<'_>) -> Result<(), String> {
    native::enable_pin_taskbar(handle)
}

/// Enable minimization while keeping GPUI's titlebar-less pin panels undecorated.
/// macOS only; a no-op on Windows, where `enable_pin_taskbar` adds the minimize box.
pub fn enable_pin_minimization(handle: WindowHandle<'_>) -> Result<(), String> {
    native::enable_pin_minimization(handle)
}

pub fn window_minimized(handle: WindowHandle<'_>) -> Option<bool> {
    native::window_minimized(handle)
}

/// Disable DWM transitions before a pin is first shown, including minimize/restore.
/// Windows only; a no-op on macOS.
pub fn disable_window_animation(handle: WindowHandle<'_>) -> Result<(), String> {
    native::disable_window_animation(handle)
}

/// Keep capture overlays rectangular even when DWM rounds ordinary windows.
/// Windows only; a no-op on macOS.
pub fn disable_window_rounding(handle: WindowHandle<'_>) -> Result<(), String> {
    native::disable_window_rounding(handle)
}

/// Use DWM's smaller corner radius for compact floating windows such as pins.
/// Windows only; a no-op on macOS.
pub fn use_small_window_corners(handle: WindowHandle<'_>) -> Result<(), String> {
    native::use_small_window_corners(handle)
}

pub fn settle_desktop() -> Result<(), String> {
    native::settle_desktop()
}

/// Activate a visible overlay without restoring or changing its calibrated bounds.
#[cfg(target_os = "windows")]
pub fn activate_window_in_place(handle: WindowHandle<'_>) -> Result<(), String> {
    native::activate_window_in_place(handle)
}

/// Synchronously service a hidden window's paint on its owning UI thread.
/// Call outside any UI framework borrow: the paint path re-enters the window's
/// renderer. The caller obtains the raw handle immediately before this call
/// and must keep the owning window alive until it returns.
pub fn paint_hidden_window(handle: RawWindowHandle) -> Result<(), String> {
    native::paint_hidden_window(handle)
}

pub fn pointer_capture(handle: WindowHandle<'_>, capture: bool) -> Result<(), String> {
    native::pointer_capture(handle, capture)
}

pub fn set_client_bounds(
    handle: WindowHandle<'_>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f32,
) -> Result<(), String> {
    if !scale.is_finite() || scale <= 0. || width == 0 || height == 0 {
        return Err("Invalid pin window bounds".into());
    }
    native::set_client_bounds(handle, x, y, width, height, scale)
}

/// Fit a capture mask to the entire display, including the menu bar and Dock.
#[cfg(target_os = "macos")]
pub fn fit_capture_screen(handle: WindowHandle<'_>, display_id: u32) -> Result<(), String> {
    native::fit_capture_screen(handle, display_id)
}

pub fn hide_window(handle: WindowHandle<'_>) -> Result<(), String> {
    native::hide_window(handle)
}

pub fn show_window(handle: WindowHandle<'_>) -> Result<(), String> {
    native::show_window(handle)
}

/// Native visibility flag only; does not prove compositor presentation or scanout.
pub fn window_visible(handle: WindowHandle<'_>) -> Result<bool, String> {
    native::window_visible(handle)
}

#[cfg(target_os = "windows")]
pub fn client_origin(handle: WindowHandle<'_>) -> Result<(i32, i32), String> {
    native::client_origin(handle)
}

#[cfg(target_os = "windows")]
pub fn fit_client_bounds(
    handle: WindowHandle<'_>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<(), String> {
    native::fit_client_bounds(handle, x, y, width, height)
}

// Positions are physical pixels, so the tolerance only absorbs conversion noise.
#[cfg(any(target_os = "macos", test))]
fn resize_content_anchor(previous: (f64, f64), next: (i32, i32)) -> (bool, bool) {
    (
        (previous.0 - next.0 as f64).abs() > 0.25,
        (previous.1 - next.1 as f64).abs() > 0.25,
    )
}

#[cfg(test)]
mod resize_content_tests {
    use super::resize_content_anchor;

    #[test]
    fn resizing_preserves_the_opposite_corner_in_physical_pixels() {
        let origin = (-400., 200.);
        // Right/bottom edges move without moving the client origin.
        assert_eq!(resize_content_anchor(origin, (-400, 200)), (false, false));
        for step in [-20, -1, 1, 20] {
            assert_eq!(
                resize_content_anchor(origin, (-400 + step, 200)),
                (true, false)
            );
            assert_eq!(
                resize_content_anchor(origin, (-400, 200 + step)),
                (false, true)
            );
            assert_eq!(
                resize_content_anchor(origin, (-400 + step, 200 + step)),
                (true, true)
            );
        }
        assert_eq!(
            resize_content_anchor((-399.999999, 199.999999), (-400, 200)),
            (false, false)
        );
    }
}
