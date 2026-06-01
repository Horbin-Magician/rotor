pub mod file_data;

use std::error::Error;
use std::str::FromStr;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::Shortcut;

use file_data::{
    FileData, FileState, SearchIndexStatus, SearchResultItem, SearcherMessage, SharedFileState,
};
use rotor_common::AppConfig;

pub struct Searcher {
    app_hander: Option<tauri::AppHandle>,
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
    pub fn flag(&self) -> &str {
        "searcher"
    }

    pub fn init(&mut self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        self.app_hander = Some(app.clone());
        self.build_window()?;
        Ok(())
    }

    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let _ = self.searcher_msg_sender.send(SearcherMessage::Update);

        let app_handle = match &self.app_hander {
            Some(handle) => handle,
            None => return Err("AppHandle not initialized".into()),
        };

        if let Some(window) = app_handle.get_webview_window("searcher") {
            window.show()?;
            window.set_focus()?;
        }

        Ok(())
    }

    pub fn get_shortcut(&self) -> Option<Shortcut> {
        let app_config = AppConfig::lock_global();
        let shortcut = app_config.get("shortcut_search").cloned();
        drop(app_config);
        if let Some(shortcut_str) = shortcut {
            match Shortcut::from_str(&shortcut_str) {
                Ok(shortcut) => return Some(shortcut),
                Err(error) => {
                    log::warn!("Invalid search shortcut `{shortcut_str}`: {error}");
                    return None;
                }
            }
        }
        None
    }

    pub fn new<F>(find_result_callback: F) -> Searcher
    where
        F: Fn(String, Vec<SearchResultItem>, bool) + Send + 'static,
    {
        let (searcher_msg_sender, searcher_msg_receiver) = mpsc::channel::<SearcherMessage>();
        let search_index_state = Arc::new(Mutex::new(FileState::Unbuild));

        let _file_data = FileData::new(find_result_callback, search_index_state.clone());
        FileData::event_loop(searcher_msg_receiver, _file_data);
        let _ = searcher_msg_sender.send(SearcherMessage::Init);

        Searcher {
            app_hander: None,
            searcher_msg_sender,
            search_index_state,
        }
    }

    fn build_window(&self) -> Result<(), Box<dyn Error>> {
        if let Some(ref app) = self.app_hander {
            let mut win_builder =
                WebviewWindowBuilder::new(app, "searcher", WebviewUrl::App("Searcher".into()))
                    .always_on_top(true)
                    .resizable(false)
                    .visible(false);

            #[cfg(target_os = "windows")]
            {
                win_builder = win_builder.decorations(false).skip_taskbar(true);
            }

            #[cfg(target_os = "macos")]
            {
                win_builder = win_builder
                    .hidden_title(true)
                    .title_bar_style(tauri::TitleBarStyle::Overlay)
                    .traffic_light_position(tauri::LogicalPosition { x: (0), y: (-100) });
            }

            let _window = win_builder.build()?;
            Ok(())
        } else {
            Err("AppHandle not initialized".into())
        }
    }

    pub fn find(&self, filename: String) {
        let _ = self
            .searcher_msg_sender
            .send(SearcherMessage::Find(filename));
    }

    pub fn release(&self) {
        let _ = self.searcher_msg_sender.send(SearcherMessage::Release);
    }

    pub fn index_status_reader(&self) -> SearchIndexStatusReader {
        SearchIndexStatusReader {
            searcher_msg_sender: self.searcher_msg_sender.clone(),
            search_index_state: self.search_index_state.clone(),
        }
    }
}
