//! Shared "latest request wins" primitives for the asynchronous services.
//!
//! Every feature that can be superseded (translation, chat, provider tests,
//! capture) used to carry its own `AtomicU64` plus `Mutex<Option<JoinHandle>>`
//! and re-derive the same three rules. They live here once:
//! a worker publishes only while its request is still current, cancellation
//! advances the identity so unfinished work is dropped rather than shown, and
//! starting a new request aborts the previous task of the same kind.
use super::{lock, next_operation, OperationId};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};
use tokio::task::JoinHandle;

/// Identity of the newest accepted request of one kind.
///
/// Workers compare against it before publishing. Cancelling a request that
/// cannot be interrupted (an in-flight OS capture, for example) only advances
/// this value; the work finishes but its result is never published.
#[derive(Clone, Default)]
pub(super) struct Latest(Arc<AtomicU64>);

impl Latest {
    pub fn claim(&self, id: OperationId) {
        self.0.store(id.0, Ordering::Release);
    }

    pub fn is_current(&self, id: OperationId) -> bool {
        self.0.load(Ordering::Acquire) == id.0
    }

    /// Supersede whatever is in flight with an identity nobody holds.
    pub fn invalidate(&self) {
        self.claim(next_operation());
    }

    #[cfg(test)]
    pub fn current(&self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
}

/// At most one running task per kind. Starting a new request aborts the
/// previous task; cancellation is scoped to a request identity so a stale
/// caller (a closing window) cannot cancel a newer request.
#[derive(Default)]
pub(super) struct SingleFlight {
    current: Latest,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl SingleFlight {
    #[cfg(test)]
    fn latest(&self) -> &Latest {
        &self.current
    }

    /// Admit and start a request while holding the flight lock, so a
    /// concurrent shutdown cannot slip a task in after `admit` fails.
    pub fn begin(
        &self,
        admit: impl FnOnce() -> Result<(), String>,
        spawn: impl FnOnce(OperationId, Latest) -> JoinHandle<()>,
    ) -> Result<OperationId, String> {
        let mut task = lock(&self.task);
        admit()?;
        let id = next_operation();
        self.current.claim(id);
        if let Some(previous) = task.take() {
            previous.abort();
        }
        *task = Some(spawn(id, self.current.clone()));
        Ok(id)
    }

    /// Abort the running task when `id` is the current request, or
    /// unconditionally when `id` is `None`.
    pub fn cancel(&self, id: Option<OperationId>) {
        let mut task = lock(&self.task);
        if id.is_none_or(|id| self.current.is_current(id)) {
            self.current.invalidate();
            if let Some(task) = task.take() {
                task.abort();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_tracks_only_the_newest_claim_and_invalidation_orphans_it() {
        let latest = Latest::default();
        let first = next_operation();
        let second = next_operation();
        latest.claim(first);
        assert!(latest.is_current(first));
        latest.claim(second);
        assert!(!latest.is_current(first));
        assert!(latest.is_current(second));
        latest.invalidate();
        assert!(!latest.is_current(second));
    }

    #[test]
    fn single_flight_aborts_predecessors_and_respects_request_scoped_cancel() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let flight = SingleFlight::default();
        let first = flight
            .begin(|| Ok(()), |_, _| runtime.spawn(std::future::pending()))
            .unwrap();
        let second = flight
            .begin(|| Ok(()), |_, _| runtime.spawn(std::future::pending()))
            .unwrap();
        assert!(!flight.latest().is_current(first));
        assert!(flight.latest().is_current(second));
        // A stale identity must not cancel the newer request.
        flight.cancel(Some(first));
        assert!(flight.latest().is_current(second));
        assert!(lock(&flight.task).is_some());

        flight.cancel(Some(second));
        assert!(!flight.latest().is_current(second));
        assert!(lock(&flight.task).is_none());

        // Admission failures leave no task behind and report the error.
        assert_eq!(
            flight.begin(|| Err("stopped".into()), |_, _| runtime.spawn(async {})),
            Err("stopped".to_string())
        );
        assert!(lock(&flight.task).is_none());
    }
}
