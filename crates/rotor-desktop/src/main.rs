#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod capture;
mod fonts;
mod logging;
mod pins;
mod placement;
mod system;

use futures::future::{Either, select};
use gpui_kit::{
    component::{Root, Theme, ThemeMode},
    *,
};
use rotor_common::{AppConfig, Config, ConfigService, ResourceLocator, file_path};
use rotor_platform::single_instance::{Instance, InstanceGuard};
use rotor_runtime::{OperationId, RuntimeEvent, ServiceOptions, Services};
use std::{cell::Cell, collections::HashMap, error::Error, path::PathBuf, rc::Rc, sync::Arc};
use system::{Command, CommandBus, SystemServices};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum WindowRole {
    Settings,
    Translator,
    Search,
    Mask { session: u64, monitor: u32 },
    Pin(u64),
}

enum WindowView {
    Settings(WeakEntity<rotor_ui::SettingsView>),
    Translator(WeakEntity<rotor_ui::TranslatorView>),
    Search(WeakEntity<rotor_ui::SearchView>),
    Mask(WeakEntity<rotor_ui::MaskView>),
    Pin(WeakEntity<rotor_ui::PinView>),
}

struct WindowSlot {
    window: AnyWindowHandle,
    view: WindowView,
    _appearance: Option<Subscription>,
}

struct ShellState {
    windows: HashMap<WindowRole, WindowSlot>,
    config: Config,
    services: Arc<Services>,
    commands: CommandBus,
    system: SystemServices,
    _task: Option<Task<()>>,
    _closed: Option<Subscription>,
    _quit: Option<Subscription>,
    pending_selection: Option<OperationId>,
    capture: capture::CaptureState,
    pins: pins::PinWindows,
    monitors: Vec<rotor_runtime::MonitorConfig>,
    fonts: Option<Task<()>>,
    index_rebuild: Option<Task<()>>,
}
impl Global for ShellState {}

fn quit_in_progress(cx: &App) -> bool {
    cx.global::<ShellState>()
        .windows
        .get(&WindowRole::Settings)
        .and_then(|entry| match &entry.view {
            WindowView::Settings(view) => view.upgrade(),
            _ => None,
        })
        .is_some_and(|view| view.read(cx).waiting_to_quit())
}

fn request_quit(cx: &mut App) {
    if let Some((handle, view)) = cx
        .global::<ShellState>()
        .windows
        .get(&WindowRole::Settings)
        .and_then(|entry| match &entry.view {
            WindowView::Settings(view) => Some((entry.window, view.clone())),
            _ => None,
        })
        && handle
            .update(cx, |_, window, cx| {
                view.update(cx, |view, cx| view.request_quit(window, cx))
            })
            .is_ok_and(|result| result.is_ok())
    {
        return;
    }
    cx.quit();
}

/// Retain background warnings for the next settings open and update an already
/// open settings view without taking focus away from the user's current app.
pub(crate) fn publish_warning(message: String, cx: &mut App) {
    let view = {
        let state = cx.global_mut::<ShellState>();
        state.system.warning = Some(message.clone());
        state
            .windows
            .get(&WindowRole::Settings)
            .and_then(|entry| match &entry.view {
                WindowView::Settings(view) => Some(view.clone()),
                _ => None,
            })
    };
    if let Some(view) = view {
        let _ = view.update(cx, |view, cx| view.show_message(message, cx));
    }
}

fn apply_theme(config: &Config, cx: &mut App) {
    match config.get("theme").map(String::as_str) {
        Some("1") => Theme::change(ThemeMode::Light, None, cx),
        Some("2") => Theme::change(ThemeMode::Dark, None, cx),
        _ => Theme::sync_system_appearance(None, cx),
    }
}

