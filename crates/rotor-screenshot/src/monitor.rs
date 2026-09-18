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
