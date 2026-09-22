//! Display topology used by capture workers, independent of any UI runtime.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
use macos as native;
#[cfg(target_os = "windows")]
use windows as native;

#[derive(Debug, Clone, PartialEq)]
pub struct MonitorConfig {
    /// Quartz display ID on macOS; low 32 bits of HMONITOR on Windows.
    /// Matches the existing capture records and the shell's GPUI display lookup.
    pub id: u32,
    /// Desktop origin: points on macOS, physical pixels on Windows.
    pub x: i32,
    pub y: i32,
    /// Capture dimensions in physical pixels on both supported platforms.
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

/// Offset between a monitor's desktop origin and the origin of its captured
/// pixels. macOS reports origins in points while captures are in physical
/// pixels, so a scaled display's pixel origin moves by `origin * (scale - 1)`.
/// Windows origins are already physical pixels.
pub fn pixel_origin_offset(config: &MonitorConfig) -> (i32, i32) {
    if cfg!(target_os = "macos") {
        let scale = f64::from(config.scale_factor) - 1.;
        (
            (f64::from(config.x) * scale).round() as i32,
            (f64::from(config.y) * scale).round() as i32,
        )
    } else {
        (0, 0)
    }
}

pub fn current_configs() -> Result<Vec<MonitorConfig>, String> {
    native::current_configs()
}

/// Re-read native geometry immediately before the worker captures pixels.
pub fn validate_capture_monitor(expected: &MonitorConfig) -> Result<(), String> {
    if native::config_for_capture(expected)? != *expected {
        return Err("Display topology changed before capture".into());
    }
    Ok(())
}
