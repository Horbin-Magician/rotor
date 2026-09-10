use super::*;
use std::sync::Condvar;
use std::time::Instant;

#[derive(Clone, Copy)]
pub(super) struct CaptureRequest {
    pub id: OperationId,
    pub settle: bool,
    pub submitted: Instant,
}

#[derive(Default)]
struct Pending {
    request: Option<CaptureRequest>,
    stopped: bool,
}

pub(super) struct CaptureWorker {
    pending: Arc<(Mutex<Pending>, Condvar)>,
}

impl CaptureWorker {
    pub fn new(events: Sender<RuntimeEvent>, current: Arc<AtomicU64>) -> Result<Self, String> {
        let preparation_events = events.clone();
        Self::start_initialized(events, current, move || {
            let mut pool = monitor::CapturePool::default();
            // Prestart output workers without reading any desktop pixels.
            #[cfg(not(test))]
            if let Ok(monitors) = monitor::current_configs() {
                if let Err(error) = pool.prepare(&monitors) {
                    log::warn!("Capture worker warmup: {error}");
                }
            }
            move |request| capture_monitors(&mut pool, request, &preparation_events)
        })
    }

    #[cfg(test)]
    pub(super) fn start(
        events: Sender<RuntimeEvent>,
        current: Arc<AtomicU64>,
        capture: impl FnMut(CaptureRequest) -> Result<CaptureBundle, String> + Send + 'static,
    ) -> Result<Self, String> {
        Self::start_initialized(events, current, || capture)
    }

    fn start_initialized<F>(
        events: Sender<RuntimeEvent>,
        current: Arc<AtomicU64>,
        initialize: impl FnOnce() -> F + Send + 'static,
    ) -> Result<Self, String>
    where
        F: FnMut(CaptureRequest) -> Result<CaptureBundle, String> + Send + 'static,
    {
        let pending = Arc::new((Mutex::new(Pending::default()), Condvar::new()));
        let queue = pending.clone();
        std::thread::Builder::new().name("rotor-capture".into()).spawn(move || {
            let mut capture = initialize();
            loop {
                let request = {
                    let (mutex, wake) = &*queue;
                    let mut pending = lock(mutex);
                    while pending.request.is_none() && !pending.stopped {
                        pending = wake.wait(pending).unwrap_or_else(|error| error.into_inner());
                    }
                    if pending.stopped { break; }
                    pending.request.take().unwrap()
                };
                if current.load(Ordering::Acquire) != request.id.0 { continue; }
                log::debug!(target: "rotor_capture_latency", "capture_latency id={} stage=worker_start elapsed_us={}",
                    request.id.0, request.submitted.elapsed().as_micros());
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| capture(request)))
                    .unwrap_or_else(|_| Err("Screenshot coordinator panicked".into()));
                if current.load(Ordering::Acquire) == request.id.0 && !lock(&queue.0).stopped {
                    let _ = events.send_blocking(RuntimeEvent::CaptureFinished { id: request.id, result });
                }
            }
        }).map_err(|error| error.to_string())?;
        Ok(Self { pending })
    }

    pub fn submit(&self, request: CaptureRequest, current: &AtomicU64) -> Result<(), String> {
        let mut pending = lock(&self.pending.0);
        if pending.stopped {
            return Err("Screenshot worker is stopped".into());
        }
        current.store(request.id.0, Ordering::Release);
        // Only the newest request waits behind an in-flight OS capture.
        pending.request = Some(request);
        self.pending.1.notify_one();
        Ok(())
    }

    pub fn stop(&self) {
        let mut pending = lock(&self.pending.0);
        pending.stopped = true;
        pending.request = None;
        self.pending.1.notify_one();
    }
}

