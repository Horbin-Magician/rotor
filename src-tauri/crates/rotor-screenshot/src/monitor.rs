use image::RgbaImage;
use std::collections::HashMap;
use std::error::Error;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use xcap::Monitor;

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(5);

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

pub(crate) fn capture_all(monitors: Vec<Monitor>) -> Result<HashMap<String, RgbaImage>, String> {
    capture_all_inner(monitors)
}

#[cfg(target_os = "windows")]
fn capture_all_inner(monitors: Vec<Monitor>) -> Result<HashMap<String, RgbaImage>, String> {
    let mut capture_points = Vec::with_capacity(monitors.len());
    for monitor in monitors {
        let x = monitor
            .x()
            .map_err(|error| format!("failed to get monitor x coordinate: {error}"))?;
        let y = monitor
            .y()
            .map_err(|error| format!("failed to get monitor y coordinate: {error}"))?;
        capture_points.push((x, y));
    }

    run_capture_workers(capture_points, |(x, y)| capture_monitor_at_point(x, y))
}

#[cfg(not(target_os = "windows"))]
fn capture_all_inner(monitors: Vec<Monitor>) -> Result<HashMap<String, RgbaImage>, String> {
    run_capture_workers(monitors, capture_monitor)
}

fn run_capture_workers<T, F>(jobs: Vec<T>, capture: F) -> Result<HashMap<String, RgbaImage>, String>
where
    T: Send + 'static,
    F: Fn(T) -> Result<(String, RgbaImage), String> + Copy + Send + 'static,
{
    if jobs.is_empty() {
        return Err("No monitors available for screenshot capture".to_string());
    }

    let worker_count = jobs.len();
    let (sender, receiver) = mpsc::channel();
    for job in jobs {
        let sender = sender.clone();
        thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| capture(job)))
                .unwrap_or_else(|_| Err("Screenshot capture worker panicked".to_string()));
            let _ = sender.send(result);
        });
    }
    drop(sender);

    let deadline = Instant::now() + CAPTURE_TIMEOUT;
    let mut captures = HashMap::new();
    let mut errors = Vec::new();
    for completed in 0..worker_count {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(capture_timeout_message(completed, worker_count));
        }

        match receiver.recv_timeout(remaining) {
            Ok(Ok((label, image))) => {
                captures.insert(label, image);
            }
            Ok(Err(error)) => errors.push(error),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(capture_timeout_message(completed, worker_count));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("Screenshot capture workers stopped unexpectedly".to_string());
            }
        }
    }

    if !errors.is_empty() {
        return Err(format!(
            "Failed to capture all monitors: {}",
            errors.join("; ")
        ));
    }
    if captures.len() != worker_count {
        return Err(format!(
            "Screenshot capture returned {}/{} monitor images",
            captures.len(),
            worker_count
        ));
    }

    Ok(captures)
}

fn capture_timeout_message(completed: usize, worker_count: usize) -> String {
    format!(
        "Screenshot capture timed out after {} ms ({completed}/{worker_count} monitors completed)",
        CAPTURE_TIMEOUT.as_millis()
    )
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
