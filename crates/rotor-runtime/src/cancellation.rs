use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Owned by the requesting view/future. Dropping it invalidates queued work;
/// workers retain only the flag, never a UI entity.
#[derive(Default)]
struct State {
    cancelled: AtomicBool,
    wake: tokio::sync::Notify,
}

#[derive(Clone, Default)]
pub struct CancellationFlag(Arc<State>);

impl CancellationFlag {
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::Acquire)
    }

    pub(crate) async fn cancelled(&self) {
        let notified = self.0.wake.notified();
        tokio::pin!(notified);
        // Register before reading the flag so cancellation cannot be lost
        // between the atomic read and the first poll of the notification.
        notified.as_mut().enable();
        if !self.is_cancelled() {
            notified.await;
        }
    }
}

#[derive(Default)]
pub struct Cancellation(CancellationFlag);

impl Cancellation {
    pub fn flag(&self) -> CancellationFlag {
        self.0.clone()
    }
}
impl Drop for Cancellation {
    fn drop(&mut self) {
        self.0 .0.cancelled.store(true, Ordering::Release);
        self.0 .0.wake.notify_waiters();
    }
}
