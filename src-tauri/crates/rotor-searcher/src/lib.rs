pub mod file_data;

use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use file_data::{
    FileData, FileState, SearchIndexStatus, SearchResultItem, SearcherMessage, SharedFileState,
};

pub struct Searcher {
    searcher_msg_sender: mpsc::Sender<SearcherMessage>,
    search_index_state: SharedFileState,
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
                    state: state.as_str().to_string(),
                    ..SearchIndexStatus::empty()
                }
            })
    }
}

impl Searcher {
    pub fn new<F>(
        find_result_callback: F,
        state_change_callback: Option<Box<dyn Fn(String) + Send>>,
    ) -> Searcher
    where
        F: Fn(String, Vec<SearchResultItem>, bool) + Send + 'static,
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
        }
    }

    pub fn update(&self) {
        let _ = self.searcher_msg_sender.send(SearcherMessage::Update);
    }

    pub fn find(&self, filename: String) {
        let _ = self
            .searcher_msg_sender
            .send(SearcherMessage::Find(filename));
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
