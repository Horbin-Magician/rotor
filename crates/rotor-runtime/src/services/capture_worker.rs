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
        Self::start_initialized(events, current, || {
            let mut pool = monitor::CapturePool::default();
            // Prestart output workers without reading any desktop pixels.
            #[cfg(not(test))]
            if let Ok(monitors) = monitor::current_configs() {
                if let Err(error) = pool.prepare(&monitors) {
                    log::warn!("Capture worker warmup: {error}");
                }
            }
            move |request| capture_monitors(&mut pool, request)
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

    #[test]
    fn latest_request_replaces_pending_and_cancelled_result_is_not_published() {
        let (events, received) = async_channel::bounded(4);
        let current = Arc::new(AtomicU64::new(0));
        let (started, starts) = std::sync::mpsc::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let worker = CaptureWorker::start(events, current.clone(), move |request| {
            started.send(request.id).unwrap();
            wait.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok(CaptureBundle {
                monitors: Vec::new(),
                windows: Vec::new(),
            })
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
        worker.submit(request(2), &current).unwrap();
        worker.submit(request(3), &current).unwrap();
        release.send(()).unwrap();
        assert_eq!(
            starts.recv_timeout(Duration::from_secs(5)).unwrap(),
            OperationId(3)
        );
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
}
