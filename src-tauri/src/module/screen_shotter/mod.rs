mod pin_win;

use std::any::Any;
use std::error::Error;
use std::str::FromStr;
use std::collections::HashMap;
use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::Shortcut;
use xcap::Monitor;

use crate::core::config::AppConfig;
use crate::module::Module;

type Image = Vec<u8>;
pub struct ScreenShotter {
    pub masks: HashMap<String, Image>,
}

impl Module for ScreenShotter {
    fn flag(&self) -> &str {
        "screenshot"
    }

    fn init(&mut self, _app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        // do nothing now
        Ok(())
    }

    fn run(&mut self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        self.masks.clear();

        let monitor = Monitor::from_point(0, 0)?;
        let label = format!("ssmask-{}", self.masks.len());
        let monitor_img = monitor.capture_image().unwrap_or_default().to_vec();
        self.masks.insert(label.clone(), monitor_img);

        let win_builder =
            WebviewWindowBuilder::new(app, label, WebviewUrl::App("ssmask".into()))
                .position(monitor.x()? as f64, monitor.y()? as f64)
                .always_on_top(true)
                .resizable(false)
                .decorations(false) // TODO del
                .fullscreen(true)   // TODO windows only
                // .simple_fullscreen(true)                       // TODO wait tauri update
                .visible(false)
                .skip_taskbar(true);                        // TODO windows only

        let _window = win_builder.build()?;
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
            masks: HashMap::new(),
        })
    }


    pub fn new_pin(&self) {
        println!("TODO: new pin win");
    }
}
