use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

use crate::util::log_util;
use crate::module;

pub fn handle_global_hotkey_event(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() == ShortcutState::Pressed {
        let rotor_app = INSTANCE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for module in rotor_app.modules.values() {
            if let Some(module_shortcut) = module.get_shortcut() {
                if module_shortcut == *shortcut {
                    module.run(app)
                        .unwrap_or_else(|e| log_util::log_error(format!("Module {} run error: {:?}", module.flag(), e)));
                }
            }
        }
    }
}

pub struct Application {
    pub app: Option<AppHandle>,
    pub modules: HashMap<String, Box<dyn module::Module + Send + 'static>>,
}

impl Application {
    fn new() -> Application {
        let mut modules = HashMap::new();

        for module in module::get_all_modules() {
            modules.insert(
                module.flag().to_string(),
                module,
            );
        }

        Application { app: None, modules }
    }

    pub fn global() -> &'static Mutex<Application> {
        &INSTANCE
    }

    pub fn init(&mut self, app: tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        for module in self.modules.values_mut() {
            module.init(&app)
                .unwrap_or_else(|e| log_util::log_error(format!("Module {} init error: {:?}", module.flag(), e)));
            if let Some(shortcut) = module.get_shortcut() {
                app.global_shortcut().register(shortcut)?;
            }
        }
        self.app = Some(app);
        Ok(())
    }
}

static INSTANCE: LazyLock<Mutex<Application>> = LazyLock::new(|| Mutex::new(Application::new()));
