use std::str::FromStr;

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::core::config::{AppConfig, Config};
use crate::util::log_util;

#[tauri::command]
pub fn get_all_cfg() -> Config {
    AppConfig::global().lock().unwrap().get_all()
}

#[tauri::command]
pub fn set_cfg(k: String, mut v: String, app: AppHandle) {
    let tokens = k.split('_').collect::<Vec<&str>>();
    let mut app_config = AppConfig::global().lock().unwrap();
    if tokens[0] == "shortcut" {
        if let Ok(shortcut) = Shortcut::from_str(&v) {
            v = shortcut.to_string();
            if tokens.len() == 2 {
                if let Some(old_shortcut) = app_config.get(&k) {
                    let _ = app
                        .global_shortcut()
                        .unregister(Shortcut::from_str(old_shortcut).unwrap());
                }
                let _ = app.global_shortcut().register(shortcut);
            }
        }
    }
    app_config.set(k, v).unwrap_or_else(|e| {
        log_util::log_error(format!("Command set_cfg error: {:?}", e));
    });
}

#[tauri::command]
pub fn get_cfg(k: String) -> String {
    if let Some(config) = AppConfig::global().lock().unwrap().get(&k) {
        return config.clone();
    }
    "".to_string()
}
