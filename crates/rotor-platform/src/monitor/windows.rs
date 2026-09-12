use super::MonitorConfig;
use windows::{
    core::{BOOL, PCWSTR},
    Win32::{
        Foundation::{LPARAM, POINT, RECT},
        Graphics::Gdi::{
            CreateDCW, DeleteDC, EnumDisplayMonitors, EnumDisplaySettingsW, GetDeviceCaps,
            GetMonitorInfoW, MonitorFromPoint, DESKTOPHORZRES, DEVMODEW, ENUM_CURRENT_SETTINGS,
            HDC, HMONITOR, HORZRES, MONITORINFO, MONITORINFOEXW, MONITOR_DEFAULTTONULL,
        },
        UI::HiDpi::{
            GetDpiForMonitor, GetProcessDpiAwareness, MDT_EFFECTIVE_DPI, PROCESS_DPI_UNAWARE,
        },
    },
};

pub(super) fn current_configs() -> Result<Vec<MonitorConfig>, String> {
    unsafe extern "system" fn collect(
        monitor: HMONITOR,
        _: HDC,
        _: *mut RECT,
        state: LPARAM,
    ) -> BOOL {
        // EnumDisplayMonitors invokes this synchronously while the Vec is alive.
        unsafe { &mut *(state.0 as *mut Vec<HMONITOR>) }.push(monitor);
        BOOL(1)
    }
    let mut handles = Vec::<HMONITOR>::new();
    unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut handles as *mut _ as isize),
        )
        .ok()
        .map_err(|error| error.to_string())?;
    }
    handles.into_iter().map(config).collect()
}

pub(super) fn config_for_capture(expected: &MonitorConfig) -> Result<MonitorConfig, String> {
    let monitor = unsafe {
        MonitorFromPoint(
            POINT {
                x: expected.x,
                y: expected.y,
            },
            MONITOR_DEFAULTTONULL,
        )
    };
    if monitor.is_invalid() {
        return Err("Captured monitor is unavailable".into());
    }
    config(monitor)
}

fn config(monitor: HMONITOR) -> Result<MonitorConfig, String> {
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    let mut mode = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    unsafe {
        GetMonitorInfoW(monitor, &mut info as *mut _ as *mut MONITORINFO)
            .ok()
            .map_err(|error| error.to_string())?;
        EnumDisplaySettingsW(
            PCWSTR(info.szDevice.as_ptr()),
            ENUM_CURRENT_SETTINGS,
            &mut mode,
        )
        .ok()
        .map_err(|error| error.to_string())?;
    }
    config_from_mode(monitor, &mode, scale_factor(monitor, &info)?)
}

fn config_from_mode(
    monitor: HMONITOR,
    mode: &DEVMODEW,
    scale_factor: f32,
) -> Result<MonitorConfig, String> {
    if mode.dmPelsWidth == 0
        || mode.dmPelsHeight == 0
        || !scale_factor.is_finite()
        || scale_factor <= 0.
    {
        return Err("Invalid display geometry".into());
    }
    // EnumDisplaySettings supplies physical pixels, including negative desktop
    // origins, regardless of the caller's DPI awareness. Do not rescale them.
    let position = unsafe { mode.Anonymous1.Anonymous2.dmPosition };
    Ok(MonitorConfig {
        id: monitor.0 as usize as u32,
        x: position.x,
        y: position.y,
        width: mode.dmPelsWidth,
        height: mode.dmPelsHeight,
        scale_factor,
    })
}

fn scale_factor(monitor: HMONITOR, info: &MONITORINFOEXW) -> Result<f32, String> {
    unsafe {
        // Preserve the fallback used by diagnostics that run before GPUI sets
        // process DPI awareness. Normal desktop startup uses effective monitor DPI.
        if GetProcessDpiAwareness(None).is_ok_and(|awareness| awareness != PROCESS_DPI_UNAWARE) {
            let (mut x, mut y) = (0, 0);
            if GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y).is_ok() && x > 0 {
                return Ok(x as f32 / 96.);
            }
        }
        let device = PCWSTR(info.szDevice.as_ptr());
        let dc = CreateDCW(device, device, PCWSTR::null(), None);
        if dc.is_invalid() {
            return Err("Could not read display scale".into());
        }
        let physical = GetDeviceCaps(Some(dc), DESKTOPHORZRES);
        let logical = GetDeviceCaps(Some(dc), HORZRES);
        let _ = DeleteDC(dc);
        if physical <= 0 || logical <= 0 {
            return Err("Invalid display scale".into());
        }
        Ok(physical as f32 / logical as f32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_geometry_keeps_handle_identity_and_physical_pixels() {
        let mut mode = DEVMODEW {
            dmPelsWidth: 2560,
            dmPelsHeight: 1440,
            ..Default::default()
        };
        mode.Anonymous1.Anonymous2.dmPosition =
            windows::Win32::Foundation::POINTL { x: -2560, y: -120 };
        let monitor = config_from_mode(HMONITOR(42usize as *mut _), &mode, 1.5).unwrap();
        assert_eq!(
            monitor,
            MonitorConfig {
                id: 42,
                x: -2560,
                y: -120,
                width: 2560,
                height: 1440,
                scale_factor: 1.5,
            }
        );
        assert!(config_from_mode(HMONITOR::default(), &mode, f32::NAN).is_err());
        mode.dmPelsWidth = 0;
        assert!(config_from_mode(HMONITOR::default(), &mode, 1.).is_err());
    }
}
