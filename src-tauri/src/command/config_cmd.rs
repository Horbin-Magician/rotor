use crate::core::config::{AppConfig, Config};
use crate::util::log_util;

#[tauri::command]
pub fn get_all_cfg() -> Config {
    AppConfig::global().lock().unwrap().get_all()
}

#[tauri::command]
pub fn set_cfg(k: String, v: String) {
    AppConfig::global()
        .lock()
        .unwrap()
        .set(k, v)
        .unwrap_or_else(|e| {
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
