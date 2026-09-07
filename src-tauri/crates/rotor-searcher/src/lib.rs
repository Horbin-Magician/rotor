pub mod file_data;
mod request;
pub use file_data::FileState as IndexState;
pub use request::{QueryId, SearchBatch, SearchRequest, SearchUnavailable};

use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};
use std::time::Duration;

use file_data::{FileData, FileState, SearchIndexStatus, SearcherMessage, SharedFileState};

static NEXT_QUERY: AtomicU64 = AtomicU64::new(1);

pub struct Searcher {
    searcher_msg_sender: mpsc::Sender<SearcherMessage>,
    search_index_state: SharedFileState,
    stopped: AtomicBool,
}

#[derive(Clone)]
pub struct SearchIndexStatusReader {
    searcher_msg_sender: mpsc::Sender<SearcherMessage>,
    search_index_state: SharedFileState,
}

impl SearchIndexStatusReader {
    pub fn index_status(&self) -> SearchIndexStatus {
        let (sender, receiver) = mpsc::channel();

        if self
            .searcher_msg_sender
            .send(SearcherMessage::Status(sender))
            .is_err()
        {
            log::warn!("Failed to request search index status");
            return SearchIndexStatus::empty();
        }

        receiver
            .recv_timeout(Duration::from_millis(800))
            .unwrap_or_else(|error| {
                log::warn!("Failed to receive search index status: {error}");
                let state = *self.search_index_state.lock().unwrap_or_else(|poisoned| {
                    log::error!("Search index state lock poisoned; recovering inner state");
                    poisoned.into_inner()
                });

                SearchIndexStatus {
                    state,
                    ..SearchIndexStatus::empty()
                }
            })
    }
}

impl Searcher {
    pub fn new<F>(
        find_result_callback: F,
        state_change_callback: Option<Box<dyn Fn(IndexState) + Send>>,
    ) -> Searcher
    where
        F: Fn(SearchBatch) + Send + 'static,
    {
        let (searcher_msg_sender, searcher_msg_receiver) = mpsc::channel::<SearcherMessage>();
        let search_index_state = Arc::new(Mutex::new(FileState::Unbuild));

        let _file_data = FileData::new(
            find_result_callback,
            state_change_callback,
            search_index_state.clone(),
        );
        FileData::event_loop(searcher_msg_receiver, _file_data);
        let _ = searcher_msg_sender.send(SearcherMessage::Init);

        Searcher {
            searcher_msg_sender,
            search_index_state,
            stopped: AtomicBool::new(false),
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
            searcher_msg_sender: self.searcher_msg_sender.clone(),
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

    #[test]
    fn typed_index_state_preserves_legacy_json_names() {
        assert_eq!(
            serde_json::to_value(IndexState::Unbuild).unwrap(),
            "unbuilt"
        );
        assert_eq!(
            serde_json::to_value(SearchIndexStatus::empty()).unwrap()["state"],
            "unavailable"
        );
    }

    fn service() -> (Searcher, mpsc::Receiver<SearcherMessage>) {
        let (sender, receiver) = mpsc::channel();
        (
            Searcher {
                searcher_msg_sender: sender,
                search_index_state: Arc::new(Mutex::new(FileState::Ready)),
                stopped: AtomicBool::new(false),
            },
            receiver,
        )
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
