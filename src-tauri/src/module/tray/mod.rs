use std::any::Any;
use std::error::Error;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager,
};
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::Shortcut;

use crate::module::Module;

pub struct Tray {}

impl Module for Tray {
    fn flag(&self) -> &str {
        "tray"
    }

    fn init(&mut self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        // do nothing now
        Tray::set_system_tray(app)?;
        Ok(())
    }

    fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // do nothing now
        Ok(())
    }

    fn get_shortcut(&self) -> Option<Shortcut> {
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Tray {
    pub fn new() -> Result<Tray, Box<dyn Error>> {
        Ok(Tray {})
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
        let icon = tauri::image::Image::from_path(icon_path).unwrap();
        let setting_i = MenuItem::with_id(app, "setting", "设置", true, None::<&str>)?;
        let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&setting_i, &quit_i])?;

        let _tray = TrayIconBuilder::new()
            .icon(icon)
            .tooltip("小云管家")
            .menu(&menu)
            .on_menu_event(|app, event| match event.id.as_ref() {
                "setting" => {
                    if let Some(win) = app.get_webview_window("setting") {
                        win.show().unwrap();
                        win.set_focus().unwrap();
                    } else {
                        let mut win_builder =
                            WebviewWindowBuilder::new(app, "setting", WebviewUrl::default())
                                .title("设置")
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

                        let _window = win_builder.build().unwrap();
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {
                    println!("menu item {:?} not handled", event.id);
                }
            })
            .build(app)?;
        Ok(())
    }
}
