//! Native cursor coordinates converted with the DPI of the monitor containing
//! the cursor. Windows desktop origins cannot use the primary monitor's scale.

/// A drag-scoped cursor restriction. Keep this on the owning UI thread and drop
/// it before hiding the capture window or handing focus to another application.
#[cfg(target_os = "windows")]
pub struct CursorConfinement {
    _ui_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

#[cfg(target_os = "windows")]
impl CursorConfinement {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Result<Self, String> {
        use windows::Win32::{Foundation::RECT, UI::WindowsAndMessaging::ClipCursor};

        let (right, bottom) =
            confinement_edges(x, y, width, height).ok_or("Invalid cursor confinement bounds")?;
        let rect = RECT {
            left: x,
            top: y,
            right,
            bottom,
        };
        unsafe { ClipCursor(Some(&rect)) }.map_err(|error| error.to_string())?;
        Ok(Self {
            _ui_thread: std::marker::PhantomData,
        })
    }
}

#[cfg(target_os = "windows")]
impl Drop for CursorConfinement {
    fn drop(&mut self) {
        if let Err(error) = unsafe { windows::Win32::UI::WindowsAndMessaging::ClipCursor(None) } {
            log::warn!("Release capture cursor confinement: {error}");
        }
    }
}

#[cfg(target_os = "windows")]
fn confinement_edges(x: i32, y: i32, width: u32, height: u32) -> Option<(i32, i32)> {
    if width == 0 || height == 0 {
        return None;
    }
    Some((
        i32::try_from(i64::from(x) + i64::from(width)).ok()?,
        i32::try_from(i64::from(y) + i64::from(height)).ok()?,
    ))
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::confinement_edges;

    #[test]
    fn confinement_uses_physical_bounds_including_negative_origins() {
        assert_eq!(confinement_edges(-3840, -600, 3840, 2160), Some((0, 1560)));
        assert_eq!(confinement_edges(1920, 0, 2560, 1440), Some((4480, 1440)));
        assert_eq!(confinement_edges(0, 0, 0, 1080), None);
        assert_eq!(confinement_edges(0, 0, 1920, 0), None);
        assert_eq!(confinement_edges(i32::MAX, 0, 1, 1), None);
        assert_eq!(confinement_edges(0, i32::MAX, 1, 1), None);
    }
}

#[cfg(target_os = "windows")]
pub struct CursorLocation {
    pub display_id: u64,
    pub x: f32,
    pub y: f32,
}

#[cfg(target_os = "windows")]
pub fn location() -> Result<CursorLocation, String> {
    use windows::Win32::{
        Foundation::POINT,
        Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTONEAREST},
        UI::{
            HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
            WindowsAndMessaging::GetCursorPos,
        },
    };
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor) }.map_err(|error| error.to_string())?;
    let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
    let (mut x_dpi, mut y_dpi) = (0, 0);
    unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x_dpi, &mut y_dpi) }
        .map_err(|error| error.to_string())?;
    if x_dpi == 0 || x_dpi != y_dpi {
        return Err("Unsupported monitor DPI".into());
    }
    let scale = x_dpi as f32 / 96.;
    Ok(CursorLocation {
        display_id: monitor.0 as u64,
        x: cursor.x as f32 / scale,
        y: cursor.y as f32 / scale,
    })
}
