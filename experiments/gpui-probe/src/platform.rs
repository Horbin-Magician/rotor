//! P0 Windows boundary: GPUI borderless windows still have non-client insets.
//! Keep physical monitor fitting here instead of baking DPI offsets into views.
use anyhow::{Context, Result, ensure};
use gpui_kit::Window;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Graphics::Gdi::{
        ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    },
    UI::WindowsAndMessaging::{
        GetClientRect, GetWindowRect, HWND_TOPMOST, SWP_NOACTIVATE, SetWindowPos,
    },
};

pub fn fit_mask_to_monitor(window: &Window) -> Result<()> {
    let RawWindowHandle::Win32(raw) = HasWindowHandle::window_handle(window)
        .map_err(|error| anyhow::anyhow!("window handle unavailable: {error}"))?
        .as_raw()
    else {
        anyhow::bail!("Windows mask has no Win32 handle");
    };
    let hwnd = HWND(raw.hwnd.get() as *mut _);
    // SAFETY: GPUI owns this live HWND on the UI thread; all outputs are local,
    // correctly sized Win32 structures. No handle is stored beyond this call.
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        ensure!(
            GetMonitorInfoW(monitor, &mut info).as_bool(),
            "GetMonitorInfoW failed"
        );
        let desired = info.rcMonitor;
        // A DPI transition may change the insets after the first placement.
        for _ in 0..2 {
            let mut outer = RECT::default();
            let mut client = RECT::default();
            let mut origin = POINT::default();
            GetWindowRect(hwnd, &mut outer)?;
            GetClientRect(hwnd, &mut client)?;
            ensure!(
                ClientToScreen(hwnd, &mut origin).as_bool(),
                "ClientToScreen failed"
            );
            if origin.x == desired.left
                && origin.y == desired.top
                && client.right == desired.right - desired.left
                && client.bottom == desired.bottom - desired.top
            {
                eprintln!(
                    "mask physical client: ({}, {}) {}x{}",
                    origin.x, origin.y, client.right, client.bottom
                );
                return Ok(());
            }
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                outer.left + desired.left - origin.x,
                outer.top + desired.top - origin.y,
                desired.right - desired.left + (outer.right - outer.left - client.right),
                desired.bottom - desired.top + (outer.bottom - outer.top - client.bottom),
                SWP_NOACTIVATE,
            )
            .context("fit mask client area to physical monitor")?;
        }
        let mut client = RECT::default();
        let mut origin = POINT::default();
        GetClientRect(hwnd, &mut client)?;
        ensure!(
            ClientToScreen(hwnd, &mut origin).as_bool(),
            "ClientToScreen failed"
        );
        ensure!(
            origin.x == desired.left
                && origin.y == desired.top
                && client.right == desired.right - desired.left
                && client.bottom == desired.bottom - desired.top,
            "mask client area still differs from monitor bounds"
        );
        eprintln!(
            "mask physical client: ({}, {}) {}x{}",
            origin.x, origin.y, client.right, client.bottom
        );
    }
    Ok(())
}
