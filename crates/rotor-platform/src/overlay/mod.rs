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

/// Queue a native caption drag without entering the Win32 move loop in a UI borrow.
#[cfg(target_os = "windows")]
pub fn start_window_move(handle: WindowHandle<'_>) -> Result<(), String> {
    native::start_window_move(handle)
}

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

/// Keep overlays rectangular: disable DWM rounding on Windows and remove
/// AppKit's titled decoration on macOS while preserving client geometry.
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

/// Physical-pixel screen rectangle: left, top, width, height.
pub type ScreenRect = (i32, i32, u32, u32);

// Rotor's capture accent (0x4ba3e3).
const FRAME_COLOR: (u8, u8, u8) = (0x4b, 0xa3, 0xe3);

/// A click-through outline drawn just outside a screen region while it is
/// captured repeatedly. The outline never covers the region's own pixels, so
/// it neither enters the capture nor intercepts input to the page beneath.
/// Create and drop it on the UI thread; dropping it removes the outline.
pub struct SelectionFrame(#[allow(dead_code)] native::SelectionFrame);

impl SelectionFrame {
    /// `region` and `display` are physical pixels; sides that would fall
    /// outside `display` are omitted rather than drawn on a neighboring display.
    pub fn show(
        region: ScreenRect,
        display: ScreenRect,
        thickness: u32,
        scale: f32,
    ) -> Result<Self, String> {
        if !scale.is_finite() || scale <= 0. || thickness == 0 {
            return Err("Invalid selection frame".into());
        }
        native::SelectionFrame::show(&frame_strips(region, display, thickness), scale).map(Self)
    }
}

fn frame_strips(region: ScreenRect, display: ScreenRect, thickness: u32) -> Vec<ScreenRect> {
    let (x, y, w, h) = (
        i64::from(region.0),
        i64::from(region.1),
        i64::from(region.2),
        i64::from(region.3),
    );
    let t = i64::from(thickness);
    let clip = (
        i64::from(display.0),
        i64::from(display.1),
        i64::from(display.0) + i64::from(display.2),
        i64::from(display.1) + i64::from(display.3),
    );
    [
        (x - t, y - t, x + w + t, y),
        (x - t, y + h, x + w + t, y + h + t),
        (x - t, y, x, y + h),
        (x + w, y, x + w + t, y + h),
    ]
    .into_iter()
    .filter_map(|(left, top, right, bottom)| {
        let (left, top) = (left.max(clip.0), top.max(clip.1));
        let (right, bottom) = (right.min(clip.2), bottom.min(clip.3));
        (right > left && bottom > top).then(|| {
            (
                left as i32,
                top as i32,
                (right - left) as u32,
                (bottom - top) as u32,
            )
        })
    })
    .collect()
}

#[cfg(test)]
mod frame_tests {
    use super::frame_strips;

    #[test]
    fn frame_surrounds_the_region_without_covering_it() {
        let region = (100, 50, 400, 300);
        let strips = frame_strips(region, (0, 0, 1920, 1080), 3);
        assert_eq!(
            strips,
            vec![
                (97, 47, 406, 3),
                (97, 350, 406, 3),
                (97, 50, 3, 300),
                (500, 50, 3, 300),
            ]
        );
        for (x, y, w, h) in strips {
            let disjoint = x + w as i32 <= region.0
                || y + h as i32 <= region.1
                || x >= region.0 + region.2 as i32
                || y >= region.1 + region.3 as i32;
            assert!(disjoint);
        }
    }

    #[test]
    fn frame_stays_on_the_captured_display() {
        // A full-height region on a secondary display left of the primary.
        let strips = frame_strips((-1920, 0, 800, 1080), (-1920, 0, 1920, 1080), 2);
        assert_eq!(strips, vec![(-1120, 0, 2, 1080)]);
        assert!(frame_strips((0, 0, 1920, 1080), (0, 0, 1920, 1080), 2).is_empty());
    }
}

// Positions are physical pixels, so the tolerance only absorbs conversion noise.
#[cfg(any(target_os = "macos", test))]
fn resize_content_anchor(previous: (f64, f64), next: (i32, i32)) -> (bool, bool) {
    (
        (previous.0 - next.0 as f64).abs() > 0.25,
        (previous.1 - next.1 as f64).abs() > 0.25,
    )
}

/// Live client rectangle in the coordinate space used by `set_client_bounds`.
/// On macOS all windows use the caller's scale, including across displays.
pub fn client_bounds(handle: WindowHandle<'_>, scale: f32) -> Result<(i32, i32, u32, u32), String> {
    if !scale.is_finite() || scale <= 0. {
        return Err("Invalid window scale".into());
    }
    native::client_bounds(handle, scale)
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
