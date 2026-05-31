use std::str::FromStr;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use rotor_common::{AppConfig, Config};
use rotor_runtime::Application;

#[tauri::command]
pub fn get_all_cfg() -> Config {
    AppConfig::lock_global().get_all()
}

#[tauri::command]
pub fn set_cfg(k: String, mut v: String, app: AppHandle) -> Result<(), String> {
    let tokens = k.split('_').collect::<Vec<&str>>();
    {
        let mut app_config = AppConfig::lock_global();
        let old_value = app_config.get(&k).cloned();
        if tokens[0] == "shortcut" {
            match Shortcut::from_str(&v) {
                Ok(shortcut) => {
                    v = shortcut.to_string();
                    if old_value
                        .as_deref()
                        .and_then(|old_shortcut| Shortcut::from_str(old_shortcut).ok())
                        .is_some_and(|old_shortcut| old_shortcut == shortcut)
                    {
                        return Ok(());
                    }

                    if tokens.len() == 2 {
                        let old_shortcut =
                            old_value
                                .as_deref()
                                .and_then(|old_shortcut| match Shortcut::from_str(old_shortcut) {
                                    Ok(old_shortcut) => Some(old_shortcut),
                                    Err(error) => {
                                        log::warn!(
                                            "Invalid old shortcut `{old_shortcut}`: {error}"
                                        );
                                        None
                                    }
                                });

                        if let Some(old_shortcut) = old_shortcut {
                            app.global_shortcut()
                                .unregister(old_shortcut)
                                .unwrap_or_else(|e| {
                                    log::error!("Failed to unregister old shortcut: {e}");
                                });
                        }

                        if let Err(error) = app.global_shortcut().register(shortcut) {
                            log::error!("Failed to register new shortcut `{shortcut}`: {error}");

                            if let Some(old_shortcut) = old_shortcut {
                                app.global_shortcut()
                                    .register(old_shortcut)
                                    .unwrap_or_else(|rollback_error| {
                                        log::error!(
                                            "Failed to restore old shortcut `{old_shortcut}`: {rollback_error}"
                                        );
                                    });
                            }

                            return Err(format!(
                                "Shortcut `{shortcut}` is unavailable or already in use: {error}"
                            ));
                        }
                    }
                }
                Err(error) => {
                    if tokens.len() == 2 {
                        return Err(format!("Invalid shortcut `{v}`: {error}"));
                    }
                }
            }
        }
        if old_value.as_deref() == Some(v.as_str()) {
            return Ok(());
        }
        app_config.set(k, v).map_err(|e| {
            let error = format!("Command set_cfg error: {e}");
            log::error!("{error}");
            error
        })?;
    }
    Ok(())
}

#[tauri::command]
pub fn get_cfg(k: String) -> String {
    if let Some(config) = AppConfig::lock_global().get(&k) {
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
    Application::lock_global().ws_port
}

#[tauri::command]
pub fn take_shortcut_registration_notices() -> Vec<rotor_runtime::ShortcutRegistrationNotice> {
    Application::lock_global().take_shortcut_registration_notices()
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
