use std::str::FromStr;
use std::sync::{LazyLock, Mutex};
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use rotor_common::{AppConfig, Config};
use rotor_runtime::Application;

static GLOBAL_SHORTCUT_UPDATE: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewInfo {
    memory: rotor_platform::sys_util::MemoryUsage,
    search_index: rotor_searcher::file_data::SearchIndexStatus,
    permissions: Vec<rotor_platform::sys_util::PermissionStatus>,
}

#[tauri::command]
pub fn get_all_cfg() -> Config {
    AppConfig::lock_global().get_all()
}

#[tauri::command]
pub fn set_cfg(k: String, mut v: String, app: AppHandle) -> Result<(), String> {
    let tokens = k.split('_').collect::<Vec<&str>>();
    let should_rebuild_search_index = k == "search_excluded_dirs";
    let is_global_shortcut = is_global_shortcut_key(&k);
    let _shortcut_update_guard = is_global_shortcut.then(|| {
        GLOBAL_SHORTCUT_UPDATE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    });
    let old_value = AppConfig::lock_global().get(&k).cloned();
    let mut registered_shortcut = None;
    let mut old_registered_shortcut = None;

    if tokens[0] == "shortcut" {
        match Shortcut::from_str(&v) {
            Ok(shortcut) => {
                v = shortcut.to_string();
                if old_value.as_deref() == Some(v.as_str()) {
                    return Ok(());
                }

                if is_global_shortcut {
                    let old_shortcut = old_value.as_deref().and_then(|value| {
                        Shortcut::from_str(value)
                            .map_err(|error| {
                                log::warn!("Invalid old shortcut `{value}`: {error}");
                            })
                            .ok()
                    });

                    if let Some(old_shortcut) = old_shortcut {
                        if let Err(error) = app.global_shortcut().unregister(old_shortcut) {
                            log::warn!(
                                "Failed to unregister old shortcut `{old_shortcut}`: {error}"
                            );
                        }
                    }

                    if let Err(error) = app.global_shortcut().register(shortcut) {
                        log::error!("Failed to register new shortcut `{shortcut}`: {error}");
                        restore_shortcut(&app, old_shortcut);
                        return Err(format!(
                            "Shortcut `{shortcut}` is unavailable or already in use: {error}"
                        ));
                    }

                    registered_shortcut = Some(shortcut);
                    old_registered_shortcut = old_shortcut;
                }
            }
            Err(error) => {
                if is_global_shortcut {
                    return Err(format!("Invalid shortcut `{v}`: {error}"));
                }
            }
        }
    }

    if old_value.as_deref() == Some(v.as_str()) {
        return Ok(());
    }

    let save_result = {
        let mut app_config = AppConfig::lock_global();
        app_config.set(k.clone(), v)
    };
    if let Err(error) = save_result {
        if let Some(shortcut) = registered_shortcut {
            if let Err(rollback_error) = app.global_shortcut().unregister(shortcut) {
                log::warn!("Failed to rollback shortcut `{shortcut}`: {rollback_error}");
            }
            restore_shortcut(&app, old_registered_shortcut);
        }

        let message = format!("Command set_cfg error: {error}");
        log::error!("{message}");
        return Err(message);
    }

    if let Some(shortcut) = registered_shortcut {
        Application::lock_global().update_module_shortcut(&k, shortcut);
    }

    if should_rebuild_search_index {
        Application::lock_global().searcher.rebuild_index();
    }
    Ok(())
}

fn restore_shortcut(app: &AppHandle, shortcut: Option<Shortcut>) {
    if let Some(shortcut) = shortcut {
        if let Err(error) = app.global_shortcut().register(shortcut) {
            log::error!("Failed to restore old shortcut `{shortcut}`: {error}");
        }
    }
}

/// Global shortcuts are all `shortcut_*` keys except the pin-window local
/// shortcuts (`shortcut_pinwin_*`), which are only persisted as config.
fn is_global_shortcut_key(k: &str) -> bool {
    k.starts_with("shortcut_") && !k.starts_with("shortcut_pinwin_")
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

fn collect_overview_info() -> OverviewInfo {
    let memory = rotor_platform::sys_util::get_memory_usage().unwrap_or_else(|error| {
        log::warn!("Failed to collect memory usage: {error}");
        rotor_platform::sys_util::MemoryUsage { resident_bytes: 0 }
    });

    let search_index_reader = {
        let app_state = Application::lock_global();
        app_state.searcher.index_status_reader()
    };
    let search_index = search_index_reader.index_status();
    let permissions = rotor_platform::sys_util::get_permission_statuses();

    OverviewInfo {
        memory,
        search_index,
        permissions,
    }
}

#[tauri::command]
pub async fn get_overview_info() -> Result<OverviewInfo, String> {
    tauri::async_runtime::spawn_blocking(collect_overview_info)
        .await
        .map_err(|error| format!("Failed to collect overview info: {error}"))
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
