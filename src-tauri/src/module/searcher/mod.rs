mod file_data;

use crate::core::config::AppConfig;
use crate::module::Module;
use std::{any::Any, sync::mpsc};
use std::error::Error;
use std::str::FromStr;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::Shortcut;

use file_data::{FileData, SearcherMessage, SearchResultItem};

pub struct Searcher {
    app_hander: Option<tauri::AppHandle>,
    searcher_msg_sender: mpsc::Sender<SearcherMessage>,
}

impl Module for Searcher {
    fn flag(&self) -> &str {
        "searcher"
    }

    fn init(&mut self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        self.app_hander = Some(app.clone());
        self.build_window()?;
        Ok(())
    }

    fn run(&mut self) -> Result<(), Box<dyn Error>> {
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

    fn get_shortcut(&self) -> Option<Shortcut> {
        let app_config = AppConfig::global().lock().unwrap();
        let shortcut = app_config.get(&"shortcut_search".to_string()).cloned();
        drop(app_config);
        if let Some(shortcut_str) = shortcut {
            return Some(Shortcut::from_str(&shortcut_str).unwrap());
        }
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Searcher {
    pub fn new() -> Result<Searcher, Box<dyn Error>> {
        let (searcher_msg_sender, searcher_msg_receiver) = mpsc::channel::<SearcherMessage>();

        let _file_data = FileData::new(Searcher::update_result_model);
        FileData::event_loop(searcher_msg_receiver, _file_data);
        let _ = searcher_msg_sender.send(SearcherMessage::Init);

        Ok(Searcher {
            app_hander: None,
            searcher_msg_sender,
        })
    }

    fn build_window(&self) -> Result<(), Box<dyn Error>> {
        if let Some(ref app) = self.app_hander {
            let mut win_builder = WebviewWindowBuilder::new(
                app,
                "searcher",
                WebviewUrl::App("Searcher".into()),
            )
            .always_on_top(true)
            .resizable(false)
            .visible(false);

            #[cfg(target_os = "windows")]
            {
                win_builder = win_builder
                    .decorations(false)
                    .skip_taskbar(true);
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

    fn find(&self, filename: String) {
        let _ = self.searcher_msg_sender.send(SearcherMessage::Find(filename));
    }

    fn release(&self, filename: String) {
        let _ = self.searcher_msg_sender.send(SearcherMessage::Release);
    }

    fn update_result_model(filename: String, update_result: Vec<SearchResultItem>, increment_find: bool) {
        for result in update_result {
            println!("{:?}", result.file_name);
        }
        // if let Some(app_handle) = &self.app_hander {
        //     app_handle.emit("searcher-update-result", reply).unwrap();
        // }
    }
}
