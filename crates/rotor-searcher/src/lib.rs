pub mod file_data;
mod icons;
mod latest;
mod mailbox;
mod request;
pub use file_data::FileState as IndexState;
pub use request::{QueryId, SearchBatch, SearchIconBatch, SearchRequest, SearchUnavailable};

use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

use file_data::{FileData, FileState, SearchIndexStatus, SearcherMessage, SharedFileState};

static NEXT_QUERY: AtomicU64 = AtomicU64::new(1);

pub struct Searcher {
    searcher_msg_sender: mailbox::Sender,
    search_index_state: SharedFileState,
    stopped: AtomicBool,
    snapshot: Arc<Mutex<SearchIndexStatus>>,
}

#[derive(Clone)]
pub struct SearchIndexStatusReader {
    snapshot: Arc<Mutex<SearchIndexStatus>>,
    search_index_state: SharedFileState,
}

impl SearchIndexStatusReader {
    pub fn index_status(&self) -> SearchIndexStatus {
        let mut status = self
            .snapshot
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        status.state = *self
            .search_index_state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        status
    }
}

impl Searcher {
    pub fn new<F>(
        find_result_callback: F,
        icon_result_callback: impl Fn(SearchIconBatch) + Send + 'static,
        state_change_callback: Option<Box<dyn Fn(IndexState) + Send>>,
    ) -> Searcher
    where
        F: Fn(SearchBatch) + Send + 'static,
    {
        let (searcher_msg_sender, searcher_msg_receiver) = mailbox::channel();
        let search_index_state = Arc::new(Mutex::new(FileState::Unbuild));

        let mut _file_data = FileData::new(
            find_result_callback,
            state_change_callback,
            search_index_state.clone(),
        );
        _file_data.icons = Some(icons::IconWorker::new(icon_result_callback));
        let snapshot = _file_data.snapshot.clone();
        FileData::event_loop(searcher_msg_receiver, _file_data);
        let _ = searcher_msg_sender.send(SearcherMessage::Startup);

        Searcher {
            searcher_msg_sender,
            search_index_state,
            stopped: AtomicBool::new(false),
            snapshot,
        }
    }

    pub fn update(&self) {
        let _ = self.searcher_msg_sender.send(SearcherMessage::Update);
    }

    pub fn find(&self, filename: String) -> Result<QueryId, SearchUnavailable> {
        if self.stopped.load(Ordering::Acquire) {
            return Err(SearchUnavailable);
        }
        let id = QueryId(NEXT_QUERY.fetch_add(1, Ordering::Relaxed));
        self.searcher_msg_sender
            .send(SearcherMessage::Find(SearchRequest {
                id,
                query: filename,
            }))
            .map_err(|_| SearchUnavailable)?;
        Ok(id)
    }

    pub fn shutdown(&self) {
        self.stopped.store(true, Ordering::Release);
        let _ = self.searcher_msg_sender.send(SearcherMessage::Shutdown);
    }

    pub fn release(&self) {
        let _ = self.searcher_msg_sender.send(SearcherMessage::Release);
    }

    pub fn rebuild_index(&self) {
        let _ = self.searcher_msg_sender.send(SearcherMessage::Init);
    }

    pub fn index_status_reader(&self) -> SearchIndexStatusReader {
        SearchIndexStatusReader {
            snapshot: self.snapshot.clone(),
            search_index_state: self.search_index_state.clone(),
        }
    }
}

impl Drop for Searcher {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service() -> (Searcher, mailbox::Receiver) {
        let (sender, receiver) = mailbox::channel();
        (
            Searcher {
                searcher_msg_sender: sender,
                search_index_state: Arc::new(Mutex::new(FileState::Ready)),
                stopped: AtomicBool::new(false),
                snapshot: Arc::new(Mutex::new(SearchIndexStatus::empty())),
            },
            receiver,
        )
    }

    #[test]
    fn status_reads_snapshot_without_waiting_for_busy_coordinator() {
        let (searcher, _receiver) = service();
        searcher.snapshot.lock().unwrap().index_item_count = 1234;
        for _ in 0..64 {
            searcher.update();
        }
        let status = searcher.index_status_reader().index_status();
        assert_eq!(status.index_item_count, 1234);
        assert_eq!(status.state, FileState::Ready);
    }

    #[test]
    fn ids_do_not_repeat_across_service_instances() {
        let (first, _first_receiver) = service();
        let (second, _second_receiver) = service();
        assert_ne!(
            first.find("same".into()).unwrap(),
            second.find("same".into()).unwrap()
        );
    }

    #[test]
    fn requests_fail_after_shutdown_or_receiver_disconnect() {
        let (first, _receiver) = service();
        first.shutdown();
        assert!(first.find("query".into()).is_err());
        let (second, receiver) = service();
        drop(receiver);
        assert!(second.find("query".into()).is_err());
    }
}
