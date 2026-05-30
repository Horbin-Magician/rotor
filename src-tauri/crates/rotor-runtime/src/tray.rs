use std::error::Error;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tauri::{WebviewUrl, WebviewWindowBuilder};

use rotor_common::i18n;

pub struct Tray {}

impl Tray {
    pub fn flag(&self) -> &str {
        "tray"
    }

    pub fn init(&mut self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        Tray::set_system_tray(app)?;
        Ok(())
    }

    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    pub fn new() -> Tray {
        Tray {}
    }

    fn set_system_tray(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(target_os = "macos")]
        let icon_path = app.path().resolve(
            "assets/icons/128x128White.png",
            tauri::path::BaseDirectory::Resource,
        )?;
        #[cfg(target_os = "windows")]
        let icon_path = app.path().resolve(
            "assets/icons/128x128.png",
            tauri::path::BaseDirectory::Resource,
        )?;
        let icon = tauri::image::Image::from_path(icon_path)?;
        let setting_i = MenuItem::with_id(app, "setting", i18n::t("setting"), true, None::<&str>)?;
        let quit_i = MenuItem::with_id(app, "quit", i18n::t("quit"), true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&setting_i, &quit_i])?;

        let _tray = TrayIconBuilder::new()
            .icon(icon)
            .tooltip(i18n::t("appName"))
            .menu(&menu)
            .on_menu_event(|app, event| match event.id.as_ref() {
                "setting" => {
                    if let Some(win) = app.get_webview_window("setting") {
                        if let Err(error) = win.show() {
                            log::error!("Failed to show setting window: {error}");
                        }
                        if let Err(error) = win.set_focus() {
                            log::error!("Failed to focus setting window: {error}");
                        }
                    } else {
                        let mut win_builder =
                            WebviewWindowBuilder::new(app, "setting", WebviewUrl::default())
                                .title(i18n::t("settingWindowTitle"))
                                .inner_size(500.0, 400.0)
                                .resizable(false)
                                .maximizable(false)
                                .visible(false);
                        // set transparent title bar only when building for macOS
                        #[cfg(target_os = "macos")]
                        {
                            win_builder = win_builder
                                .hidden_title(true)
                                .title_bar_style(tauri::TitleBarStyle::Overlay);
                        }
                        // disable decorations on Windows for custom titlebar
                        #[cfg(target_os = "windows")]
                        {
                            win_builder = win_builder.decorations(false);
                        }

                        if let Err(error) = win_builder.build() {
                            log::error!("Failed to build setting window: {error}");
                        }
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {
                    log::warn!("menu item {:?} not handled", event.id);
                }
            })
            .build(app)?;
        Ok(())
    }
}
