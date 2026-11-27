use super::*;
use rotor_screenshot::recorder::{
    bgra_preview_rgba, export_gif, export_mp4, ExportFormat, ExportProgress, GifPreset,
    RecordError, RecordedClip, RecordingSpool, DEFAULT_RECORD_FPS,
};
use std::{
    path::PathBuf,
    sync::{
        atomic::AtomicU8,
        mpsc::{self, Sender as StdSender},
    },
    time::Instant,
};

const CMD_NONE: u8 = 0;
const CMD_PAUSE: u8 = 1;
const CMD_RESUME: u8 = 2;
const CMD_STOP: u8 = 3;

const PREVIEW_WIDTH: u32 = 480;
const PREVIEW_HEIGHT: u32 = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenRecordLimit {
    StorageBudget,
    DurationLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenRecordStatus {
    Recording,
    Paused,
}

pub enum ScreenRecordEvent {
    Preview(Arc<RgbaImage>),
    Progress {
        elapsed_ms: u64,
        frames: usize,
        unique_frames: usize,
        spool_bytes: u64,
        status: ScreenRecordStatus,
    },
    Stopped {
        duration_ms: u64,
        frames: usize,
        unique_frames: usize,
        spool_bytes: u64,
        limit: Option<ScreenRecordLimit>,
    },
    ExportProgress {
        format: ExportFormat,
        progress: ExportProgress,
    },
    ExportFinished {
        format: ExportFormat,
        path: PathBuf,
        result: Result<(), String>,
    },
    Failed(String),
}

struct ExportRequest {
    format: ExportFormat,
    path: PathBuf,
    gif_preset: GifPreset,
    cancel: Arc<AtomicBool>,
}

pub struct ScreenRecordControl {
    cancel: Arc<AtomicBool>,
    command: Arc<AtomicU8>,
    export_tx: StdSender<ExportRequest>,
    active_export_cancel: Mutex<Option<Arc<AtomicBool>>>,
}

impl ScreenRecordControl {
    pub fn pause(&self) {
        self.command.store(CMD_PAUSE, Ordering::Release);
    }

    pub fn resume(&self) {
        self.command.store(CMD_RESUME, Ordering::Release);
    }

    pub fn stop(&self) {
        self.command.store(CMD_STOP, Ordering::Release);
    }

    pub fn export(
        &self,
        format: ExportFormat,
        path: PathBuf,
        gif_preset: GifPreset,
    ) -> Result<(), String> {
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut active = self
                .active_export_cancel
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(prev) = active.replace(cancel.clone()) {
                prev.store(true, Ordering::Release);
            }
        }
        self.export_tx
            .send(ExportRequest {
                format,
                path,
                gif_preset,
                cancel,
            })
            .map_err(|_| "Screen recording worker is no longer running".to_string())
    }

    pub fn cancel_export(&self) {
        if let Some(cancel) = self
            .active_export_cancel
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            cancel.store(true, Ordering::Release);
        }
    }
}

impl Drop for ScreenRecordControl {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        self.cancel_export();
    }
}

