use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

// use crate::util::log_util;
use crate::module::{screen_shotter::ScreenShotter, Module}; // setting::Setting, earcher::Searcher TODO enable

pub fn handle_global_hotkey_event(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() == ShortcutState::Pressed {
        let rotor_app = INSTANCE.lock().unwrap();
        for module in rotor_app.modules.values() {
            if let Some(module_shortcut) = module.get_shortcut() {
                if module_shortcut == *shortcut {
                    module.run(app);
                }
            }
        }
    }
}

pub struct Application {
    pub app: Option<AppHandle>,
    pub modules: HashMap<String, Box<dyn Module + Send + 'static>>,
}

impl Application {
    fn new() -> Application {
        let mut modules = HashMap::new();

        // if let Ok(searcher) = Searcher::new() {
        //     modules.push(Box::new(searcher));
        // }
        if let Ok(screen_shotter) = ScreenShotter::new() {
            modules.insert(
                screen_shotter.flag().to_string(),
                Box::new(screen_shotter) as Box<dyn Module + Send>,
            );
        }
        // if let Ok(setting) = Setting::new(_msg_sender.clone()) {
        //     modules.push(Box::new(setting));
        // }

        Application { app: None, modules }
    }

    pub fn global() -> &'static Mutex<Application> {
        &INSTANCE
    }

    pub fn init(&mut self, app: tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        for module in self.modules.values() {
            if let Some(shortcut) = module.get_shortcut() {
                app.global_shortcut().register(shortcut)?;
            }
        }
        Application::set_system_tray(&app)?;
        self.app = Some(app);
        Ok(())
    }

    fn set_system_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        let icon_path = app
            .path()
            .resolve("icons/128x128.png", tauri::path::BaseDirectory::Resource)?;
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
                            win_builder = win_builder.hidden_title(true).title_bar_style(tauri::TitleBarStyle::Overlay);
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

static INSTANCE: LazyLock<Mutex<Application>> = LazyLock::new(|| Mutex::new(Application::new()));
