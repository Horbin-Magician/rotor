#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod capture;
mod cli;
mod dispatch;
mod logging;
mod pins;
mod placement;
mod shell;
mod system;
#[cfg(target_os = "windows")]
mod tray_menu;

pub(crate) use shell::{
    ShellState, WindowRole, WindowSlot, WindowView, publish_warning, show_settings,
};

use futures::future::{Either, select};
use gpui_kit::*;
use rotor_common::{AppConfig, ConfigService, ResourceLocator, file_path};
use rotor_platform::single_instance::{Instance, InstanceGuard};
use rotor_runtime::{ServiceOptions, Services};
use shell::apply_theme;
use std::{cell::Cell, collections::HashMap, error::Error, path::PathBuf, rc::Rc, sync::Arc};
use system::{Command, CommandBus, SystemServices};

fn run() -> Result<(), Box<dyn Error>> {
    let args = cli::Arguments::from_env();
    if args.is_build_info() {
        println!("{}", rotor_common::native_app::build_info_json());
        return Ok(());
    }
    if args.has("--check-resources") {
        let resources = match args.value("--resource-dir")? {
            Some(path) => ResourceLocator::from_root(std::path::Path::new(&path))?,
            None => ResourceLocator::for_current_process()?,
        };
        resources.verify_native_resources(&rotor_runtime::OCR_MODEL_FILES)?;
        println!(
            "Native resources verified at {}",
            resources.root().display()
        );
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    if let Some(job) = args.value("--apply-update")? {
        if !args.is_exactly(&["--apply-update", job.as_str()]) {
            return Err("Invalid update helper arguments".into());
        }
        return rotor_updater::run_helper(std::path::Path::new(&job)).map_err(Into::into);
    }
    let update_ready = args.value("--update-ready")?.map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let update_warning = args.value("--update-error-file")?.and_then(|path| {
        rotor_updater::handoff_error(std::path::Path::new(&path))
            .unwrap_or_else(|error| Some(format!("Cannot read update result: {error}")))
    });
    let directory = cli::resolve_data_directory(
        args.value("--data-dir")?
            .map(std::ffi::OsString::from)
            .or_else(|| std::env::var_os("ROTOR_DATA_DIR")),
        std::env::home_dir(),
        std::env::current_dir().ok(),
    )?;
    file_path::initialize_data_directory(directory.clone())?;
    if args.has("--check-config") {
        let service = ConfigService::load_from(&directory)?;
        println!(
            "Configuration loaded from {} ({} keys)",
            directory.display(),
            service.get_all().len()
        );
        return Ok(());
    }
    let (commands, command_receiver) = CommandBus::new();
    let activate = commands.clone();
    let acquire = if args.has("--wait-for-instance") {
        InstanceGuard::acquire_after_exit(
            &directory,
            std::time::Duration::from_secs(8),
            move || activate.request(Command::ShowSettings),
        )
    } else {
        InstanceGuard::acquire(&directory, move || activate.request(Command::ShowSettings))
    };
    let instance = match acquire? {
        Instance::Primary(guard) => guard,
        Instance::ActivatedExisting => return Ok(()),
    };
    let _logging = match logging::initialize(&directory) {
        Ok(guard) => Some(guard),
        Err(error) => {
            eprintln!("Logging: {error}");
            None
        }
    };
    let _validated_config = ConfigService::load_from(&directory)?;
    let resources = match args.value("--resource-dir")? {
        Some(path) => ResourceLocator::from_root(std::path::Path::new(&path)),
        None => ResourceLocator::for_current_process(),
    }
    .map_err(|error| log::warn!("OCR resources: {error}"))
    .ok();
    #[cfg(target_os = "windows")]
    if !args.has("--no-elevate") && !rotor_platform::desktop::is_elevated() {
        let forwarded = args.elevated_relaunch(
            &directory,
            resources.as_ref().map(|resources| resources.root()),
        );
        match rotor_platform::desktop::launch_elevated(&std::env::current_exe()?, &forwarded) {
            Ok(()) => {
                drop(instance);
                return Ok(());
            }
            Err(error) => log::warn!("{error}"),
        }
    }
    let _instance = instance;
    let (services, events) = Services::new(
        AppConfig::shared_global(),
        resources,
        ServiceOptions {
            index_files: !args.has("--no-index"),
        },
    )
    .map_err(std::io::Error::other)?;
    let services = Arc::new(services);
    services.configure_startup_flags(args.runtime_flags());
    let config = services.settings();
    let app_services = services.clone();
    let enable_hotkeys = !args.has("--no-hotkeys");
    let development_shortcuts =
        !rotor_common::native_app::PRODUCTION && !args.has("--production-shortcuts");
    let failed = Rc::new(Cell::new(false));
    let startup_failed = failed.clone();
    let application = gpui_kit::application()
        .with_assets(rotor_ui::UiAssets)
        .with_quit_mode(QuitMode::Explicit);
    application.on_reopen(|cx| {
        if cx.try_global::<ShellState>().is_some() {
            let _ = show_settings(cx);
        }
    });
    application.run(move |cx| {
        gpui_kit::init(cx);
        rotor_ui::configure_theme(cx);
        if let Err(error) = rotor_platform::desktop::configure_background_application() {
            log::warn!("Application policy: {error}");
        }
        apply_theme(&config, cx);
        let mut system = match SystemServices::new(
            commands.clone(),
            &config,
            enable_hotkeys,
            development_shortcuts,
            app_services.shortcut_recording_flag(),
        ) {
            Ok(system) => system,
            Err(error) => {
                log::error!("System services: {error}");
                startup_failed.set(true);
                cx.quit();
                return;
            }
        };
        if let Some(warning) = app_services.startup_warning() {
            system.warning = Some(match system.warning.take() {
                Some(previous) => format!("{previous}\n{warning}"),
                None => warning,
            });
        }
        #[cfg(target_os = "macos")]
        if let Some(warning) = update_warning {
            system.warning = Some(match system.warning.take() {
                Some(previous) => format!("{previous}\n{warning}"),
                None => warning,
            });
        }
        app_services.coordinate_shortcuts();
        cx.set_global(ShellState {
            windows: HashMap::new(),
            config,
            services: app_services,
            commands: commands.clone(),
            system,
            _task: None,
            _closed: None,
            _quit: None,
            pending_selection: None,
            capture: capture::CaptureState::default(),
            pins: pins::PinWindows::default(),
            monitors: Vec::new(),
            index_rebuild: None,
        });
        let closed = cx.on_window_closed(|cx, id| {
            if cx.try_global::<ShellState>().is_some() {
                let role = cx
                    .global::<ShellState>()
                    .windows
                    .iter()
                    .find(|(_, entry)| entry.window.window_id() == id)
                    .map(|(role, _)| *role);
                cx.global_mut::<ShellState>()
                    .windows
                    .retain(|_, entry| entry.window.window_id() != id);
                if role == Some(WindowRole::Settings)
                    && let Err(error) = rotor_platform::desktop::configure_background_application()
                {
                    log::warn!("Application policy: {error}");
                }
                if role == Some(WindowRole::Search) {
                    // Hide windows and release index memory. Resolve by the
                    // closing window ID above: an obsolete window must not
                    // release the index after a replacement has been opened.
                    cx.global::<ShellState>().services.release_search();
                }
                if let Some(WindowRole::Mask { session, .. }) = role {
                    cx.defer(move |cx| {
                        if cx.global::<ShellState>().capture.session.generation() == Some(session) {
                            let _ = capture::cancel(None, cx);
                        }
                    });
                }
            }
        });
        cx.global_mut::<ShellState>()._closed = Some(closed);
        let quit = cx.on_app_quit(|cx| {
            let final_records = pins::final_records(cx);
            capture::stop(cx);
            pins::stop(cx);
            let state = cx.global_mut::<ShellState>();
            state.index_rebuild = None;
            state.commands.close();
            state.system.stop_events();
            state.services.shutdown_with_pin_updates(final_records);
            async {}
        });
        cx.global_mut::<ShellState>()._quit = Some(quit);
        let _ = cx.global::<ShellState>().services.restore_pins();
        let task = cx.spawn(async move |cx| {
            loop {
                let command = command_receiver.recv();
                let event = events.recv();
                futures::pin_mut!(command, event);
                match select(command, event).await {
                    Either::Left((Ok(()), _)) => {
                        if let Some(command) = commands.take() {
                            cx.update(|cx| dispatch::dispatch_command(command, cx));
                        }
                    }
                    Either::Right((Ok(event), _)) => {
                        cx.update(|cx| dispatch::handle_event(event, cx))
                    }
                    _ => break,
                }
            }
        });
        cx.global_mut::<ShellState>()._task = Some(task);
        if let Some(path) = update_ready {
            std::thread::spawn(move || {
                use std::io::Write;
                let result = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .and_then(|mut file| {
                        file.write_all(b"native-startup-ready")?;
                        file.sync_all()
                    });
                if let Err(error) = result {
                    log::error!("Update startup acknowledgement: {error}");
                }
            });
        }
    });
    services.shutdown();
    log::logger().flush();
    if failed.get() {
        return Err("native application startup failed".into());
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Rotor: {error}");
        #[cfg(target_os = "windows")]
        if cli::Arguments::from_env().has("--installation-check")
            && let Some(profile) = file_path::get_userdata_path()
        {
            let flags = cli::Arguments::from_env().runtime_flags();
            match rotor_platform::desktop::rollback_failed_install(&profile, &flags) {
                Ok(()) => std::process::exit(1),
                Err(rollback) => eprintln!("Rollback could not start: {rollback}"),
            }
        }
        let diagnostic = cli::Arguments::from_env().is_diagnostic();
        if !diagnostic {
            rotor_platform::desktop::show_startup_error(&error.to_string());
        }
        std::process::exit(1);
    }
}
