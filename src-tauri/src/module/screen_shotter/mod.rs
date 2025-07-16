// mod pin_win;

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
    app_hander: Option<tauri::AppHandle>,
    pub masks: HashMap<String, Image>,
    max_pin_id: u8,
}

impl Module for ScreenShotter {
    fn flag(&self) -> &str {
        "screenshot"
    }

    fn init(&mut self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        self.app_hander = Some(app.clone());
        Ok(())
    }

    fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_hander {
            Some(handle) => handle,
            None => return Err("AppHandle not initialized".into()),
        };

        self.masks.clear();
        let monitor = Monitor::from_point(0, 0)?;
        let label = format!("ssmask-{}", self.masks.len());
        let monitor_img = monitor.capture_image().unwrap_or_default().to_vec();
        self.masks.insert(label.clone(), monitor_img);

        let win_builder =
            WebviewWindowBuilder::new(app_handle, label, WebviewUrl::App("ScreenShotter/Mask".into()))
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

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl ScreenShotter {
    pub fn new() -> Result<ScreenShotter, Box<dyn Error>> {

        Ok(ScreenShotter{
            app_hander: None,
            masks: HashMap::new(),
            max_pin_id: 0,
        })
    }


    pub fn new_pin(&mut self, offset_x: f64, offset_y: f64, width: f64, height: f64, webview_window: tauri::WebviewWindow) -> Result<(), Box<dyn Error>> {
        webview_window.close().unwrap();

        let app_handle = match &self.app_hander {
            Some(handle) => handle,
            None => return Err("AppHandle not initialized".into()),
        };

        let label = format!("sspin-{}", self.max_pin_id);
        self.max_pin_id += 1;

        let monitor = match webview_window.current_monitor()? {
            Some(handle) => handle,
            None => return Err("Unable to get current monitor".into()),
        };
        let monitor_pos = monitor.position();
        let x = monitor_pos.x as f64 + offset_x;
        let y = monitor_pos.y as f64 + offset_y;

        let win_builder =
            WebviewWindowBuilder::new(app_handle, label, WebviewUrl::App("ScreenShotter/Pin".into()))
                .position(x, y)
                .inner_size(width, height)
                .always_on_top(true)
                .resizable(false)
                .decorations(false) 
                // .accept_first_mouse(true) // TODO: del with 
                .visible(false);
        let _window = win_builder.build()?;

        Ok(())
    }
}
