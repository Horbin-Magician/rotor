use super::*;
use rotor_screenshot::stitch::{self, AppendOutcome, StitchError, StitchSession};
use std::sync::atomic::AtomicU8;

const FINISH: u8 = 1;
const KEEP_ACCEPTED: u8 = 2;
const RESUME: u8 = 3;

/// What the worker currently sees. Issues other than `ScrolledBack` are only
/// reported after repeated failures, so a transient mid-scroll frame is quiet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LongCaptureStatus {
    Capturing,
    ScrolledBack,
    NoOverlap,
    Ambiguous,
    Limit,
    ViewportChanged,
}
impl LongCaptureStatus {
    fn from_error(error: &StitchError) -> Self {
        match error {
            StitchError::AmbiguousOverlap => Self::Ambiguous,
            StitchError::LimitExceeded => Self::Limit,
            StitchError::DimensionsChanged => Self::ViewportChanged,
            StitchError::NoOverlap | StitchError::Cancelled => Self::NoOverlap,
        }
    }
    pub fn is_issue(self) -> bool {
        !matches!(self, Self::Capturing | Self::ScrolledBack)
    }
}

pub enum LongCaptureEvent {
    /// Bounded thumbnails: the current viewport, the newest part of the long
    /// image at readable width, and the whole long image for orientation.
    Preview {
        current: Arc<RgbaImage>,
        tail: Arc<RgbaImage>,
        overview: Arc<RgbaImage>,
    },
    Progress {
        frames: usize,
        height: u32,
        status: LongCaptureStatus,
        review: bool,
    },
    Finished(Result<Arc<RgbaImage>, String>),
}

