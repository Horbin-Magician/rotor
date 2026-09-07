mod capture;
mod fonts;
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
}
impl Global for ShellState {}

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
        app_id: Some("cc.fluctus.rotor.gpui-dev".into()),
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
    let _ = cx.global::<ShellState>().services.request_index_status();
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
        cx.global_mut::<ShellState>().config = config.clone();
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
    if let Some(view) = cx
        .global::<ShellState>()
        .windows
        .get(&WindowRole::Translator)
        .and_then(|entry| match &entry.view {
            WindowView::Translator(view) => Some(view.clone()),
            _ => None,
        })
    {
        let _ = view.update(cx, |view, cx| view.handle_event(&event, cx));
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
            .join(".rotor-gpui"),
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
    let _instance = match InstanceGuard::acquire(&directory, move || {
        activate.request(Command::ShowSettings)
    })? {
        Instance::Primary(guard) => guard,
        Instance::ActivatedExisting => return Ok(()),
    };
    let _validated_config = ConfigService::load_from(&directory)?;
    let resources = match option("--resource-dir")? {
        Some(path) => ResourceLocator::from_root(std::path::Path::new(&path)),
        None => ResourceLocator::for_current_process(),
    }
    .map_err(|error| eprintln!("OCR resources: {error}"))
    .ok();
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
                    "--no-index" | "--no-hotkeys" | "--production-shortcuts"
                )
            })
            .cloned()
            .collect(),
    );
    let config = services.settings();
    let app_services = services.clone();
    let background = args.iter().any(|arg| arg == "--background");
    let enable_hotkeys = !args.iter().any(|arg| arg == "--no-hotkeys");
    let development_shortcuts = !args.iter().any(|arg| arg == "--production-shortcuts");
    let failed = Rc::new(Cell::new(false));
    let startup_failed = failed.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
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
                    if let Some(WindowRole::Mask { session, .. }) = role {
                        cx.defer(move |cx| {
                            if cx.global::<ShellState>().capture.session.generation()
                                == Some(session)
                            {
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
                                let quit = matches!(command, Command::Quit);
                                cx.update(|cx| {
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
                                                    view.receive_recorded_shortcut(
                                                        value, window, cx,
                                                    )
                                                });
                                            });
                                        }
                                        return;
                                    }
                                    let command = if let Command::Shortcut { key, generation } =
                                        command
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
                                        match cx
                                            .global::<ShellState>()
                                            .system
                                            .resolve(key, generation)
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
                                        Command::Quit => cx.quit(),
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
                                        Command::Shortcut { .. }
                                        | Command::RecordedShortcut { .. } => {
                                            unreachable!("shortcut was resolved before dispatch")
                                        }
                                    }
                                });
                                if quit {
                                    break;
                                }
                            }
                        }
                        Either::Right((Ok(event), _)) => cx.update(|cx| handle_event(event, cx)),
                        _ => break,
                    }
                }
            });
            cx.global_mut::<ShellState>()._task = Some(task);
        });
    services.shutdown();
    if failed.get() {
        return Err("native application startup failed".into());
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Rotor: {error}");
        std::process::exit(1);
    }
}
