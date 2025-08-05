use crate::core::config::AppConfig;
use crate::module::Module;
use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;
use image::RgbaImage;
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::Shortcut;
use tokio::sync::Mutex;
use xcap::Monitor;

use crate::util::i18n;
#[cfg(target_os = "windows")]
use crate::util::sys_util;

pub struct ScreenShotter {
    app_hander: Option<tauri::AppHandle>,
    pub masks: Arc<Mutex<HashMap<String, RgbaImage>>>,
    pub pin_mask_label: String,
    max_pin_id: u8,
    mask_window_label: String,
}

impl Module for ScreenShotter {
    fn flag(&self) -> &str {
        "screenshot"
    }

    fn init(&mut self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        self.app_hander = Some(app.clone());
        self.build_mask_windows()?; // Pre-build mask window for faster response
        Ok(())
    }

    fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_hander {
            Some(handle) => handle,
            None => return Err("AppHandle not initialized".into()),
        };

        // Capture screen
        let masks_clone = Arc::clone(&self.masks);
        let label_clone = self.mask_window_label.clone();
        tauri::async_runtime::spawn(async move {
            let masks_clone = Arc::clone(&masks_clone);
            let label_clone = label_clone.clone();
            let mut masks = masks_clone.lock().await;

            if let Ok(monitor) = Monitor::from_point(0, 0) {
                if let Ok(img) = monitor.capture_image() {
                    masks.clear();
                    masks.insert(label_clone, img);
                }
            }
        });

        app_handle.emit("show-mask", ()).unwrap();
        self.build_pin_window()?; // Pre-build pin window for faster response after mask is shown

        Ok(())
    }

    fn get_shortcut(&self) -> Option<Shortcut> {
        let app_config = AppConfig::global().lock().unwrap();
        let shortcut = app_config.get(&"shortcut_screenshot".to_string()).cloned();
        drop(app_config);
        if let Some(shortcut_str) = shortcut {
            return Some(Shortcut::from_str(&shortcut_str).unwrap());
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
        Ok(ScreenShotter {
            app_hander: None,
            masks: Arc::new(Mutex::new(HashMap::new())),
            max_pin_id: 0,
            pin_mask_label: String::new(),
            mask_window_label: String::new(),
        })
    }

    pub fn build_mask_windows(&mut self) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_hander {
            Some(handle) => handle,
            None => return Err("AppHandle not initialized".into()),
        };

        let monitor = Monitor::from_point(0, 0)?;
        let label = format!("ssmask-0");
        self.mask_window_label = label.clone();

        let win_builder = WebviewWindowBuilder::new(
            app_handle,
            &label,
            WebviewUrl::App("ScreenShotter/Mask".into()),
        )
        .position(monitor.x()? as f64, monitor.y()? as f64)
        .always_on_top(true)
        .resizable(false)
        .decorations(false)
        .fullscreen(true)
        .visible(false)
        .skip_taskbar(true);
        #[cfg(target_os = "windows")]
        let window = win_builder.build()?;
        #[cfg(target_os = "macos")]
        let _window = win_builder.build()?;
        
        #[cfg(target_os = "windows")]
        window.hwnd().map(|hwnd| {
            sys_util::forbid_window_animation(hwnd);
        }).ok();

        Ok(())
    }

    pub fn build_pin_window(&mut self) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_hander {
            Some(handle) => handle,
            None => return Err("AppHandle not initialized".into()),
        };

        let label = format!("sspin-{}", self.max_pin_id);

        if app_handle.get_webview_window(&label) == None {
            let win_builder = WebviewWindowBuilder::new(
                app_handle,
                &label,
                WebviewUrl::App("ScreenShotter/Pin".into()),
            )
            .title(&i18n::t("pinWindowName"))
            .position(0.0, 0.0)
            .inner_size(100.0, 100.0)
            .always_on_top(true)
            .resizable(false)
            .decorations(false)
            .visible(false);
            #[cfg(target_os = "windows")]
            let window = win_builder.build()?;
            #[cfg(target_os = "macos")]
            let _window = win_builder.build()?;

            #[cfg(target_os = "windows")]
            window.hwnd().map(|hwnd| {
                sys_util::forbid_window_animation(hwnd);
            }).ok();
        }

        Ok(())
    }

    pub fn new_pin(
        &mut self,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        label: String,
    ) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_hander {
            Some(handle) => handle,
            None => return Err("AppHandle not initialized".into()),
        };

        self.pin_mask_label = label;
        let pin_label = format!("sspin-{}", self.max_pin_id);
        app_handle.emit_to(&pin_label, "show-pin", (x, y, width, height)).unwrap();
        self.max_pin_id += 1;

        Ok(())
    }
}