impl Drop for CaptureWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor() -> MonitorConfig {
        MonitorConfig {
            id: 9,
            x: -2,
            y: 0,
            width: 2,
            height: 3,
            scale_factor: 1.,
        }
    }

    fn synthetic_bundle(monitors: Vec<MonitorConfig>) -> CaptureBundle {
        CaptureBundle {
            monitors: monitors
                .into_iter()
                .map(|monitor| CapturedMonitor {
                    image: monitor::BgraCapture {
                        width: monitor.width,
                        height: monitor.height,
                        bytes: vec![255; (monitor.width * monitor.height * 4) as usize],
                    },
                    monitor,
                })
                .collect(),
            windows: Vec::new(),
        }
    }

    #[test]
    fn preparation_overlaps_capture_and_superseded_results_are_not_published() {
        let (events, received) = async_channel::bounded(4);
        let preparation_events = events.clone();
        let current = Arc::new(AtomicU64::new(0));
        let (started, starts) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let worker = CaptureWorker::start(events, current.clone(), move |request| {
            capture_with_preparation(
                request,
                &preparation_events,
                || Ok(vec![monitor()]),
                |monitors| {
                    started.send(request.id).unwrap();
                    wait.recv_timeout(Duration::from_secs(5)).unwrap();
                    Ok(synthetic_bundle(monitors))
                },
            )
        })
        .unwrap();
        let request = |id| CaptureRequest {
            id: OperationId(id),
            settle: false,
            submitted: Instant::now(),
        };
        worker.submit(request(1), &current).unwrap();
        assert_eq!(
            starts.recv_timeout(Duration::from_secs(5)).unwrap(),
            OperationId(1)
        );
        // The shell receives the exact topology while the pixel read is still
        // blocked, without acknowledging window preparation to the worker.
        assert!(matches!(
            received.try_recv().unwrap(),
            RuntimeEvent::CapturePreparing { id: OperationId(1), monitors }
                if monitors == vec![monitor()]
        ));
        assert!(received.is_empty());
        worker.submit(request(2), &current).unwrap();
        worker.submit(request(3), &current).unwrap();
        release.send(()).unwrap();
        assert_eq!(
            starts.recv_timeout(Duration::from_secs(5)).unwrap(),
            OperationId(3)
        );
        assert!(matches!(
            received.try_recv().unwrap(),
            RuntimeEvent::CapturePreparing { id: OperationId(3), monitors }
                if monitors == vec![monitor()]
        ));
        assert!(received.is_empty());
        release.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let event = loop {
            if let Ok(event) = received.try_recv() {
                break event;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(1));
        };
        assert!(matches!(
            event,
            RuntimeEvent::CaptureFinished {
                id: OperationId(3),
                result: Ok(_)
            }
        ));
        worker.stop();
        assert!(worker.submit(request(4), &current).is_err());
    }

    #[test]
    fn prepared_windows_do_not_make_failed_or_changed_captures_ready() {
        for fail_pixels in [false, true] {
            let (events, received) = async_channel::bounded(4);
            let mut topology_reads = 0;
            let result = capture_with_preparation(
                CaptureRequest {
                    id: OperationId(7),
                    settle: false,
                    submitted: Instant::now(),
                },
                &events,
                || {
                    let mut monitor = monitor();
                    topology_reads += 1;
                    if topology_reads > 1 {
                        monitor.scale_factor = 2.;
                    }
                    Ok(vec![monitor])
                },
                |monitors| {
                    assert!(matches!(
                        received.try_recv().unwrap(),
                        RuntimeEvent::CapturePreparing { id: OperationId(7), monitors }
                            if monitors == vec![monitor()]
                    ));
                    if fail_pixels {
                        Err("synthetic pixel failure".into())
                    } else {
                        Ok(synthetic_bundle(monitors))
                    }
                },
            );
            let error = result.err().expect("capture must fail after preparation");
            if fail_pixels {
                assert_eq!(error, "synthetic pixel failure");
                assert_eq!(topology_reads, 1);
            } else {
                assert!(error.contains("display topology changed"));
                assert_eq!(topology_reads, 2);
            }
            assert!(received.is_empty());
        }
    }
}