pub struct LongCaptureControl {
    cancel: Arc<AtomicBool>,
    finish: Arc<AtomicU8>,
}
impl LongCaptureControl {
    pub fn finish(&self) {
        self.finish.store(FINISH, Ordering::Release);
    }
    pub fn keep_accepted(&self) {
        self.finish.store(KEEP_ACCEPTED, Ordering::Release);
    }
    pub fn resume(&self) {
        self.finish.store(RESUME, Ordering::Release);
    }
}
impl Drop for LongCaptureControl {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

pub(super) fn start(
    id: OperationId,
    monitor: MonitorConfig,
    rect: rotor_canvas::ImageRect,
    first: Arc<RgbaImage>,
    current: Latest,
    events: Sender<RuntimeEvent>,
) -> Result<LongCaptureControl, String> {
    let mut session = StitchSession::new(first).map_err(|e| e.to_string())?;
    let control = LongCaptureControl {
        cancel: Arc::new(AtomicBool::new(false)),
        finish: Arc::new(AtomicU8::new(0)),
    };
    let cancel = control.cancel.clone();
    let finish = control.finish.clone();
    std::thread::Builder::new()
        .name("rotor-long-capture".into())
        .spawn(move || {
            let cancelled = || cancel.load(Ordering::Acquire) || !current.is_current(id);
            let run = || -> Result<(), String> {
                rotor_platform::overlay::settle_desktop()?;
                // Only the selection is read, reusing native resources, so the
                // sampling interval stays short while the page scrolls.
                let mut region = monitor::RegionCapture::new(monitor, rect)?;
                let result = collect(
                    &mut session,
                    &cancel,
                    &finish,
                    cancelled,
                    || region.read().map(Arc::new),
                    |event| {
                        events
                            .try_send(RuntimeEvent::LongCapture { id, event })
                            .is_ok()
                    },
                    |moving| {
                        for _ in 0..if moving { 1 } else { 5 } {
                            if cancelled() || finish.load(Ordering::Acquire) != 0 {
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(20));
                        }
                    },
                )?;
                if let Some(image) = result {
                    if !cancelled() {
                        events
                            .send_blocking(RuntimeEvent::LongCapture {
                                id,
                                event: LongCaptureEvent::Finished(Ok(image)),
                            })
                            .map_err(|e| e.to_string())?;
                    }
                }
                Ok(())
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
                .unwrap_or_else(|_| Err("Long capture worker panicked".into()));
            if !cancelled() {
                if let Err(error) = result {
                    let _ = events.send_blocking(RuntimeEvent::LongCapture {
                        id,
                        event: LongCaptureEvent::Finished(Err(error)),
                    });
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(control)
}

// Native capture and waiting are injected so cancellation, failed final frames
// and recovery can be exercised without reading the user's screen.
fn collect(
    session: &mut StitchSession,
    cancel: &AtomicBool,
    finish: &AtomicU8,
    cancelled: impl Fn() -> bool,
    mut capture: impl FnMut() -> Result<Arc<RgbaImage>, String>,
    mut progress: impl FnMut(LongCaptureEvent) -> bool,
    mut pause: impl FnMut(bool),
) -> Result<Option<Arc<RgbaImage>>, String> {
    let mut previous_status = None;
    let mut last_error = None;
    let mut shown = LongCaptureStatus::Capturing;
    let mut rejected = 0;
    let mut reviewing = false;
    let mut review_delivered = false;
    let mut preview_sent = None::<(std::time::Instant, usize)>;
    let mut stitched_preview: Option<(usize, Arc<RgbaImage>, Arc<RgbaImage>)> = None;
    let mut last_current = session.preview();
    let mut stats = Stats::default();
    let result = loop {
        if cancelled() {
            break Ok(None);
        }
        match finish.swap(0, Ordering::AcqRel) {
            KEEP_ACCEPTED => break Ok(Some(session.preview())),
            FINISH if last_error.is_none() => break Ok(Some(session.preview())),
            FINISH => {
                reviewing = true;
                review_delivered = false;
            }
            RESUME => {
                reviewing = false;
                previous_status = None;
            }
            _ => {}
        }
        if reviewing {
            // Freeze the accepted image and retry delivery under backpressure.
            // Do not capture more pixels after the user has asked to finish.
            if !review_delivered {
                // Deliver the final viewport and the frozen accepted result together
                // before offering the explicit keep/resume choice.
                let (tail, overview) = previews(&session.preview());
                review_delivered = progress(LongCaptureEvent::Preview {
                    current: fit(&last_current),
                    tail,
                    overview,
                }) && progress(LongCaptureEvent::Progress {
                    frames: session.frame_count(),
                    height: session.preview().height(),
                    status: last_error.unwrap_or(LongCaptureStatus::NoOverlap),
                    review: true,
                });
            }
            pause(false);
            continue;
        }
        let started = std::time::Instant::now();
        let next = match capture() {
            Ok(next) => next,
            Err(error) => break Err(error),
        };
        let captured = started.elapsed();
        last_current = next.clone();
        if cancelled() {
            break Ok(None);
        }
        let outcome = session.append(next.clone(), cancel);
        stats.record(captured, started.elapsed() - captured, &outcome);
        if cancelled() {
            break Ok(None);
        }
        // A request arriving during an OS read is processed at the next loop:
        // the in-flight frame is accounted for, but no extra read is scheduled.
        let moving = match outcome {
            Ok(outcome) => {
                last_error = None;
                rejected = 0;
                shown = if outcome == AppendOutcome::ScrolledBack {
                    LongCaptureStatus::ScrolledBack
                } else {
                    LongCaptureStatus::Capturing
                };
                outcome != AppendOutcome::Unchanged
            }
            Err(StitchError::Cancelled) => break Ok(None),
            Err(error) => {
                let status = LongCaptureStatus::from_error(&error);
                last_error = Some(status);
                rejected += 1;
                // Frames taken mid-scroll can fail once; report persistent issues.
                if rejected >= 3 {
                    shown = status;
                }
                true
            }
        };
        let status = (session.frame_count(), session.preview().height(), shown);
        if previous_status.as_ref() != Some(&status)
            && progress(LongCaptureEvent::Progress {
                frames: status.0,
                height: status.1,
                status: status.2,
                review: false,
            })
        {
            previous_status = Some(status);
        }
        // Keep event payloads small and prepare previews off the UI thread.
        // New stitched rows are shown promptly; the viewport refreshes slowly.
        let frames = session.frame_count();
        if preview_sent.is_none_or(|(sent, sent_frames)| {
            let elapsed = sent.elapsed();
            elapsed >= Duration::from_millis(300)
                || (sent_frames != frames && elapsed >= Duration::from_millis(120))
        }) {
            let (tail, overview) = match &stitched_preview {
                Some((previous, tail, overview)) if *previous == frames => {
                    (tail.clone(), overview.clone())
                }
                _ => previews(&session.preview()),
            };
            let current = fit(&next);
            if cancelled() {
                break Ok(None);
            }
            if progress(LongCaptureEvent::Preview {
                current,
                tail: tail.clone(),
                overview: overview.clone(),
            }) {
                stitched_preview = Some((frames, tail, overview));
                preview_sent = Some((std::time::Instant::now(), frames));
            }
        }
        // Sample quickly while the page moves so each step keeps an overlap.
        pause(moving);
    };
    stats.log(session);
    result
}

// Timing summary for diagnosing missed overlaps: a slow sampling interval lets
// the page scroll past the previous frame.
#[derive(Default)]
struct Stats {
    samples: u32,
    rejected: u32,
    capture: Duration,
    stitch: Duration,
    slowest: Duration,
}
impl Stats {
    fn record(
        &mut self,
        capture: Duration,
        stitch: Duration,
        outcome: &Result<AppendOutcome, StitchError>,
    ) {
        self.samples += 1;
        self.capture += capture;
        self.stitch += stitch;
        self.slowest = self.slowest.max(capture + stitch);
        if let Err(error) = outcome {
            self.rejected += 1;
            log::debug!(
                "Long capture sample {} rejected after {} ms capture, {} ms stitch: {error}",
                self.samples,
                capture.as_millis(),
                stitch.as_millis()
            );
        }
    }
    fn log(&self, session: &StitchSession) {
        if self.samples == 0 {
            return;
        }
        log::info!(
            "Long capture: {} frames from {} samples, {} rejected; average capture {} ms, stitch {} ms, slowest sample {} ms",
            session.frame_count(),
            self.samples,
            self.rejected,
            (self.capture / self.samples).as_millis(),
            (self.stitch / self.samples).as_millis(),
            self.slowest.as_millis()
        );
    }
}

const PREVIEW_WIDTH: u32 = 480;
const PREVIEW_HEIGHT: u32 = 400;

fn fit(image: &RgbaImage) -> Arc<RgbaImage> {
    Arc::new(stitch::fit_preview(image, PREVIEW_WIDTH, PREVIEW_HEIGHT))
}

// The newest rows at full preview width, and a narrow whole-image overview.
fn previews(image: &RgbaImage) -> (Arc<RgbaImage>, Arc<RgbaImage>) {
    (
        Arc::new(stitch::tail_preview(image, PREVIEW_WIDTH, PREVIEW_HEIGHT)),
        Arc::new(stitch::fit_preview(image, 96, PREVIEW_HEIGHT)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(start: u8) -> Arc<RgbaImage> {
        Arc::new(RgbaImage::from_fn(8, 24, |x, y| {
            image::Rgba([x as u8, y as u8 + start, 70, 255])
        }))
    }
    #[test]
    fn finish_uses_last_accepted_frame_without_an_extra_capture() {
        let cancel = AtomicBool::new(false);
        let finish = AtomicU8::new(0);
        let mut session = StitchSession::new(frame(0)).unwrap();
        let mut calls = 0;
        let result = collect(
            &mut session,
            &cancel,
            &finish,
            || false,
            || {
                calls += 1;
                assert_eq!(
                    calls, 1,
                    "finish must not read the hovered/inactive page again"
                );
                Ok(frame(12))
            },
            |_| true,
            |_| finish.store(FINISH, Ordering::Release),
        )
        .unwrap()
        .unwrap();
        assert_eq!(result.height(), 36);
        assert_eq!(session.frame_count(), 2);
    }
    #[test]
    fn rejected_tail_requires_explicit_choice_and_review_does_not_capture() {
        let cancel = AtomicBool::new(false);
        let finish = AtomicU8::new(0);
        let mut session = StitchSession::new(frame(0)).unwrap();
        session.append(frame(12), &cancel).unwrap();
        let accepted = session.preview();
        let mut calls = 0;
        let mut reviews = 0;
        let mut previews = 0;
        let result = collect(
            &mut session,
            &cancel,
            &finish,
            || false,
            || {
                calls += 1;
                assert_eq!(calls, 1);
                Ok(frame(90))
            },
            |event| {
                if let LongCaptureEvent::Preview { current, tail, .. } = &event {
                    previews += 1;
                    assert_eq!(**current, *fit(&frame(90)));
                    assert_eq!(**tail, *super::previews(&accepted).0);
                }
                if let LongCaptureEvent::Progress {
                    review: true,
                    status,
                    frames,
                    ..
                } = event
                {
                    assert!(status.is_issue());
                    assert_eq!(frames, 2);
                    reviews += 1;
                    // First delivery fails; frozen review must retry it.
                    if reviews == 1 {
                        return false;
                    }
                    finish.store(KEEP_ACCEPTED, Ordering::Release);
                }
                true
            },
            |_| {
                let _ = finish.compare_exchange(0, FINISH, Ordering::AcqRel, Ordering::Acquire);
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(reviews, 2);
        assert!(previews >= 2, "review retries include the frozen preview");
        assert!(Arc::ptr_eq(&result, &accepted));
    }
    #[test]
    fn preview_is_bounded_and_preserves_the_entire_long_image() {
        let image = RgbaImage::from_fn(400, 8000, |_, y| {
            if y < 4000 {
                image::Rgba([255, 0, 0, 255])
            } else {
                image::Rgba([0, 0, 255, 255])
            }
        });
        let (tail, overview) = previews(&image);
        // The newest rows stay readable instead of shrinking to a sliver.
        assert_eq!(tail.dimensions(), (400, 333));
        assert!(tail.pixels().all(|pixel| pixel.0 == [0, 0, 255, 255]));
        // The overview keeps the aspect ratio and shows both ends.
        assert_eq!(overview.dimensions(), (20, PREVIEW_HEIGHT));
        assert_eq!(overview.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(
            overview.get_pixel(0, overview.height() - 1).0,
            [0, 0, 255, 255]
        );
        let current = fit(&RgbaImage::new(1600, 900));
        assert_eq!(current.dimensions(), (PREVIEW_WIDTH, 270));
        assert_eq!(image.dimensions(), (400, 8000));
    }
    #[test]
    fn review_can_resume_and_include_the_previously_missing_tail() {
        let cancel = AtomicBool::new(false);
        let finish = AtomicU8::new(0);
        let mut session = StitchSession::new(frame(0)).unwrap();
        let mut frames = [frame(90), frame(12)].into_iter();
        let mut reviews = 0;
        let result = collect(
            &mut session,
            &cancel,
            &finish,
            || false,
            || Ok(frames.next().unwrap()),
            |event| {
                if let LongCaptureEvent::Progress { review: true, .. } = event {
                    reviews += 1;
                    finish.store(RESUME, Ordering::Release);
                }
                true
            },
            |_| {
                let _ = finish.compare_exchange(0, FINISH, Ordering::AcqRel, Ordering::Acquire);
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(reviews, 1);
        assert_eq!(result.height(), 36);
        assert!(frames.next().is_none());
    }
    #[test]
    fn scrolling_back_is_reported_immediately_and_finish_keeps_the_result() {
        let cancel = AtomicBool::new(false);
        let finish = AtomicU8::new(0);
        let mut session = StitchSession::new(frame(0)).unwrap();
        let mut frames = [frame(12), frame(2)].into_iter();
        let mut statuses = Vec::new();
        let mut pauses = Vec::new();
        let result = collect(
            &mut session,
            &cancel,
            &finish,
            || false,
            || Ok(frames.next().unwrap()),
            |event| {
                if let LongCaptureEvent::Progress { status, .. } = event {
                    statuses.push(status);
                }
                true
            },
            |moving| {
                pauses.push(moving);
                assert!(pauses.len() <= 2, "finish must not enter review");
                if pauses.len() == 2 {
                    finish.store(FINISH, Ordering::Release);
                }
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            statuses,
            [
                LongCaptureStatus::Capturing,
                LongCaptureStatus::ScrolledBack
            ]
        );
        // Both frames moved the page, so sampling stayed fast.
        assert_eq!(pauses, [true, true]);
        assert_eq!(result.height(), 36);
    }
    #[test]
    fn cancellation_during_capture_discards_the_frame_and_completion() {
        let cancel = AtomicBool::new(false);
        let finish = AtomicU8::new(0);
        let mut session = StitchSession::new(frame(0)).unwrap();
        let result = collect(
            &mut session,
            &cancel,
            &finish,
            || cancel.load(Ordering::Acquire),
            || {
                cancel.store(true, Ordering::Release);
                Ok(frame(12))
            },
            |_| panic!("cancelled progress"),
            |_| {},
        )
        .unwrap();
        assert!(result.is_none());
        assert_eq!(session.frame_count(), 1);
    }
    #[test]
    fn stale_generation_does_not_capture_or_publish() {
        let cancel = AtomicBool::new(false);
        let finish = AtomicU8::new(FINISH);
        let mut session = StitchSession::new(frame(0)).unwrap();
        assert!(collect(
            &mut session,
            &cancel,
            &finish,
            || true,
            || panic!("stale capture"),
            |_| panic!("stale progress"),
            |_| {}
        )
        .unwrap()
        .is_none());
    }
}