pub(super) fn start(
    id: OperationId,
    monitor: MonitorConfig,
    rect: rotor_canvas::ImageRect,
    fps: u32,
    current: Latest,
    events: Sender<RuntimeEvent>,
) -> Result<ScreenRecordControl, String> {
    let fps = if fps == 0 {
        DEFAULT_RECORD_FPS
    } else {
        fps.clamp(5, 60)
    };
    let spool = RecordingSpool::new(rect.width, rect.height, fps, monitor.scale_factor)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let command = Arc::new(AtomicU8::new(CMD_NONE));
    let (export_tx, export_rx) = mpsc::channel::<ExportRequest>();

    let worker_cancel = cancel.clone();
    let worker_command = command.clone();
    let width = rect.width;
    let height = rect.height;

    std::thread::Builder::new()
        .name("rotor-screen-record".into())
        .spawn(move || {
            let cancelled = || worker_cancel.load(Ordering::Acquire) || !current.is_current(id);
            let run = || -> Result<(), String> {
                rotor_platform::overlay::settle_desktop()?;
                let mut region = monitor::RegionCapture::new(monitor, rect)?;
                let mut clock = Instant::now();
                let recorded = record_loop(
                    spool,
                    width,
                    height,
                    fps,
                    &worker_command,
                    &cancelled,
                    || region.read_bgra(),
                    || {
                        let now = Instant::now();
                        let delta = now.saturating_duration_since(clock).as_millis() as u64;
                        clock = now;
                        delta
                    },
                    |delay| sleep_interruptible(delay, &cancelled, &worker_command),
                    |event| {
                        events
                            .try_send(RuntimeEvent::ScreenRecord { id, event })
                            .is_ok()
                    },
                )?;
                let Some((clip, limit)) = recorded else {
                    return Ok(());
                };
                if cancelled() {
                    return Ok(());
                }
                let _ = events.send_blocking(RuntimeEvent::ScreenRecord {
                    id,
                    event: ScreenRecordEvent::Stopped {
                        duration_ms: clip.duration_ms(),
                        frames: clip.captured_frames(),
                        unique_frames: clip.unique_frames(),
                        spool_bytes: clip.spool_bytes(),
                        limit,
                    },
                });
                export_loop(
                    &clip,
                    export_rx,
                    &cancelled,
                    |event| {
                        let _ = events.send_blocking(RuntimeEvent::ScreenRecord { id, event });
                    },
                    |event| {
                        let _ = events.try_send(RuntimeEvent::ScreenRecord { id, event });
                    },
                );
                Ok(())
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
                .unwrap_or_else(|_| Err("Screen recording worker panicked".into()));
            if !cancelled() {
                if let Err(error) = result {
                    let _ = events.send_blocking(RuntimeEvent::ScreenRecord {
                        id,
                        event: ScreenRecordEvent::Failed(error),
                    });
                }
            }
        })
        .map_err(|e| e.to_string())?;

    Ok(ScreenRecordControl {
        cancel,
        command,
        export_tx,
        active_export_cancel: Mutex::new(None),
    })
}

fn sleep_interruptible(total: Duration, cancelled: &impl Fn() -> bool, command: &AtomicU8) {
    let deadline = Instant::now() + total;
    while Instant::now() < deadline {
        if cancelled() || command.load(Ordering::Acquire) != CMD_NONE {
            break;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        std::thread::sleep(remaining.min(Duration::from_millis(10)));
    }
}

#[allow(clippy::too_many_arguments)]
fn record_loop(
    mut spool: RecordingSpool,
    width: u32,
    height: u32,
    fps: u32,
    command: &AtomicU8,
    cancelled: &impl Fn() -> bool,
    mut capture: impl FnMut() -> Result<Vec<u8>, String>,
    mut advance_ms: impl FnMut() -> u64,
    mut wait: impl FnMut(Duration),
    mut publish: impl FnMut(ScreenRecordEvent) -> bool,
) -> Result<Option<(RecordedClip, Option<ScreenRecordLimit>)>, String> {
    let frame_period_ms = (1000 / u64::from(fps.max(1))).max(16);
    let mut paused = false;
    let mut elapsed_ms: u64 = 0;
    let mut limit_reached = None;
    let mut last_progress = None::<Instant>;
    let mut last_preview = None::<Instant>;
    // Reset clock anchor right before capturing frame 0.
    let _ = advance_ms();

    loop {
        if cancelled() {
            return Ok(None);
        }
        match command.swap(CMD_NONE, Ordering::AcqRel) {
            CMD_STOP => break,
            CMD_PAUSE if !paused => {
                elapsed_ms = elapsed_ms.saturating_add(advance_ms());
                paused = true;
                // A full UI event queue must not lose the pause acknowledgement.
                // The paused branch retries without capturing or advancing time.
                last_progress = None;
            }
            CMD_RESUME if paused => {
                // Discard time spent paused.
                let _ = advance_ms();
                paused = false;
                publish(ScreenRecordEvent::Progress {
                    elapsed_ms,
                    frames: spool.captured_frames(),
                    unique_frames: spool.unique_frames(),
                    spool_bytes: spool.spool_bytes(),
                    status: ScreenRecordStatus::Recording,
                });
            }
            _ => {}
        }
        if paused {
            if last_progress.is_none()
                && publish(ScreenRecordEvent::Progress {
                    elapsed_ms,
                    frames: spool.captured_frames(),
                    unique_frames: spool.unique_frames(),
                    spool_bytes: spool.spool_bytes(),
                    status: ScreenRecordStatus::Paused,
                })
            {
                last_progress = Some(Instant::now());
            }
            wait(Duration::from_millis(40));
            continue;
        }

        let step_started = Instant::now();
        let bgra = capture()?;
        if cancelled() {
            return Ok(None);
        }
        let delta = if spool.captured_frames() == 0 {
            let _ = advance_ms();
            0
        } else {
            advance_ms()
        };
        elapsed_ms = elapsed_ms.saturating_add(delta);

        let should_preview =
            last_preview.is_none_or(|sent| sent.elapsed() >= Duration::from_millis(250));
        let preview_image = should_preview.then(|| {
            Arc::new(bgra_preview_rgba(
                &bgra,
                width,
                height,
                PREVIEW_WIDTH,
                PREVIEW_HEIGHT,
            ))
        });

        match spool.push_bgra(elapsed_ms, bgra)? {
            Ok(()) => {}
            Err(RecordError::StorageBudget) => {
                limit_reached = Some(ScreenRecordLimit::StorageBudget);
                break;
            }
            Err(RecordError::DurationLimit) => {
                limit_reached = Some(ScreenRecordLimit::DurationLimit);
                break;
            }
        }

        if let Some(preview) = preview_image {
            if publish(ScreenRecordEvent::Preview(preview)) {
                last_preview = Some(Instant::now());
            }
        }

        if last_progress.is_none_or(|sent| sent.elapsed() >= Duration::from_millis(100))
            && publish(ScreenRecordEvent::Progress {
                elapsed_ms,
                frames: spool.captured_frames(),
                unique_frames: spool.unique_frames(),
                spool_bytes: spool.spool_bytes(),
                status: ScreenRecordStatus::Recording,
            })
        {
            last_progress = Some(Instant::now());
        }

        let spent = step_started.elapsed();
        let target = Duration::from_millis(frame_period_ms);
        if let Some(remaining) = target.checked_sub(spent) {
            wait(remaining);
        }
    }

    if cancelled() {
        return Ok(None);
    }
    if !paused && spool.captured_frames() > 0 {
        elapsed_ms = elapsed_ms.saturating_add(advance_ms());
    }
    let clip = spool.finish(elapsed_ms)?;
    Ok(Some((clip, limit_reached)))
}

fn export_loop(
    clip: &RecordedClip,
    rx: mpsc::Receiver<ExportRequest>,
    session_cancelled: &impl Fn() -> bool,
    mut send_blocking: impl FnMut(ScreenRecordEvent),
    mut try_send: impl FnMut(ScreenRecordEvent),
) {
    while let Ok(request) = rx.recv() {
        if session_cancelled() {
            break;
        }
        let format = request.format;
        let path = request.path;
        let export_cancel = request.cancel;
        let is_cancelled = || session_cancelled() || export_cancel.load(Ordering::Acquire);
        let mut last_sent = None::<Instant>;
        let mut report = |progress: ExportProgress| {
            let final_step = progress.completed >= progress.total;
            if final_step || last_sent.is_none_or(|t| t.elapsed() >= Duration::from_millis(60)) {
                try_send(ScreenRecordEvent::ExportProgress { format, progress });
                last_sent = Some(Instant::now());
            }
        };
        let result = match format {
            ExportFormat::Mp4 => export_mp4(clip, &path, is_cancelled, &mut report),
            ExportFormat::Gif => {
                export_gif(clip, &path, request.gif_preset, is_cancelled, &mut report)
            }
        };
        if !session_cancelled() {
            send_blocking(ScreenRecordEvent::ExportFinished {
                format,
                path,
                result,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_bgra(value: u8) -> Vec<u8> {
        let mut frame = vec![0u8; 32 * 24 * 4];
        for pixel in frame.chunks_exact_mut(4) {
            pixel[0] = value;
            pixel[1] = 80;
            pixel[2] = 255u8.saturating_sub(value);
            pixel[3] = 255;
        }
        frame
    }

    #[test]
    fn pause_excludes_paused_time_and_stop_yields_exportable_clip() {
        let spool = RecordingSpool::new(32, 24, 20, 1.).unwrap();
        let command = AtomicU8::new(CMD_NONE);
        let cancel = AtomicBool::new(false);
        let step = std::cell::Cell::new(0usize);
        let paused_once = std::cell::Cell::new(false);
        let mut statuses = Vec::new();
        let (clip, limit) = record_loop(
            spool,
            32,
            24,
            20,
            &command,
            &|| cancel.load(Ordering::Acquire),
            || {
                let next = step.get() + 1;
                step.set(next);
                Ok(synthetic_bgra((next * 40) as u8))
            },
            || 50,
            |_| match step.get() {
                1 if !paused_once.replace(true) => command.store(CMD_PAUSE, Ordering::Release),
                2 => command.store(CMD_STOP, Ordering::Release),
                _ => {}
            },
            |event| {
                if let ScreenRecordEvent::Progress { status, .. } = event {
                    statuses.push(status);
                    if status == ScreenRecordStatus::Paused {
                        command.store(CMD_RESUME, Ordering::Release);
                    }
                }
                true
            },
        )
        .unwrap()
        .unwrap();

        assert_eq!(limit, None);
        assert_eq!(clip.captured_frames(), 2);
        assert!(statuses.contains(&ScreenRecordStatus::Paused));
        assert!(statuses.contains(&ScreenRecordStatus::Recording));

        let dir = tempfile::tempdir().unwrap();
        let gif_out = dir.path().join("recorded.gif");
        let (tx, rx) = mpsc::channel();
        tx.send(ExportRequest {
            format: ExportFormat::Gif,
            path: gif_out.clone(),
            gif_preset: GifPreset::Standard,
            cancel: Arc::new(AtomicBool::new(false)),
        })
        .unwrap();
        drop(tx);
        let mut finished = Vec::new();
        export_loop(
            &clip,
            rx,
            &|| false,
            |event| {
                if let ScreenRecordEvent::ExportFinished { format, result, .. } = event {
                    finished.push((format, result));
                }
            },
            |_| {},
        );
        assert_eq!(finished, vec![(ExportFormat::Gif, Ok(()))]);
        assert!(gif_out.is_file());
    }

    #[test]
    fn cancellation_stops_recording_without_producing_clip() {
        let spool = RecordingSpool::new(32, 24, 20, 1.).unwrap();
        let command = AtomicU8::new(CMD_NONE);
        let cancel = AtomicBool::new(false);
        let result = record_loop(
            spool,
            32,
            24,
            20,
            &command,
            &|| cancel.load(Ordering::Acquire),
            || {
                cancel.store(true, Ordering::Release);
                Ok(synthetic_bgra(10))
            },
            || 50,
            |_| {},
            |_| panic!("cancelled recording must not emit events"),
        )
        .unwrap();
        assert!(result.is_none());
    }
    #[test]
    fn paused_progress_retries_after_a_full_event_queue() {
        let spool = RecordingSpool::new(32, 24, 20, 1.).unwrap();
        let command = AtomicU8::new(CMD_NONE);
        let captures = std::cell::Cell::new(0);
        let attempts = std::cell::Cell::new(0);
        let (clip, _) = record_loop(
            spool,
            32,
            24,
            20,
            &command,
            &|| false,
            || {
                captures.set(captures.get() + 1);
                Ok(synthetic_bgra(40))
            },
            || 50,
            |_| {
                if attempts.get() == 0 {
                    command.store(CMD_PAUSE, Ordering::Release);
                }
            },
            |event| {
                if matches!(
                    event,
                    ScreenRecordEvent::Progress {
                        status: ScreenRecordStatus::Paused,
                        ..
                    }
                ) {
                    attempts.set(attempts.get() + 1);
                    if attempts.get() == 1 {
                        return false;
                    }
                    command.store(CMD_STOP, Ordering::Release);
                }
                true
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(attempts.get(), 2);
        assert_eq!(captures.get(), 1);
        assert_eq!(clip.captured_frames(), 1);
        assert_eq!(clip.duration_ms(), 50);
    }
}
