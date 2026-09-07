use gpui_kit::{component::Root, *};
use rotor_common::{ConfigService, file_path};
use std::{cell::Cell, error::Error, path::PathBuf, rc::Rc};

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
    let failed = Rc::new(Cell::new(false));
    let startup_failed = failed.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
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
                let view = cx.new(|_| rotor_ui::SettingsView::new(config));
                cx.new(|cx| Root::new(view, window, cx))
            }) {
                eprintln!("Failed to open settings: {error:#}");
                startup_failed.set(true);
                cx.quit();
            }
        });
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
