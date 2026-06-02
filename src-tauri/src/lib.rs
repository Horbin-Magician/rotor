mod command;

use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;
#[cfg(debug_assertions)]
use tauri_plugin_log::fern::colors::ColoredLevelConfig;
use tauri_plugin_log::{Target, TargetKind};

use command::{core_cmd, quick_cmd, screen_shotter_cmd, searcher_cmd};

fn format_plain_log(
    out: tauri_plugin_log::fern::FormatCallback,
    message: &std::fmt::Arguments<'_>,
    record: &log::Record<'_>,
) {
    out.finish(format_args!(
        "[{}][{}][{}] {}",
        chrono::Local::now().format("%Y-%m-%d][%H:%M:%S"),
        record.level(),
        record.target(),
        message
    ))
}

fn log_targets() -> Vec<Target> {
    let stdout_target = {
        #[cfg(debug_assertions)]
        {
            let colors = ColoredLevelConfig::new()
                .trace(tauri_plugin_log::fern::colors::Color::BrightBlack)
                .debug(tauri_plugin_log::fern::colors::Color::Blue)
                .info(tauri_plugin_log::fern::colors::Color::Green)
                .warn(tauri_plugin_log::fern::colors::Color::Yellow)
                .error(tauri_plugin_log::fern::colors::Color::Red);

            Target::new(TargetKind::Stdout).format(move |out, message, record| {
                out.finish(format_args!(
                    "[{}][{}][{}] {}",
                    chrono::Local::now().format("%Y-%m-%d][%H:%M:%S"),
                    colors.color(record.level()),
                    record.target(),
                    message
                ))
            })
        }

        #[cfg(not(debug_assertions))]
        {
            Target::new(TargetKind::Stdout).format(format_plain_log)
        }
    };

    let log_file_target = Target::new(TargetKind::Folder {
        path: rotor_platform::file_util::get_userdata_path()
            .unwrap_or_else(|| std::path::PathBuf::from("./")),
        file_name: Some("logs".to_string()),
    })
    .format(format_plain_log);

    let app_log_dir_target =
        Target::new(TargetKind::LogDir { file_name: None }).format(format_plain_log);

    vec![
        stdout_target,
        app_log_dir_target,
        log_file_target,
    ]
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    if rotor_platform::sys_util::run_as_admin().unwrap_or_else(|e| {
        log::error!("run_as_admin error: {:?}", e);
        true
    }) {
        return;
    }

    rotor_platform::file_util::del_useless_files().unwrap_or_else(|e| {
        log::error!("del_useless_files error: {:?}", e);
    });

    let app = match tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .targets(log_targets())
                .level(log::LevelFilter::Debug)
                .timezone_strategy(tauri_plugin_log::TimezoneStrategy::UseLocal)
                .clear_format()
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
                .with_handler(rotor_runtime::handle_global_hotkey_event)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            core_cmd::get_cfg,
            core_cmd::get_all_cfg,
            core_cmd::set_cfg,
            core_cmd::get_app_version,
            core_cmd::get_ws_port,
            core_cmd::get_overview_info,
            core_cmd::take_shortcut_registration_notices,
            core_cmd::open_url,
            quick_cmd::get_quick_actions,
            quick_cmd::set_quick_actions,
            quick_cmd::run_quick_action,
            screen_shotter_cmd::new_pin,
            screen_shotter_cmd::save_img,
            screen_shotter_cmd::get_screen_rects,
            screen_shotter_cmd::change_current_mask,
            screen_shotter_cmd::get_pin_state,
            screen_shotter_cmd::clear_screenshot_cache,
            screen_shotter_cmd::close_cache_pin,
            screen_shotter_cmd::new_cache_pin,
            screen_shotter_cmd::update_pin_state,
            screen_shotter_cmd::update_pin_selection,
            screen_shotter_cmd::delete_pin_record,
            screen_shotter_cmd::img2text,
            searcher_cmd::searcher_find,
            searcher_cmd::searcher_release,
            searcher_cmd::searcher_index_status,
            searcher_cmd::open_file,
            searcher_cmd::open_file_as_admin,
        ])
        .setup(move |app| {
            #[cfg(target_os = "macos")]
            app.set_dock_visibility(false);
            Ok(())
        })
        .build(tauri::generate_context!())
    {
        Ok(app) => app,
        Err(e) => {
            log::error!("Build tauri::application error: {e}");
            return;
        }
    };

    if let Err(e) = rotor_runtime::Application::lock_global().init(app.app_handle().clone()) {
        log::error!("Error while init rotor application: {e}");
    }

    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
            if code.is_none() {
                api.prevent_exit();
                for (_label, window) in app.webview_windows() {
                    if let Err(e) = window.close() {
                        log::warn!("Failed to close window during exit request: {e}");
                    }
                }
            }
        }
    });
}
