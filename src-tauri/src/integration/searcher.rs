use rotor_common::AppConfig;
use rotor_searcher::file_data::SearchResultItem;
use std::{error::Error, str::FromStr};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::Shortcut;

pub struct Searcher {
    service: rotor_searcher::Searcher,
    app_hander: Option<tauri::AppHandle>,
}
impl std::ops::Deref for Searcher {
    type Target = rotor_searcher::Searcher;
    fn deref(&self) -> &Self::Target {
        &self.service
    }
}
impl Searcher {
    pub fn find(&self, query: String) {
        if let Err(error) = self.service.find(query) { log::warn!("Search request failed: {error}"); }
    }
    pub fn new<F>(callback: F, state_callback: Option<Box<dyn Fn(String) + Send>>) -> Self
    where
        F: Fn(String, Vec<SearchResultItem>, bool) + Send + 'static,
    {
        Self {
            service: rotor_searcher::Searcher::new(
                move |batch| callback(batch.query, batch.items, batch.append),
                state_callback.map(|callback| Box::new(move |state: rotor_searcher::IndexState| callback(state.as_str().to_string())) as Box<dyn Fn(rotor_searcher::IndexState) + Send>),
            ),
            app_hander: None,
        }
    }
    pub fn flag(&self) -> &str {
        "searcher"
    }
    pub fn init(&mut self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        self.app_hander = Some(app.clone());
        self.build_window()?;
        Ok(())
    }
    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        self.service.update();

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
}
