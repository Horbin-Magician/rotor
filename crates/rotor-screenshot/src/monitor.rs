pub use rotor_platform::monitor::MonitorConfig;
use std::collections::HashMap;
use std::error::Error;
use std::sync::mpsc;
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
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            let expected = &job.monitor;
                            rotor_platform::monitor::validate_capture_monitor(expected)?;
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

pub fn current_configs() -> Result<Vec<MonitorConfig>, Box<dyn Error>> {
    rotor_platform::monitor::current_configs().map_err(Into::into)
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