fn show_settings(cx: &mut App) -> Result<(), String> {
    if let Some((view, warning)) = cx
        .global::<ShellState>()
        .windows
        .get(&WindowRole::Settings)
        .and_then(|entry| match &entry.view {
            WindowView::Settings(view) => cx
                .global::<ShellState>()
                .system
                .warning
                .clone()
                .map(|warning| (view.clone(), warning)),
            _ => None,
        })
    {
        let _ = view.update(cx, |view, cx| view.show_message(warning, cx));
    }
    if let Some(handle) = cx
        .global::<ShellState>()
        .windows
        .get(&WindowRole::Settings)
        .map(|entry| entry.window)
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return Ok(());
    }
    let state = cx.global::<ShellState>();
    let config = state.config.clone();
    let services = state.services.clone();
    let warning = state.system.warning.clone();
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(820.), px(600.)), cx)),
        titlebar: Some(TitlebarOptions {
            title: Some(rotor_ui::settings_title(&config).into()),
            ..Default::default()
        }),
        app_id: Some(rotor_common::native_app::IDENTIFIER.into()),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| {
        let appearance = window.observe_window_appearance(|window, cx| {
            if !matches!(
                cx.global::<ShellState>()
                    .config
                    .get("theme")
                    .map(String::as_str),
                Some("1" | "2")
            ) {
                Theme::sync_system_appearance(Some(window), cx);
            }
        });
        let view = cx.new(|cx| rotor_ui::SettingsView::new(config, services, window, cx));
        let closing = view.downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            closing
                .update(cx, |view, cx| view.request_close(window, cx))
                .is_err()
        });
        if let Some(warning) = warning {
            view.update(cx, |view, cx| view.show_message(warning, cx));
        }
        cx.global_mut::<ShellState>().windows.insert(
            WindowRole::Settings,
            WindowSlot {
                window: window.window_handle(),
                view: WindowView::Settings(view.downgrade()),
                _appearance: Some(appearance),
            },
        );
        cx.new(|cx| Root::new(view, window, cx))
    })
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn show_translator(cx: &mut App) -> Result<(), String> {
    if let Some((handle, view)) = cx
        .global::<ShellState>()
        .windows
        .get(&WindowRole::Translator)
        .and_then(|entry| match &entry.view {
            WindowView::Translator(view) => Some((entry.window, view.clone())),
            _ => None,
        })
        && handle
            .update(cx, |_, window, cx| {
                window.activate_window();
                let _ = view.update(cx, |view, cx| view.begin_input(window, cx));
            })
            .is_ok()
    {
        return Ok(());
    }
    let services = cx.global::<ShellState>().services.clone();
    cx.open_window(
        placement::utility_options(size(px(560.), px(420.)), true, cx),
        |window, cx| {
            let appearance = window.observe_window_appearance(|window, cx| {
                if !matches!(
                    cx.global::<ShellState>()
                        .config
                        .get("theme")
                        .map(String::as_str),
                    Some("1" | "2")
                ) {
                    Theme::sync_system_appearance(Some(window), cx);
                }
            });
            let view = cx.new(|cx| rotor_ui::TranslatorView::new(services, window, cx));
            cx.global_mut::<ShellState>().windows.insert(
                WindowRole::Translator,
                WindowSlot {
                    window: window.window_handle(),
                    view: WindowView::Translator(view.downgrade()),
                    _appearance: Some(appearance),
                },
            );
            cx.new(|cx| Root::new(view, window, cx))
        },
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn show_search(cx: &mut App) -> Result<(), String> {
    if let Some(handle) = cx
        .global::<ShellState>()
        .windows
        .get(&WindowRole::Search)
        .map(|entry| entry.window)
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return Ok(());
    }
    let services = cx.global::<ShellState>().services.clone();
    cx.open_window(
        placement::utility_options(size(px(680.), px(140.)), false, cx),
        |window, cx| {
            let appearance = window.observe_window_appearance(|window, cx| {
                if !matches!(
                    cx.global::<ShellState>()
                        .config
                        .get("theme")
                        .map(String::as_str),
                    Some("1" | "2")
                ) {
                    Theme::sync_system_appearance(Some(window), cx);
                }
            });
            let view = cx.new(|cx| rotor_ui::SearchView::new(services, window, cx));
            cx.global_mut::<ShellState>().windows.insert(
                WindowRole::Search,
                WindowSlot {
                    window: window.window_handle(),
                    view: WindowView::Search(view.downgrade()),
                    _appearance: Some(appearance),
                },
            );
            cx.new(|cx| Root::new(view, window, cx))
        },
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

fn handle_event(event: RuntimeEvent, cx: &mut App) {
    if matches!(&event, RuntimeEvent::Update(snapshot) if snapshot.phase == rotor_runtime::UpdatePhase::HandedOff)
    {
        request_quit(cx);
        return;
    }
    if let RuntimeEvent::SettingsCoordination(request) = event {
        match request {
            rotor_runtime::SettingsCoordination::Prepare {
                id,
                candidate,
                reply,
            } => {
                let result = cx.global_mut::<ShellState>().system.prepare(id, &candidate);
                let _ = reply.send(result);
            }
            rotor_runtime::SettingsCoordination::Finish {
                id,
                committed,
                reply,
            } => {
                let result = cx.global_mut::<ShellState>().system.finish(id, committed);
                let _ = reply.send(result);
            }
        }
        return;
    }
    if let RuntimeEvent::CaptureFinished { id, result } = event {
        capture::completed(id, result, cx);
        return;
    }
    pins::handle_event(&event, cx);
    if let RuntimeEvent::SelectionFinished { id, result } = event {
        if cx.global::<ShellState>().pending_selection != Some(id) {
            return;
        }
        cx.global_mut::<ShellState>().pending_selection = None;
        match result {
            Ok(selected) => {
                if let Err(error) = show_translator(cx) {
                    eprintln!("Translator: {error}");
                    return;
                }
                if let Some((handle, view)) = cx
                    .global::<ShellState>()
                    .windows
                    .get(&WindowRole::Translator)
                    .and_then(|entry| match &entry.view {
                        WindowView::Translator(view) => Some((entry.window, view.clone())),
                        _ => None,
                    })
                {
                    let _ = handle.update(cx, |_, window, cx| {
                        let _ = view.update(cx, |view, cx| {
                            view.translate_text(selected.text, selected.restore_warning, window, cx)
                        });
                    });
                }
            }
            Err(error) => {
                cx.global_mut::<ShellState>().system.warning = Some(error);
                if let Err(error) = show_settings(cx) {
                    eprintln!("Selection: {error}");
                }
            }
        }
        return;
    }
    if let Some((handle, view)) = cx
        .global::<ShellState>()
        .windows
        .get(&WindowRole::Search)
        .and_then(|entry| match &entry.view {
            WindowView::Search(view) => Some((entry.window, view.clone())),
            _ => None,
        })
    {
        let _ = handle.update(cx, |_, window, cx| {
            let _ = view.update(cx, |view, cx| view.handle_event(&event, window, cx));
        });
    }
    if let RuntimeEvent::SettingsSaved {
        result: Ok(config), ..
    } = &event
    {
        let theme_changed = cx.global::<ShellState>().config.get("theme") != config.get("theme");
        let language_changed =
            cx.global::<ShellState>().config.get("language") != config.get("language");
        let exclusions_changed = cx.global::<ShellState>().config.get("search_excluded_dirs")
            != config.get("search_excluded_dirs");
        cx.global_mut::<ShellState>().config = config.clone();
        if exclusions_changed {
            let services = cx.global::<ShellState>().services.clone();
            let rebuild = cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
                services.rebuild_search();
            });
            cx.global_mut::<ShellState>().index_rebuild = Some(rebuild);
        }
        if theme_changed {
            apply_theme(config, cx);
        }
        if language_changed {
            let state = cx.global_mut::<ShellState>();
            if let Err(error) = state.system.update_menu(state.commands.clone(), config) {
                eprintln!("Tray menu: {error}");
            }
            let handles: Vec<_> = state
                .windows
                .iter()
                .filter(|(role, _)| **role == WindowRole::Settings)
                .map(|(_, entry)| entry.window)
                .collect();
            for handle in handles {
                let _ = handle.update(cx, |_, window, _| {
                    window.set_window_title(rotor_ui::settings_title(config))
                });
            }
        }
    }
    if let Some((handle, view)) = cx
        .global::<ShellState>()
        .windows
        .get(&WindowRole::Translator)
        .and_then(|entry| match &entry.view {
            WindowView::Translator(view) => Some((entry.window, view.clone())),
            _ => None,
        })
    {
        let _ = handle.update(cx, |_, window, cx| {
            let _ = view.update(cx, |view, cx| view.handle_event(&event, window, cx));
        });
    }
    if let Some((handle, view)) = cx
        .global::<ShellState>()
        .windows
        .get(&WindowRole::Settings)
        .and_then(|entry| match &entry.view {
            WindowView::Settings(view) => Some((entry.window, view.clone())),
            _ => None,
        })
    {
        let _ = handle.update(cx, |_, window, cx| {
            let _ = view.update(cx, |view, cx| view.handle_event(event, window, cx));
        });
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 2 && args[1] == "--build-info" {
        println!("{}", rotor_common::native_app::build_info_json());
        return Ok(());
    }
    let option = |key: &str| -> Result<Option<String>, String> {
        let Some(index) = args.iter().position(|arg| arg == key) else {
            return Ok(None);
        };
        args.get(index + 1)
            .filter(|value| !value.starts_with("--") && !value.is_empty())
            .cloned()
            .map(Some)
            .ok_or_else(|| format!("Missing value for {key}"))
    };
    if args.iter().any(|arg| arg == "--check-resources") {
        let resources = match option("--resource-dir")? {
            Some(path) => ResourceLocator::from_root(std::path::Path::new(&path))?,
            None => ResourceLocator::for_current_process()?,
        };
        resources.verify_native_resources()?;
        println!(
            "Native resources verified at {}",
            resources.root().display()
        );
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    if let Some(job) = option("--apply-update")? {
        if args.len() != 3 || args[1] != "--apply-update" {
            return Err("Invalid update helper arguments".into());
        }
        return rotor_updater::run_helper(std::path::Path::new(&job)).map_err(Into::into);
    }
    #[cfg(target_os = "macos")]
    let update_ready = option("--update-ready")?.map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let update_warning = option("--update-error-file")?.and_then(|path| {
        rotor_updater::handoff_error(std::path::Path::new(&path))
            .unwrap_or_else(|error| Some(format!("Cannot read update result: {error}")))
    });
    let directory_override = option("--data-dir")?
        .map(std::ffi::OsString::from)
        .or_else(|| std::env::var_os("ROTOR_DATA_DIR"));
    let directory = match directory_override {
        Some(value) if value.is_empty() => return Err("ROTOR_DATA_DIR cannot be empty".into()),
        Some(value) => {
            let path = PathBuf::from(value);
            if path.is_absolute() {
                path
            } else {
                std::env::current_dir()?.join(path)
            }
        }
        None => std::env::home_dir()
            .ok_or("home directory unavailable")?
            .join(rotor_common::native_app::PROFILE_DIRECTORY),
    };
    file_path::initialize_data_directory(directory.clone())?;
    if args.iter().any(|arg| arg == "--check-config") {
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
    let acquire = if args.iter().any(|arg| arg == "--wait-for-instance") {
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
    let legacy_guard = if rotor_common::native_app::PRODUCTION {
        let activate = commands.clone();
        Some(rotor_platform::legacy_instance::LegacyLease::acquire(
            rotor_common::native_app::IDENTIFIER,
            move || activate.request(Command::ShowSettings),
        )?)
    } else {
        None
    };
    let _logging = match logging::initialize(&directory) {
        Ok(guard) => Some(guard),
        Err(error) => {
            eprintln!("Logging: {error}");
            None
        }
    };
    if rotor_common::native_app::PRODUCTION
        && let Some(backup) = rotor_common::profile_migration::prepare_native_profile(&directory)?
    {
        log::info!("Legacy profile backup retained at {}", backup.display());
    }
    let _validated_config = ConfigService::load_from(&directory)?;
    let resources = match option("--resource-dir")? {
        Some(path) => ResourceLocator::from_root(std::path::Path::new(&path)),
        None => ResourceLocator::for_current_process(),
    }
    .map_err(|error| eprintln!("OCR resources: {error}"))
    .ok();
    #[cfg(target_os = "windows")]
    if !args.iter().any(|arg| arg == "--no-elevate") && !rotor_platform::desktop::is_elevated() {
        let mut forwarded = vec![
            "--wait-for-instance".into(),
            "--data-dir".into(),
            directory.to_string_lossy().into_owned(),
        ];
        if let Some(resources) = &resources {
            forwarded.extend([
                "--resource-dir".into(),
                resources.root().to_string_lossy().into_owned(),
            ]);
        }
        let mut index = 1;
        while index < args.len() {
            if matches!(args[index].as_str(), "--data-dir" | "--resource-dir") {
                index += 2;
                continue;
            }
            if args[index] != "--wait-for-instance" {
                forwarded.push(args[index].clone());
            }
            index += 1;
        }
        match rotor_platform::desktop::launch_elevated(&std::env::current_exe()?, &forwarded) {
            Ok(()) => {
                drop(legacy_guard);
                drop(instance);
                return Ok(());
            }
            Err(error) => log::warn!("{error}"),
        }
    }
    let _instance = instance;
    let _legacy_guard = legacy_guard;
    let font_resources = resources.clone();
    let (services, events) = Services::new(
        AppConfig::shared_global(),
        resources,
        ServiceOptions {
            index_files: !args.iter().any(|arg| arg == "--no-index"),
        },
    )
    .map_err(std::io::Error::other)?;
    let services = Arc::new(services);
    services.configure_startup_flags(
        args.iter()
            .filter(|arg| {
                matches!(
                    arg.as_str(),
                    "--no-index" | "--no-hotkeys" | "--production-shortcuts" | "--no-elevate"
                )
            })
            .cloned()
            .collect(),
    );
    if rotor_common::native_app::PRODUCTION
        && let Err(error) = services.migrate_existing_startup()
    {
        log::warn!("Startup migration: {error}");
    }
    let config = services.settings();
    let app_services = services.clone();
    let background = args.iter().any(|arg| arg == "--background");
    let enable_hotkeys = !args.iter().any(|arg| arg == "--no-hotkeys");
    let development_shortcuts = !rotor_common::native_app::PRODUCTION
        && !args.iter().any(|arg| arg == "--production-shortcuts");
    let failed = Rc::new(Cell::new(false));
    let startup_failed = failed.clone();
    let application = gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
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
                eprintln!("System services: {error}");
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
        app_services.coordinate_shortcuts(development_shortcuts);
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
            fonts: None,
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
                if role == Some(WindowRole::Search) {
                    // Restore the legacy hide/release contract. Resolve by the
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
        let font_task = fonts::load(font_resources, cx);
        cx.global_mut::<ShellState>().fonts = Some(font_task);
        cx.global_mut::<ShellState>()._closed = Some(closed);
        let quit = cx.on_app_quit(|cx| {
            let final_records = pins::final_records(cx);
            capture::stop(cx);
            pins::stop(cx);
            let state = cx.global_mut::<ShellState>();
            state.fonts = None;
            state.index_rebuild = None;
            state.commands.close();
            state.system.stop_events();
            state.services.shutdown_with_pin_updates(final_records);
            async {}
        });
        cx.global_mut::<ShellState>()._quit = Some(quit);
        let _ = cx.global::<ShellState>().services.restore_pins();
        if !background && let Err(error) = show_settings(cx) {
            eprintln!("Settings: {error}");
            startup_failed.set(true);
            cx.quit();
            return;
        }
        let task = cx.spawn(async move |cx| {
            loop {
                let command = command_receiver.recv();
                let event = events.recv();
                futures::pin_mut!(command, event);
                match select(command, event).await {
                    Either::Left((Ok(()), _)) => {
                        if let Some(command) = commands.take() {
                            cx.update(|cx| {
                                // Keep draining runtime coordination while a save
                                // finishes, but don't begin another tool operation.
                                if quit_in_progress(cx) && !matches!(command, Command::Quit) {
                                    return;
                                }
                                let command = match command {
                                    Command::Shortcut { key, generation }
                                        if cx
                                            .global::<ShellState>()
                                            .services
                                            .is_shortcut_recording() =>
                                    {
                                        Command::RecordedShortcut { key, generation }
                                    }
                                    other => other,
                                };
                                if let Command::RecordedShortcut { key, generation } = command {
                                    if let Some(value) = cx
                                        .global::<ShellState>()
                                        .system
                                        .shortcut_label(key, generation)
                                        && let Some((handle, view)) = cx
                                            .global::<ShellState>()
                                            .windows
                                            .get(&WindowRole::Settings)
                                            .and_then(|entry| match &entry.view {
                                                WindowView::Settings(view) => {
                                                    Some((entry.window, view.clone()))
                                                }
                                                _ => None,
                                            })
                                    {
                                        let _ = handle.update(cx, |_, window, cx| {
                                            let _ = view.update(cx, |view, cx| {
                                                view.receive_recorded_shortcut(value, window, cx)
                                            });
                                        });
                                    }
                                    return;
                                }
                                let command = if let Command::Shortcut { key, generation } = command
                                {
                                    if cx
                                        .global::<ShellState>()
                                        .services
                                        .shortcut_recording_flag()
                                        .quiet(std::time::Instant::now())
                                    {
                                        return;
                                    }
                                    use rotor_runtime::shortcuts::ShortcutAction;
                                    match cx.global::<ShellState>().system.resolve(key, generation)
                                    {
                                        Some(ShortcutAction::Settings) => Command::ShowSettings,
                                        Some(ShortcutAction::Search) => Command::ShowSearch,
                                        Some(ShortcutAction::Capture) => Command::Capture,
                                        Some(ShortcutAction::TranslateSelection) => {
                                            Command::SelectText
                                        }
                                        Some(ShortcutAction::TranslateInput) => {
                                            Command::ShowTranslator
                                        }
                                        Some(ShortcutAction::Quick(id)) => {
                                            if let Err(error) = cx
                                                .global::<ShellState>()
                                                .services
                                                .run_quick_action(id)
                                            {
                                                eprintln!("Quick action: {error}");
                                            }
                                            return;
                                        }
                                        None => return,
                                    }
                                } else {
                                    command
                                };
                                if !matches!(command, Command::Capture | Command::Quit)
                                    && cx
                                        .global::<ShellState>()
                                        .capture
                                        .session
                                        .generation()
                                        .is_some()
                                {
                                    let _ = capture::cancel(None, cx);
                                }
                                if !matches!(command, Command::SelectText) {
                                    let state = cx.global_mut::<ShellState>();
                                    state.services.cancel_selection();
                                    state.pending_selection = None;
                                }
                                match command {
                                    Command::ShowSettings => {
                                        if let Err(error) = show_settings(cx) {
                                            eprintln!("Settings: {error}");
                                        }
                                    }
                                    Command::Quit => request_quit(cx),
                                    Command::ShowTranslator => {
                                        if let Err(error) = show_translator(cx) {
                                            eprintln!("Translator: {error}");
                                        }
                                    }
                                    Command::ShowSearch => {
                                        if let Err(error) = show_search(cx) {
                                            eprintln!("Search: {error}");
                                        }
                                    }
                                    Command::SelectText => {
                                        let state = cx.global_mut::<ShellState>();
                                        if state.pending_selection.is_none() {
                                            match state.services.capture_selection() {
                                                Ok(id) => state.pending_selection = Some(id),
                                                Err(error) => {
                                                    state.system.warning = Some(error);
                                                    let _ = show_settings(cx);
                                                }
                                            }
                                        }
                                    }
                                    Command::Capture => {
                                        if let Err(error) = capture::begin(cx) {
                                            capture::report(error, cx);
                                        }
                                    }
                                    Command::ShowPins => pins::show_all(cx),
                                    Command::Shortcut { .. } | Command::RecordedShortcut { .. } => {
                                        unreachable!("shortcut was resolved before dispatch")
                                    }
                                }
                            });
                        }
                    }
                    Either::Right((Ok(event), _)) => cx.update(|cx| handle_event(event, cx)),
                    _ => break,
                }
            }
        });
        cx.global_mut::<ShellState>()._task = Some(task);
        #[cfg(target_os = "macos")]
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
        if std::env::args().any(|arg| arg == "--installation-check")
            && let Some(profile) = file_path::get_userdata_path()
        {
            let flags = std::env::args()
                .filter(|arg| {
                    matches!(
                        arg.as_str(),
                        "--no-elevate" | "--no-index" | "--no-hotkeys" | "--production-shortcuts"
                    )
                })
                .collect::<Vec<_>>();
            match rotor_platform::desktop::rollback_failed_install(&profile, &flags) {
                Ok(()) => std::process::exit(1),
                Err(rollback) => eprintln!("Rollback could not start: {rollback}"),
            }
        }
        let diagnostic = std::env::args().any(|arg| {
            matches!(
                arg.as_str(),
                "--check-config"
                    | "--check-resources"
                    | "--build-info"
                    | "--apply-update"
                    | "--update-ready"
            )
        });
        if !diagnostic {
            rotor_platform::desktop::show_startup_error(&error.to_string());
        }
        std::process::exit(1);
    }
}
