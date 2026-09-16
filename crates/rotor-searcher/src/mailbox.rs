use crate::file_data::SearcherMessage;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
};

const CAPACITY: usize = 64;
struct Queue {
    messages: VecDeque<SearcherMessage>,
    active: Option<(Arc<AtomicBool>, bool)>,
    closed: bool,
}
struct Shared {
    queue: Mutex<Queue>,
    ready: Condvar,
    senders: AtomicUsize,
}
pub(crate) struct Sender(Arc<Shared>);
pub(crate) struct Receiver(Arc<Shared>);

pub(crate) fn channel() -> (Sender, Receiver) {
    let shared = Arc::new(Shared {
        queue: Mutex::new(Queue {
            messages: VecDeque::new(),
            active: None,
            closed: false,
        }),
        ready: Condvar::new(),
        senders: AtomicUsize::new(1),
    });
    (Sender(shared.clone()), Receiver(shared))
}

fn cancels(message: &SearcherMessage, searching: bool) -> bool {
    matches!(
        message,
        SearcherMessage::Shutdown | SearcherMessage::Release | SearcherMessage::Init
    ) || searching
}

impl Sender {
    pub fn send(&self, message: SearcherMessage) -> Result<(), ()> {
        let mut queue = self
            .0
            .queue
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if queue.closed {
            return Err(());
        }
        if matches!(message, SearcherMessage::Find(_) | SearcherMessage::Release) {
            queue
                .messages
                .retain(|message| !matches!(message, SearcherMessage::Find(_)));
        }
        if matches!(message, SearcherMessage::Shutdown) {
            queue.messages.clear();
        }
        if queue.messages.len() >= CAPACITY {
            return Err(());
        }
        if let Some((cancel, searching)) = &queue.active {
            if cancels(&message, *searching) {
                cancel.store(true, Ordering::Release);
            }
        }
        queue.messages.push_back(message);
        self.0.ready.notify_one();
        Ok(())
    }
}

impl Clone for Sender {
    fn clone(&self) -> Self {
        self.0.senders.fetch_add(1, Ordering::Relaxed);
        Self(self.0.clone())
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        if self.0.senders.fetch_sub(1, Ordering::AcqRel) == 1 {
            let mut queue = self
                .0
                .queue
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            queue.closed = true;
            if let Some((cancel, _)) = &queue.active {
                cancel.store(true, Ordering::Release);
            }
            self.0.ready.notify_one();
        }
    }
}

impl Receiver {
    pub fn recv(&self) -> Result<SearcherMessage, std::sync::mpsc::RecvError> {
        let mut queue = self
            .0
            .queue
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        loop {
            if let Some(message) = queue.messages.pop_front() {
                return Ok(message);
            }
            if queue.closed {
                return Err(std::sync::mpsc::RecvError);
            }
            queue = self
                .0
                .ready
                .wait(queue)
                .unwrap_or_else(|error| error.into_inner());
        }
    }

    pub fn try_recv(&self) -> Result<SearcherMessage, std::sync::mpsc::TryRecvError> {
        let mut queue = self
            .0
            .queue
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        queue.messages.pop_front().ok_or(if queue.closed {
            std::sync::mpsc::TryRecvError::Disconnected
        } else {
            std::sync::mpsc::TryRecvError::Empty
        })
    }

    /// Register under the same lock as enqueueing so a command arriving between
    /// dequeue and dispatch cannot miss cancellation.
    pub fn activate(&self, cancel: Arc<AtomicBool>, searching: bool) {
        let mut queue = self
            .0
            .queue
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if queue.closed
            || queue
                .messages
                .iter()
                .any(|message| cancels(message, searching))
        {
            cancel.store(true, Ordering::Release);
        }
        queue.active = Some((cancel, searching));
    }

    pub fn deactivate(&self) {
        self.0
            .queue
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .active = None;
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        let mut queue = self
            .0
            .queue
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        queue.closed = true;
        queue.messages.clear();
        if let Some((cancel, _)) = &queue.active {
            cancel.store(true, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{QueryId, SearchRequest};

    fn query(id: u64) -> SearcherMessage {
        SearcherMessage::Find(SearchRequest {
            id: QueryId(id),
            append: false,
            query: id.to_string(),
        })
    }

    #[test]
    fn rapid_input_retains_only_latest_query_and_preserves_controls() {
        let (sender, receiver) = channel();
        sender.send(SearcherMessage::Update).unwrap();
        for id in 0..10000 {
            sender.send(query(id)).unwrap();
        }
        assert!(matches!(receiver.recv(), Ok(SearcherMessage::Update)));
        assert!(
            matches!(receiver.recv(), Ok(SearcherMessage::Find(request)) if request.id == QueryId(9999))
        );
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn close_cancels_build_and_removes_pending_search() {
        let (sender, receiver) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        receiver.activate(cancel.clone(), false);
        sender.send(query(1)).unwrap();
        assert!(!cancel.load(Ordering::Acquire));
        sender.send(SearcherMessage::Release).unwrap();
        assert!(cancel.load(Ordering::Acquire));
        assert!(matches!(receiver.recv(), Ok(SearcherMessage::Release)));
        assert!(receiver.try_recv().is_err());
    }

    #[test]
    fn cancellation_registration_covers_dispatch_race_and_disconnect() {
        let (sender, receiver) = channel();
        sender.send(query(2)).unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        receiver.activate(cancel.clone(), true);
        assert!(cancel.load(Ordering::Acquire));
        receiver.recv().unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        receiver.activate(cancel.clone(), false);
        drop(sender);
        assert!(cancel.load(Ordering::Acquire));
        assert!(receiver.recv().is_err());
    }

    #[test]
    fn shutdown_is_accepted_even_when_control_queue_is_full() {
        let (sender, receiver) = channel();
        for _ in 0..CAPACITY {
            sender.send(SearcherMessage::Update).unwrap();
        }
        assert!(sender.send(SearcherMessage::Update).is_err());
        sender.send(SearcherMessage::Shutdown).unwrap();
        assert!(matches!(receiver.recv(), Ok(SearcherMessage::Shutdown)));
    }
}
