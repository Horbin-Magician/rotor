use gpui_kit::{
    component::{Root, Theme, ThemeMode},
    *,
};
use rotor_common::{AppConfig, Config, ConfigService, ResourceLocator, file_path};
use rotor_runtime::{RuntimeEvent, ServiceOptions, Services};
use std::{cell::Cell, error::Error, path::PathBuf, rc::Rc, sync::Arc};

struct EventBridge {
    view: Option<WeakEntity<rotor_ui::SettingsView>>,
    config: Config,
    _task: Option<Task<()>>,
    _appearance: Option<Subscription>,
}
impl Global for EventBridge {}

fn apply_theme(config: &Config, cx: &mut App) {
    match config.get("theme").map(String::as_str) {
        Some("1") => Theme::change(ThemeMode::Light, None, cx),
        Some("2") => Theme::change(ThemeMode::Dark, None, cx),
        _ => Theme::sync_system_appearance(None, cx),
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let directory = match std::env::var_os("ROTOR_DATA_DIR") {
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
    let service = ConfigService::load_from(&directory)?;
    if std::env::args().any(|arg| arg == "--check-config") {
        println!(
            "Configuration loaded from {} ({} keys)",
            directory.display(),
            service.get_all().len()
        );
        return Ok(());
    }
    let config = service.get_all();
    let resources = ResourceLocator::for_current_process()
        .map_err(|error| eprintln!("OCR resources: {error}"))
        .ok();
    let (services, events) = Services::new(
        AppConfig::shared_global(),
        resources,
        ServiceOptions {
            index_files: !std::env::args().any(|arg| arg == "--no-index"),
        },
    )
    .map_err(std::io::Error::other)?;
    let services = Arc::new(services);
    let app_services = services.clone();
    let failed = Rc::new(Cell::new(false));
    let startup_failed = failed.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(move |cx| {
            gpui_kit::init(cx);
            apply_theme(&config, cx);
            cx.set_global(EventBridge {
                view: None,
                config: config.clone(),
                _task: None,
                _appearance: None,
            });
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(820.), px(600.)), cx)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Rotor（开发版）".into()),
                    ..Default::default()
                }),
                app_id: Some("cc.fluctus.rotor.gpui-dev".into()),
                ..Default::default()
            };
            if let Err(error) = cx.open_window(options, |window, cx| {
                let observer = window.observe_window_appearance(|window, cx| {
                    if !matches!(
                        cx.global::<EventBridge>()
                            .config
                            .get("theme")
                            .map(String::as_str),
                        Some("1" | "2")
                    ) {
                        Theme::sync_system_appearance(Some(window), cx);
                    }
                });
                cx.global_mut::<EventBridge>()._appearance = Some(observer);
                let view = cx.new(|_| rotor_ui::SettingsView::new(config, app_services));
                cx.global_mut::<EventBridge>().view = Some(view.downgrade());
                cx.new(|cx| Root::new(view, window, cx))
            }) {
                eprintln!("Failed to open settings: {error:#}");
                startup_failed.set(true);
                cx.quit();
            }
            let task = cx.spawn(async move |cx| {
                while let Ok(event) = events.recv().await {
                    cx.update(|cx| {
                        if let RuntimeEvent::SettingsSaved {
                            result: Ok(config), ..
                        } = &event
                        {
                            let theme_changed = cx.global::<EventBridge>().config.get("theme")
                                != config.get("theme");
                            cx.global_mut::<EventBridge>().config = config.clone();
                            if theme_changed {
                                apply_theme(config, cx);
                            }
                        }
                        if let Some(view) = cx.global::<EventBridge>().view.clone() {
                            let _ = view.update(cx, |view, cx| view.handle_event(event, cx));
                        }
                    });
                }
            });
            cx.global_mut::<EventBridge>()._task = Some(task);
        });
    services.shutdown();
    if failed.get() {
        return Err("settings window startup failed".into());
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Rotor: {error}");
        std::process::exit(1);
    }
}
