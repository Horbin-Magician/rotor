//! Native cursor coordinates converted with the DPI of the monitor containing
//! the cursor. Windows desktop origins cannot use the primary monitor's scale.
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
