use std::str::FromStr;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::core::config::{AppConfig, Config};
use crate::core::application::Application;

#[tauri::command]
pub fn get_all_cfg() -> Config {
    AppConfig::global().lock().unwrap().get_all()
}

#[tauri::command]
pub fn set_cfg(k: String, mut v: String, app: AppHandle) {
    let tokens = k.split('_').collect::<Vec<&str>>();
    {
        let mut app_config = AppConfig::global().lock().unwrap();
        if tokens[0] == "shortcut" {
            if let Ok(shortcut) = Shortcut::from_str(&v) {
                v = shortcut.to_string();
                if tokens.len() == 2 {
                    if let Some(old_shortcut) = app_config.get(&k) {
                        app.global_shortcut()
                            .unregister(Shortcut::from_str(old_shortcut).unwrap())
                            .unwrap_or_else(|e| {
                                log::error!("Failed to unregister old shortcut: {e}");
                            });
                    }
                    app.global_shortcut()
                        .register(shortcut)
                        .unwrap_or_else(|e| {
                            log::error!("Failed to register new shortcut: {e}");
                        });
                }
            }
        }
        app_config.set(k, v).unwrap_or_else(|e| {
            log::error!("Command set_cfg error: {e}");
        });
    }
}

#[tauri::command]
pub fn get_cfg(k: String) -> String {
    if let Some(config) = AppConfig::global().lock().unwrap().get(&k) {
        return config.clone();
    }
    "".to_string()
}

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn get_ws_port() -> u16 {
    Application::global().lock().unwrap().ws_port
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        
        std::process::Command::new("cmd")
            .args(["/C", "start", &url])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&url)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {}", e))?;
    }

    Ok(())
}
