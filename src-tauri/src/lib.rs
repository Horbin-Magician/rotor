mod command;
mod core;
mod module;
mod util;

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

use command::{config_cmd, screen_shotter_cmd, searcher_cmd};
use core::application;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    if util::sys_util::run_as_admin().unwrap_or_else(|e| {
        log::error!("run_as_admin error: {:?}", e);
        true
    }) {
        return;
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Debug)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_single_instance::init(|_app, _argv, _cwd| {}))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(application::handle_global_hotkey_event)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            config_cmd::get_cfg,
            config_cmd::get_all_cfg,
            config_cmd::set_cfg,
            config_cmd::get_app_version,
            config_cmd::open_url,
            screen_shotter_cmd::get_screen_img,
            screen_shotter_cmd::new_pin,
            screen_shotter_cmd::save_img,
            screen_shotter_cmd::get_screen_rects,
            screen_shotter_cmd::change_current_mask,
            screen_shotter_cmd::get_pin_img,
            screen_shotter_cmd::get_pin_state,
            screen_shotter_cmd::close_cache_pin,
            screen_shotter_cmd::update_pin_state,
            screen_shotter_cmd::delete_pin_record,
            searcher_cmd::searcher_find,
            searcher_cmd::searcher_release,
            searcher_cmd::open_file,
            searcher_cmd::open_file_as_admin,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_dock_visibility(false);
            Ok(())
        })
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| {
            log::error!("Build tauri::application error: {e}");
            panic!()
        });

    application::Application::global()
        .lock()
        .unwrap()
        .init(app.app_handle().clone())
        .unwrap_or_else(|e| {
            log::error!("Error while init rotor application: {e}");
            panic!()
        });

    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
            if code.is_none() {
                api.prevent_exit();
                for (_label, window) in app.webview_windows() {
                    window.close().unwrap();
                }
            }
        }
    });
}
