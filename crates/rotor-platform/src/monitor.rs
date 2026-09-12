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
