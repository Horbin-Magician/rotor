mod command;
mod core;
mod module;
mod util;

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

use command::{config_cmd, screen_shotter_cmd};
use core::application;
use util::log_util;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {
            // You can handle the single instance event here if needed
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(application::handle_global_hotkey_event)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            config_cmd::get_cfg,
            config_cmd::get_all_cfg,
            config_cmd::set_cfg,
            screen_shotter_cmd::capture_screen,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            let _ = app.handle().set_dock_visibility(false);
            Ok(())
        })
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| {
            log_util::log_error(format!("Build tauri::application error: {:?}", e));
            panic!()
        });

    application::Application::global()
        .lock()
        .unwrap()
        .init(app.app_handle().clone())
        .unwrap_or_else(|e| {
            log_util::log_error(format!("Error while init rotor application: {:?}", e));
            panic!()
        });

    app.run(|app, event| match event {
        tauri::RunEvent::ExitRequested { code, api, .. } => {
            if code == None {
                api.prevent_exit();
                for (_label, window) in app.webview_windows() {
                    window.close().unwrap();
                }
            }
        }
        _ => (),
    });
}
