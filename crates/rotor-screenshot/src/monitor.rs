pub use rotor_platform::monitor::MonitorConfig;
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc,
};
use std::thread;
use std::time::{Duration, Instant};

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
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}

impl CaptureJob {
    fn expired(&self) -> bool {
        self.cancelled.load(Ordering::Acquire) || Instant::now() >= self.deadline
    }
}

struct CancelPending(Arc<AtomicBool>);
impl Drop for CancelPending {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

/// A bounded, persistent worker per output. Native capture resources never cross
/// thread boundaries and are released after each job. A stalled driver cannot
/// create unbounded replacement threads.
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
            thread::Builder::new()
                .name(format!("rotor-capture-{}", monitor.id))
                .spawn(move || {
                    while let Ok(job) = receiver.recv() {
                        if job.expired() {
                            continue;
                        }
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let expected = &job.monitor;
                            rotor_platform::monitor::validate_capture_monitor(expected)?;
                            if job.expired() {
                                return Err("Screenshot request expired".into());
                            }
                            #[cfg(target_os = "windows")]
                            let bytes = {
                                let mut capture =
                                    rotor_platform::capture::DesktopCapture::default();
                                capture.capture(
                                    expected.x,
                                    expected.y,
                                    expected.width,
                                    expected.height,
                                )?
                            };
                            #[cfg(target_os = "macos")]
                            let bytes = rotor_platform::capture::capture_display_bgra(
                                expected.id,
                                expected.width,
                                expected.height,
                            )?;
                            Ok(BgraCapture {
                                width: expected.width,
                                height: expected.height,
                                bytes,
                            })
                        }))
                        .unwrap_or_else(|_| Err("Screenshot capture worker panicked".into()));
                        if !job.expired() {
                            let _ = job.reply.send(result);
                        }
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
        cancelled: impl Fn() -> bool,
        alongside: impl FnOnce() -> T,
    ) -> Result<(Vec<BgraCapture>, T), String> {
        if monitors.is_empty() {
            return Err("No monitors available for screenshot capture".into());
        }
        self.prepare(monitors)?;
        let deadline = Instant::now() + CAPTURE_TIMEOUT;
        let cancel = CancelPending(Arc::new(AtomicBool::new(false)));
        let mut pending = Vec::with_capacity(monitors.len());
        for monitor in monitors {
            if cancelled() {
                return Err("Screenshot cancelled".into());
            }
            let (reply, receiver) = mpsc::channel();
            self.workers[&monitor.id]
                .try_send(CaptureJob {
                    monitor: monitor.clone(),
                    reply,
                    deadline,
                    cancelled: cancel.0.clone(),
                })
                .map_err(|_| format!("Capture worker {} is still busy or stopped", monitor.id))?;
            pending.push(receiver);
        }
        let extra = alongside();
        let images = pending
            .into_iter()
            .enumerate()
            .map(|(completed, receiver)| {
                wait_for_capture(receiver, deadline, &cancelled, completed, monitors.len())
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok((images, extra))
    }
}

fn wait_for_capture(
    receiver: mpsc::Receiver<Result<BgraCapture, String>>,
    deadline: Instant,
    cancelled: &impl Fn() -> bool,
    completed: usize,
    count: usize,
) -> Result<BgraCapture, String> {
    loop {
        if cancelled() {
            return Err("Screenshot cancelled".into());
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(capture_timeout_message(completed, count));
        }
        match receiver.recv_timeout(remaining.min(Duration::from_millis(20))) {
            Ok(result) => return result,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if Instant::now() >= deadline {
                    return Err(capture_timeout_message(completed, count));
                }
                return Err("Capture worker stopped unexpectedly".into());
            }
        }
    }
}

/// Repeated reads of one display region, for long capture. Create and use it
/// on a single thread: on Windows only the region is copied and the native
/// bitmap is reused between frames, which keeps the sampling interval short.
pub struct RegionCapture {
    monitor: MonitorConfig,
    rect: rotor_canvas::ImageRect,
    #[cfg(target_os = "windows")]
    native: rotor_platform::capture::DesktopCapture,
}

impl RegionCapture {
    pub fn new(monitor: MonitorConfig, rect: rotor_canvas::ImageRect) -> Result<Self, String> {
        if rect.width == 0
            || rect.height == 0
            || rect
                .x
                .checked_add(rect.width)
                .is_none_or(|x| x > monitor.width)
            || rect
                .y
                .checked_add(rect.height)
                .is_none_or(|y| y > monitor.height)
        {
            return Err("Long capture selection is outside the display".into());
        }
        Ok(Self {
            monitor,
            rect,
            #[cfg(target_os = "windows")]
            native: Default::default(),
        })
    }

