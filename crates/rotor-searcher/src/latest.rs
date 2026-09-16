use std::sync::{Arc, Condvar, Mutex};

struct Slot<T> {
    value: Option<T>,
    closed: bool,
}
struct Shared<T> {
    slot: Mutex<Slot<T>>,
    ready: Condvar,
}
pub(crate) struct Sender<T>(Arc<Shared<T>>);
pub(crate) struct Receiver<T>(Arc<Shared<T>>);

/// A worker owns at most one pending job. Replacing a stale job drops its
/// result sender, allowing its cancelled coordinator to disconnect immediately.
pub(crate) fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let shared = Arc::new(Shared {
        slot: Mutex::new(Slot {
            value: None,
            closed: false,
        }),
        ready: Condvar::new(),
    });
    (Sender(shared.clone()), Receiver(shared))
}
impl<T> Sender<T> {
    pub fn send(&self, value: T) -> Result<(), ()> {
        let mut slot = self
            .0
            .slot
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if slot.closed {
            return Err(());
        }
        slot.value = Some(value);
        self.0.ready.notify_one();
        Ok(())
    }
}
impl<T> Receiver<T> {
    pub fn recv(&self) -> Result<T, ()> {
        let mut slot = self
            .0
            .slot
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        loop {
            if let Some(value) = slot.value.take() {
                return Ok(value);
            }
            if slot.closed {
                return Err(());
            }
            slot = self
                .0
                .ready
                .wait(slot)
                .unwrap_or_else(|error| error.into_inner());
        }
    }
}
impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut slot = self
            .0
            .slot
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        slot.closed = true;
        slot.value = None;
        self.0.ready.notify_one();
    }
}
impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        let mut slot = self
            .0
            .slot
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        slot.closed = true;
        slot.value = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn newest_job_replaces_pending_work_and_drop_wakes_receiver() {
        let (sender, receiver) = channel();
        for number in 0..10000 {
            sender.send(number).unwrap();
        }
        assert_eq!(receiver.recv(), Ok(9999));
        drop(sender);
        assert!(receiver.recv().is_err());
        let (sender, receiver) = channel();
        drop(receiver);
        assert!(sender.send(1).is_err());
    }
}
