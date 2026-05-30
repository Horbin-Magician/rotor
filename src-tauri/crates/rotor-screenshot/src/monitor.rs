use image::RgbaImage;
use std::collections::HashMap;
use std::error::Error;
use std::thread;
use xcap::Monitor;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MonitorConfig {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

impl MonitorConfig {
    pub fn from_monitor(monitor: &Monitor) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            id: monitor.id()?,
            x: monitor.x()?,
            y: monitor.y()?,
            width: monitor.width()?,
            height: monitor.height()?,
            scale_factor: monitor.scale_factor()?,
        })
    }
}

pub(crate) fn mask_label(id: u32) -> String {
    format!("ssmask-{id}")
}

pub(crate) fn current_configs() -> Result<Vec<MonitorConfig>, Box<dyn Error>> {
    Monitor::all()?
        .iter()
        .map(MonitorConfig::from_monitor)
        .collect()
}

pub(crate) fn sorted_configs(mut configs: Vec<MonitorConfig>) -> Vec<MonitorConfig> {
    configs.sort_by_key(|config| config.id);
    configs
}

pub(crate) fn capture_all(monitors: Vec<Monitor>) -> HashMap<String, RgbaImage> {
    capture_all_inner(monitors)
}

#[cfg(target_os = "windows")]
fn capture_all_inner(monitors: Vec<Monitor>) -> HashMap<String, RgbaImage> {
    let handles = monitors
        .into_iter()
        .filter_map(|monitor| {
            let x = match monitor.x() {
                Ok(x) => x,
                Err(error) => {
                    log::error!("Failed to get monitor x coordinate before capture: {error}");
                    return None;
                }
            };
            let y = match monitor.y() {
                Ok(y) => y,
                Err(error) => {
                    log::error!("Failed to get monitor y coordinate before capture: {error}");
                    return None;
                }
            };

            Some(thread::spawn(move || capture_monitor_at_point(x, y)))
        })
        .collect::<Vec<_>>();

    collect_captures(handles)
}

#[cfg(not(target_os = "windows"))]
fn capture_all_inner(monitors: Vec<Monitor>) -> HashMap<String, RgbaImage> {
    let handles = monitors
        .into_iter()
        .map(|monitor| thread::spawn(move || capture_monitor(monitor)))
        .collect::<Vec<_>>();

    collect_captures(handles)
}

fn collect_captures(
    handles: Vec<thread::JoinHandle<Result<(String, RgbaImage), String>>>,
) -> HashMap<String, RgbaImage> {
    let mut captures = HashMap::new();
    for handle in handles {
        match handle.join() {
            Ok(Ok((label, image))) => {
                captures.insert(label, image);
            }
            Ok(Err(error)) => {
                log::error!("Failed to capture screenshot: {error}");
            }
            Err(_) => {
                log::error!("Screenshot capture worker panicked");
            }
        }
    }

    captures
}

#[cfg(target_os = "windows")]
fn capture_monitor_at_point(x: i32, y: i32) -> Result<(String, RgbaImage), String> {
    let monitor = Monitor::from_point(x, y).map_err(|error| {
        format!("failed to refresh monitor at ({x}, {y}) before capture: {error}")
    })?;
    capture_current_monitor(monitor)
}

#[cfg(not(target_os = "windows"))]
fn capture_monitor(monitor: Monitor) -> Result<(String, RgbaImage), String> {
    capture_current_monitor(monitor)
}

fn capture_current_monitor(monitor: Monitor) -> Result<(String, RgbaImage), String> {
    let id = monitor.id().map_err(|error| error.to_string())?;
    let image = monitor
        .capture_image()
        .map_err(|error| format!("monitor {id}: {error}"))?;
    Ok((mask_label(id), image))
}