    pub fn read(&mut self) -> Result<image::RgbaImage, String> {
        rotor_platform::monitor::validate_capture_monitor(&self.monitor)?;
        let rect = self.rect;
        #[cfg(target_os = "windows")]
        {
            let bytes = self.native.capture(
                self.monitor.x + rect.x as i32,
                self.monitor.y + rect.y as i32,
                rect.width,
                rect.height,
            )?;
            crop_bgra(
                &BgraCapture {
                    width: rect.width,
                    height: rect.height,
                    bytes,
                },
                rotor_canvas::ImageRect { x: 0, y: 0, ..rect },
            )
        }
        #[cfg(target_os = "macos")]
        {
            let bytes = rotor_platform::capture::capture_display_bgra(
                self.monitor.id,
                self.monitor.width,
                self.monitor.height,
            )?;
            crop_bgra(
                &BgraCapture {
                    width: self.monitor.width,
                    height: self.monitor.height,
                    bytes,
                },
                rect,
            )
        }
    }
}

/// Crop a BGRA capture to an RGBA image.
pub fn crop_bgra(
    capture: &BgraCapture,
    rect: rotor_canvas::ImageRect,
) -> Result<image::RgbaImage, String> {
    if rect.width == 0
        || rect.height == 0
        || rect
            .x
            .checked_add(rect.width)
            .is_none_or(|x| x > capture.width)
        || rect
            .y
            .checked_add(rect.height)
            .is_none_or(|y| y > capture.height)
        || capture.bytes.len() as u64 != u64::from(capture.width) * u64::from(capture.height) * 4
    {
        return Err("Long capture selection is outside the display".into());
    }
    let stride = capture.width as usize * 4;
    let row = rect.width as usize * 4;
    let mut bytes = Vec::with_capacity(row * rect.height as usize);
    for y in rect.y as usize..(rect.y + rect.height) as usize {
        let start = y * stride + rect.x as usize * 4;
        bytes.extend_from_slice(&capture.bytes[start..start + row]);
    }
    for pixel in bytes.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    image::RgbaImage::from_raw(rect.width, rect.height, bytes)
        .ok_or_else(|| "Invalid long capture region".into())
}

pub fn current_configs() -> Result<Vec<MonitorConfig>, String> {
    rotor_platform::monitor::current_configs()
}

pub fn sorted_configs(mut configs: Vec<MonitorConfig>) -> Vec<MonitorConfig> {
    configs.sort_by_key(|config| config.id);
    configs
}

fn capture_timeout_message(completed: usize, worker_count: usize) -> String {
    format!(
        "Screenshot capture timed out after {} ms ({completed}/{worker_count} monitors completed)",
        CAPTURE_TIMEOUT.as_millis()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_uses_physical_region_and_converts_channels() {
        let capture = BgraCapture {
            width: 2,
            height: 2,
            bytes: vec![1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255],
        };
        let rect = |x, width| rotor_canvas::ImageRect {
            x,
            y: 0,
            width,
            height: 2,
        };
        let image = crop_bgra(&capture, rect(1, 1)).unwrap();
        assert_eq!(image.as_raw(), &[6, 5, 4, 255, 12, 11, 10, 255]);
        assert!(crop_bgra(&capture, rect(u32::MAX, 2)).is_err());
        assert!(crop_bgra(&capture, rect(1, 2)).is_err());
    }

    #[test]
    fn cancelled_and_expired_jobs_do_not_wait_for_a_stalled_capture() {
        let (sender, receiver) = mpsc::channel();
        let cancel = CancelPending(Arc::new(AtomicBool::new(false)));
        let mut job = CaptureJob {
            monitor: MonitorConfig {
                id: 1,
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                scale_factor: 1.,
            },
            reply: sender,
            deadline: Instant::now() + CAPTURE_TIMEOUT,
            cancelled: cancel.0.clone(),
        };
        assert!(!job.expired());
        drop(cancel);
        assert!(job.expired());
        let result = wait_for_capture(receiver, job.deadline, &|| true, 0, 1);
        assert_eq!(result.err().unwrap(), "Screenshot cancelled");
        job.cancelled = Arc::new(AtomicBool::new(false));
        job.deadline = Instant::now();
        assert!(job.expired());
        let (_sender, receiver) = mpsc::channel();
        assert!(wait_for_capture(receiver, job.deadline, &|| false, 0, 1)
            .err()
            .unwrap()
            .contains("timed out"));
    }
}
