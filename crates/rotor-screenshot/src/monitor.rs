use image::RgbaImage;
use std::collections::HashMap;
use std::error::Error;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use xcap::Monitor;

const CAPTURE_TIMEOUT: Duration = Duration::from_secs(5);

/// Top-down BGRA, ready for native rendering without a channel conversion.
pub struct BgraCapture {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

struct CaptureJob {
    monitor: MonitorConfig,
    reply: mpsc::Sender<Result<BgraCapture, String>>,
}

/// A bounded, persistent worker per output. Native capture resources never cross
/// thread boundaries. A stalled driver cannot create unbounded replacement threads.
#[derive(Default)]
pub struct CapturePool {
    workers: HashMap<u32, mpsc::SyncSender<CaptureJob>>,
}

impl CapturePool {
    pub fn prepare(&mut self, monitors: &[MonitorConfig]) -> Result<(), String> {
        self.workers
            .retain(|id, _| monitors.iter().any(|monitor| monitor.id == *id));
        for monitor in monitors {
            if self.workers.contains_key(&monitor.id) {
                continue;
            }
            let (sender, receiver) = mpsc::sync_channel::<CaptureJob>(1);
            #[cfg(target_os = "windows")]
            let warm_monitor = monitor.clone();
            thread::Builder::new()
                .name(format!("rotor-capture-{}", monitor.id))
                .spawn(move || {
                    #[cfg(target_os = "windows")]
                    let mut capture = rotor_platform::capture::DesktopCapture::default();
                    #[cfg(target_os = "windows")]
                    let mut previous_config = Some(warm_monitor.clone());
                    #[cfg(target_os = "windows")]
                    if let Err(error) = capture.prepare(warm_monitor.width, warm_monitor.height) {
                        log::warn!("Capture buffer warmup: {error}");
                    }
                    while let Ok(job) = receiver.recv() {
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let expected = &job.monitor;
                            #[cfg(target_os = "windows")]
                            let monitor = Monitor::from_point(expected.x, expected.y)
                                .map_err(|error| error.to_string())?;
                            #[cfg(not(target_os = "windows"))]
                            let monitor = Monitor::all()
                                .map_err(|error| error.to_string())?
                                .into_iter()
                                .find(|monitor| monitor.id().ok() == Some(expected.id))
                                .ok_or("Captured monitor is unavailable")?;
                            let current = MonitorConfig::from_monitor(&monitor)
                                .map_err(|error| error.to_string())?;
                            if current != *expected {
                                return Err("Display topology changed before capture".into());
                            }
                            #[cfg(target_os = "windows")]
                            let bytes = {
                                if previous_config.as_ref() != Some(expected) {
                                    capture = rotor_platform::capture::DesktopCapture::default();
                                    previous_config = Some(expected.clone());
                                }
                                capture.capture(
                                    expected.x,
                                    expected.y,
                                    expected.width,
                                    expected.height,
                                )?
                            };
                            #[cfg(not(target_os = "windows"))]
                            let bytes = {
                                let mut image =
                                    monitor.capture_image().map_err(|error| error.to_string())?;
                                if image.dimensions() != (expected.width, expected.height) {
                                    return Err("Capture dimensions changed".into());
                                }
                                for pixel in image.pixels_mut() {
                                    pixel.0.swap(0, 2);
                                }
                                image.into_raw()
                            };
                            Ok(BgraCapture {
                                width: expected.width,
                                height: expected.height,
                                bytes,
                            })
                        }))
                        .unwrap_or_else(|_| Err("Screenshot capture worker panicked".into()));
                        let _ = job.reply.send(result);
                    }
                })
                .map_err(|error| error.to_string())?;
            self.workers.insert(monitor.id, sender);
        }
        Ok(())
    }

    pub fn capture_with<T>(
        &mut self,
        monitors: &[MonitorConfig],
        alongside: impl FnOnce() -> T,
    ) -> Result<(Vec<BgraCapture>, T), String> {
        if monitors.is_empty() {
            return Err("No monitors available for screenshot capture".into());
        }
        self.prepare(monitors)?;
        let mut pending = Vec::with_capacity(monitors.len());
        for monitor in monitors {
            let (reply, receiver) = mpsc::channel();
            self.workers[&monitor.id]
                .try_send(CaptureJob {
                    monitor: monitor.clone(),
                    reply,
                })
                .map_err(|_| format!("Capture worker {} is still busy or stopped", monitor.id))?;
            pending.push(receiver);
        }
        let deadline = Instant::now() + CAPTURE_TIMEOUT;
        let extra = alongside();
        let images = pending
            .into_iter()
            .enumerate()
            .map(|(completed, receiver)| {
                receiver
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .map_err(|error| match error {
                        mpsc::RecvTimeoutError::Timeout => {
                            capture_timeout_message(completed, monitors.len())
                        }
                        mpsc::RecvTimeoutError::Disconnected => {
                            "Capture worker stopped unexpectedly".into()
                        }
                    })?
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok((images, extra))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MonitorConfig {
    pub id: u32,
    /// Desktop origin: points on macOS, physical pixels on Windows.
    pub x: i32,
    pub y: i32,
    /// Capture dimensions in physical pixels on every platform.
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

impl MonitorConfig {
    pub fn from_monitor(monitor: &Monitor) -> Result<Self, Box<dyn Error>> {
        let scale_factor = monitor.scale_factor()?;
        let (width, height) = (monitor.width()?, monitor.height()?);
        #[cfg(target_os = "macos")]
        let (width, height) = pixel_dimensions(width, height, scale_factor)?;
        Ok(Self {
            id: monitor.id()?,
            x: monitor.x()?,
            y: monitor.y()?,
            width,
            height,
            scale_factor,
        })
    }
}

#[cfg(any(target_os = "macos", test))]
fn pixel_dimensions(width: u32, height: u32, scale: f32) -> Result<(u32, u32), String> {
    if !scale.is_finite() || scale <= 0. {
        return Err("Invalid display scale".into());
    }
    let convert = |length: u32| {
        let pixels = (length as f64 * scale as f64).round();
        if pixels < 1. || pixels > u32::MAX as f64 {
            Err("Invalid capture dimensions".to_string())
        } else {
            Ok(pixels as u32)
        }
    };
    Ok((convert(width)?, convert(height)?))
}

pub fn mask_label(id: u32) -> String {
    format!("ssmask-{id}")
}

pub fn current_configs() -> Result<Vec<MonitorConfig>, Box<dyn Error>> {
    Monitor::all()?
        .iter()
        .map(MonitorConfig::from_monitor)
        .collect()
}

pub fn sorted_configs(mut configs: Vec<MonitorConfig>) -> Vec<MonitorConfig> {
    configs.sort_by_key(|config| config.id);
    configs
}

pub fn capture_all(monitors: Vec<Monitor>) -> Result<HashMap<String, RgbaImage>, String> {
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

#[cfg(test)]
mod geometry_tests {
    use super::pixel_dimensions;

    #[test]
    fn retina_and_external_display_dimensions_are_pixels() {
        assert_eq!(pixel_dimensions(1512, 982, 2.).unwrap(), (3024, 1964));
        assert_eq!(pixel_dimensions(1920, 1080, 1.).unwrap(), (1920, 1080));
        for scale in [0., -1., f32::NAN, f32::INFINITY] {
            assert!(pixel_dimensions(1512, 982, scale).is_err());
        }
        assert!(pixel_dimensions(0, 982, 2.).is_err());
        assert!(pixel_dimensions(u32::MAX, 982, 2.).is_err());
    }
}
