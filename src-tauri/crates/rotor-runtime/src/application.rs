use std::sync::{LazyLock, Mutex};

use rotor_screenshot::ScreenShotter;
use rotor_searcher::{file_data::SearchResultItem, Searcher};
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

use crate::data_server;
use crate::tray::Tray;

pub fn handle_global_hotkey_event(_app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() == ShortcutState::Pressed {
        let mut rotor_app = INSTANCE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if let Some(module_shortcut) = rotor_app.screenshot.get_shortcut() {
            if module_shortcut == *shortcut {
                rotor_app.screenshot.run().unwrap_or_else(|e| {
                    let flag = rotor_app.screenshot.flag();
                    log::error!("Module {flag} run error: {e}")
                });
                return;
            }
        }

        if let Some(module_shortcut) = rotor_app.searcher.get_shortcut() {
            if module_shortcut == *shortcut {
                rotor_app.searcher.run().unwrap_or_else(|e| {
                    let flag = rotor_app.searcher.flag();
                    log::error!("Module {flag} run error: {e}")
                });
            }
        }
    }
}

pub struct Application {
    pub app: Option<AppHandle>,
    pub tray: Tray,
    pub screenshot: ScreenShotter,
    pub searcher: Searcher,
    pub ws_port: u16,
}

impl Application {
    fn new() -> Application {
        Application {
            app: None,
            tray: Tray::new().unwrap(),
            screenshot: ScreenShotter::new().unwrap(),
            searcher: Searcher::new(Application::update_search_result).unwrap(),
            ws_port: 10000,
        }
    }

    pub fn global() -> &'static Mutex<Application> {
        &INSTANCE
    }

    pub fn init(&mut self, app: tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        self.tray.init(&app).unwrap_or_else(|e| {
            let flag = self.tray.flag();
            log::error!("Module {flag} init error: {e}");
        });

        self.screenshot.init(&app).unwrap_or_else(|e| {
            let flag = self.screenshot.flag();
            log::error!("Module {flag} init error: {e}");
        });
        if let Some(shortcut) = self.screenshot.get_shortcut() {
            app.global_shortcut().register(shortcut)?;
        }

        self.searcher.init(&app).unwrap_or_else(|e| {
            let flag = self.searcher.flag();
            log::error!("Module {flag} init error: {e}");
        });
        if let Some(shortcut) = self.searcher.get_shortcut() {
            app.global_shortcut().register(shortcut)?;
        }
        self.app = Some(app);

        tauri::async_runtime::spawn(async move {
            data_server::run().await;
        });

        Ok(())
    }

    fn update_search_result(
        filename: String,
        update_result: Vec<SearchResultItem>,
        if_increase: bool,
    ) {
        let app = Application::global().lock().unwrap();
        if let Some(app_handle) = &app.app {
            app_handle
                .emit_to(
                    "searcher",
                    "update_result",
                    (filename, update_result, if_increase),
                )
                .unwrap();
        }
    }
}

static INSTANCE: LazyLock<Mutex<Application>> = LazyLock::new(|| Mutex::new(Application::new()));
