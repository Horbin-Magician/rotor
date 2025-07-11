mod pin_win;

use std::any::Any;
use std::error::Error;
use std::str::FromStr;
use std::time::Duration;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::Shortcut;
use xcap::Monitor;

use crate::core::config::AppConfig;
use crate::module::Module;

pub struct ScreenShotter {

}

impl Module for ScreenShotter {
    fn flag(&self) -> &str {
        "screenshot"
    }

    fn init(&mut self, _app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        // do nothing now
        Ok(())
    }

    fn run(&self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        if let Some(win) = app.get_webview_window("ssmask") {
            win.show()?;
            win.set_focus()?;
        } else {
            let win_builder =
                WebviewWindowBuilder::new(app, "ssmask", WebviewUrl::App("ssmask".into()))
                    .always_on_top(true)
                    .resizable(false)
                    .decorations(false) // TODO del
                    .fullscreen(true)   // TODO windows only
                    // .simple_fullscreen(true)                       // TODO wait tauri update
                    .visible(false)
                    .skip_taskbar(true);                        // TODO windows only

            let _window = win_builder.build()?;
        }
        Ok(())
    }

    fn get_shortcut(&self) -> Option<Shortcut> {
        let app_config = AppConfig::global().lock().unwrap();
        let shortcut = app_config.get(&"shortcut_screenshot".to_string());
        if let Some(shortcut_str) = shortcut {
            return Some(Shortcut::from_str(shortcut_str).unwrap());
        }
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ScreenShotter {
    pub fn new() -> Result<ScreenShotter, Box<dyn Error>> {

        Ok(ScreenShotter{

        })
    }

    pub fn capture_screen(pos_x: i32, pos_y: i32) -> Vec<u8> {
        if let Ok(monitor) = Monitor::from_point(pos_x, pos_y) {
            let monitor_img = monitor.capture_image().unwrap_or_default();
            return monitor_img.to_vec();
        }
        Vec::new()
    }

    pub fn new_pin(&self) {
        println!("TODO: new pin win");
    }
}
