use tauri::AppHandle;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use rotor_common::{AppConfig, DEFAULT_QUICK_ACTIONS_REVISION};
use rotor_runtime::{Application, QuickAction};

#[tauri::command]
pub fn get_quick_actions() -> Vec<QuickAction> {
    Application::lock_global().quick.actions()
}

#[tauri::command]
pub fn set_quick_actions(actions: Vec<QuickAction>, app: AppHandle) -> Result<(), String> {
    let normalized = rotor_runtime::quick::normalize_actions(actions).map_err(|error| {
        let message = format!("Invalid quick actions: {error}");
        log::error!("{message}");
        message
    })?;

    let (old_actions, old_shortcuts) = {
        let rotor_app = Application::lock_global();
        let old_actions = rotor_app.quick.actions();
        let old_shortcuts = rotor_app
            .quick
            .get_shortcuts()
            .into_iter()
            .map(|(_, shortcut)| shortcut)
            .collect::<Vec<_>>();

        (old_actions, old_shortcuts)
    };

    if normalized == old_actions {
        return Ok(());
    }

    let new_shortcuts = rotor_runtime::quick::parse_shortcuts(&normalized).map_err(|error| {
        let message = format!("Invalid quick action shortcuts: {error}");
        log::error!("{message}");
        message
    })?;

    for shortcut in &old_shortcuts {
        app.global_shortcut()
            .unregister(*shortcut)
            .unwrap_or_else(|error| {
                log::warn!("Failed to unregister old quick action shortcut `{shortcut}`: {error}");
            });
    }

    let mut registered_shortcuts = Vec::new();
    for (_, shortcut) in &new_shortcuts {
        if let Err(error) = app.global_shortcut().register(*shortcut) {
            log::error!("Failed to register quick action shortcut `{shortcut}`: {error}");

            rollback_quick_shortcuts(&app, &registered_shortcuts, &old_shortcuts);

            return Err(format!(
                "Shortcut `{shortcut}` is unavailable or already in use: {error}"
            ));
        }

        registered_shortcuts.push(*shortcut);
    }

    let serialized = serde_json::to_string(&normalized).map_err(|error| {
        let message = format!("Failed to serialize quick actions: {error}");
        log::error!("{message}");
        message
    })?;

    {
        let mut app_config = AppConfig::lock_global();
        if let Err(error) = app_config.set_many([
            ("quick_actions".to_string(), serialized),
            (
                "quick_actions_revision".to_string(),
                DEFAULT_QUICK_ACTIONS_REVISION.to_string(),
            ),
        ]) {
            log::error!("Failed to save quick actions: {error}");
            rollback_quick_shortcuts(&app, &registered_shortcuts, &old_shortcuts);

            return Err(format!("Failed to save quick actions: {error}"));
        }
    }

    Application::lock_global().quick.set_actions(normalized);
    Ok(())
}

#[tauri::command]
pub fn run_quick_action(id: String) -> Result<(), String> {
    Application::lock_global()
        .quick
        .run_action(&id)
        .map_err(|error| {
            let message = format!("Failed to run quick action `{id}`: {error}");
            log::error!("{message}");
            message
        })
}

fn rollback_quick_shortcuts(
    app: &AppHandle,
    registered_shortcuts: &[tauri_plugin_global_shortcut::Shortcut],
    old_shortcuts: &[tauri_plugin_global_shortcut::Shortcut],
) {
    for shortcut in registered_shortcuts {
        app.global_shortcut()
            .unregister(*shortcut)
            .unwrap_or_else(|rollback_error| {
                log::warn!(
                    "Failed to rollback quick action shortcut `{shortcut}`: {rollback_error}"
                );
            });
    }

    for shortcut in old_shortcuts {
        app.global_shortcut()
            .register(*shortcut)
            .unwrap_or_else(|rollback_error| {
                log::error!(
                    "Failed to restore old quick action shortcut `{shortcut}`: {rollback_error}"
                );
            });
    }
}
