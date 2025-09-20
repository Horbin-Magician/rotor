pub mod shotter_record;

use crate::module::Module;
use crate::{
    core::config::AppConfig,
    module::screen_shotter::shotter_record::{ShotterConfig, ShotterRecord},
};
use image::{DynamicImage, RgbaImage};
use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::Shortcut;
use xcap::Monitor;

use crate::util::i18n;
#[cfg(target_os = "windows")]
use crate::util::sys_util;

#[derive(Debug, Clone, PartialEq)]
struct MonitorConfig {
    id: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale_factor: f32,
}

impl MonitorConfig {
    fn from_monitor(monitor: &Monitor) -> Result<Self, Box<dyn Error>> {
        Ok(MonitorConfig {
            id: monitor.id()?,
            x: monitor.x()?,
            y: monitor.y()?,
            width: monitor.width()?,
            height: monitor.height()?,
            scale_factor: monitor.scale_factor()?,
        })
    }
}

pub struct ScreenShotter {
    app_hander: Option<tauri::AppHandle>,
    pub masks: Arc<Mutex<HashMap<String, RgbaImage>>>,
    pub shotter_recort: shotter_record::ShotterRecord,
    max_pin_id: u32,
    current_monitors: Vec<MonitorConfig>,
}

impl Module for ScreenShotter {
    fn flag(&self) -> &str {
        "screenshot"
    }

    fn init(&mut self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        self.app_hander = Some(app.clone());
        
        self.update_monitor_config()?; // Store initial monitor configuration
        
        self.build_mask_windows()?; // Pre-build mask window for faster response
        self.restore_pin_wins();
        Ok(())
    }

    fn run(&mut self) -> Result<(), Box<dyn Error>> {
        self.check_and_rebuild_mask_windows()?; // Check if monitors have changed, if changed, rebuild mask windows

        let app_handle = match &self.app_hander {
            Some(handle) => handle.clone(),
            None => return Err("AppHandle not initialized".into()),
        };

        let mut mask = self.masks.lock().unwrap();
        mask.clear();
        drop(mask);

        // Capture screen
        #[cfg(target_os = "windows")]
        for monitor in Monitor::all()? {
            let masks_clone = Arc::clone(&self.masks);
            let pos_x = monitor.x()?;
            let pox_y = monitor.y()?;
            tauri::async_runtime::spawn(async move {
                let monitor = Monitor::from_point(pos_x, pox_y);
                if let Ok(monitor) = monitor {
                    if let Ok(img) = monitor.capture_image() {
                        let label = format!("ssmask-{}", monitor.id().unwrap());
                        let mut masks = masks_clone.lock().unwrap();
                        masks.insert(label.clone(), img);
                    } else {
                        log::error!("Failed to capture image for monitor at ({}, {})", pos_x, pox_y);
                    }
                } else {
                    log::error!("Failed to get monitor from point ({}, {})", pos_x, pox_y);
                }
            });
        }

        #[cfg(target_os = "macos")]
        for monitor in Monitor::all()? {
            let masks_clone = Arc::clone(&self.masks);
            tauri::async_runtime::spawn(async move {
                if let Ok(img) = monitor.capture_image() {
                    let label = format!("ssmask-{}", monitor.id().unwrap());
                    let mut masks = masks_clone.lock().unwrap();
                    masks.insert(label.clone(), img);
                } else {
                    log::error!("Failed to capture image for monitor id {}", monitor.id().unwrap_or_default());
                }
            });
        }

        app_handle.emit("show-mask", ()).unwrap();

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
            shotter_recort: ShotterRecord::new(),
            max_pin_id: 0,
            current_monitors: Vec::new(),
        })
    }

    /// Check if monitors have changed compared to stored configuration
    fn monitors_have_changed(&self) -> Result<bool, Box<dyn Error>> {
        let current_monitors = Monitor::all()?;
        
        // If the number of monitors changed, definitely changed
        if current_monitors.len() != self.current_monitors.len() {
            return Ok(true);
        }
        
        // Convert current monitors to our config format
        let mut current_configs = Vec::new();
        for monitor in current_monitors {
            current_configs.push(MonitorConfig::from_monitor(&monitor)?);
        }
        
        // Sort both vectors by monitor ID for comparison
        let mut stored_configs = self.current_monitors.clone();
        stored_configs.sort_by_key(|config| config.id);
        current_configs.sort_by_key(|config| config.id);
        
        // Compare configurations
        Ok(stored_configs != current_configs)
    }
    
    /// Update stored monitor configuration with current monitors
    fn update_monitor_config(&mut self) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_hander {
            Some(handle) => handle,
            None => return Err("AppHandle not initialized".into()),
        };

        let old_monitors = self.current_monitors.clone();

        let monitors = Monitor::all()?;
        self.current_monitors.clear();
        for monitor in monitors {
            self.current_monitors.push(MonitorConfig::from_monitor(&monitor)?);
        }

        for old_monitor in old_monitors {
            let mut if_del = true;
            for new_monitor in &self.current_monitors {
                if old_monitor.id == new_monitor.id {
                    if_del = false;
                    break;
                }
            }
            if if_del {
                let label = format!("ssmask-{}", old_monitor.id);
                if let Some(window) = app_handle.get_webview_window(&label) {
                    if let Err(e) = window.close() {
                        log::warn!("Failed to close mask window {}: {}", label, e);
                    }
                }
            }
        }
        Ok(())
    }
    
    /// Check if monitors have changed, if changed, rebuild mask windows
    fn check_and_rebuild_mask_windows(&mut self) -> Result<(), Box<dyn Error>> {
        if self.monitors_have_changed()? {
            self.update_monitor_config()?; // Update monitor configuration
            self.build_mask_windows()?; // Rebuild mask windows with new configuration
        }
        
        Ok(())
    }

    fn build_mask_windows(&mut self) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_hander {
            Some(handle) => handle,
            None => return Err("AppHandle not initialized".into()),
        };

        for monitor in Monitor::all()? {
            let label = format!("ssmask-{}", monitor.id()?);

            let mask_window = app_handle.get_webview_window(&label);
            if let Some(_) = mask_window {
                continue; // Window already exists, skip creation
            }

            let win_builder = WebviewWindowBuilder::new(
                app_handle,
                &label,
                WebviewUrl::App("ScreenShotter/Mask".into()),
            )
            .always_on_top(true)
            .resizable(false)
            .visible(false)
            .accept_first_mouse(true)
            .shadow(false)
            .skip_taskbar(true);

            let window = win_builder.build()?;

            #[cfg(target_os = "windows")]
            {
                window
                    .hwnd()
                    .map(|hwnd| {
                        sys_util::forbid_window_animation(hwnd);
                    })
                    .ok();
            }

            #[cfg(target_os = "macos")]
            {
                use cocoa::appkit::{NSMainMenuWindowLevel, NSWindow};
                use cocoa::base::id;
                let ns_window = window.ns_window().unwrap() as id;
                unsafe {
                    ns_window.setLevel_((NSMainMenuWindowLevel + 1) as i64);
                }
            }

            window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: monitor.x()?,
                y: monitor.y()?,
            }))?;
        }

        Ok(())
    }

    fn build_pin_window(&self, id: Option<u32>, pos: Option<PhysicalPosition<i32>>) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_hander {
            Some(handle) => handle,
            None => return Err("AppHandle not initialized".into()),
        };

        let set_id = if let Some(id) = id {
            id
        } else {
            self.max_pin_id
        };
        let label = format!("sspin-{}", set_id);

        let mut x = 0.0;
        let mut y = 0.0;
        if let Some(pos) = pos {
            if let Ok(monitor) = xcap::Monitor::from_point(pos.x, pos.y) {
                let scale_factor = monitor.scale_factor()?;
                x = pos.x as f64 / scale_factor as f64;
                y = pos.y as f64 / scale_factor as f64;
            }
        }

        // Only create the window if it doesn't already exist
        if app_handle.get_webview_window(&label).is_none() {
            let win_builder = WebviewWindowBuilder::new(
                app_handle,
                &label,
                WebviewUrl::App("ScreenShotter/Pin".into()),
            )
            .title(i18n::t("pinWindowName"))
            .always_on_top(true)
            .resizable(false)
            .decorations(false)
            .position(x, y)
            .visible(false);

            #[cfg(target_os = "windows")]
            {
                let window = win_builder.build()?;
                window
                    .hwnd()
                    .map(|hwnd| {
                        sys_util::forbid_window_animation(hwnd);
                    })
                    .ok();

                if let Some(pos) = pos { // just fix the focus influenced by the order of window creation
                    if let Ok(monitor) = xcap::Monitor::from_point(pos.x, pos.y) {
                        let label = format!("ssmask-{}", monitor.id().unwrap_or_default());
                        let window = app_handle.get_webview_window(&label);
                        if let Some(window) = window {
                            let _ = window.set_focus();
                        }
                    }
                }
            }

            #[cfg(target_os = "macos")]
            {
                let _window = win_builder.build()?;
            }
        }

        Ok(())
    }

    pub fn new_pin(
        &mut self,
        monitor_pos: (i32, i32),
        rect: (u32, u32, u32, u32),
        offset: (i32, i32),
        mask_label: String,
    ) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_hander {
            Some(handle) => handle.clone(),
            None => return Err("AppHandle not initialized".into()),
        };

        let config = ShotterConfig {
            monitor_pos,
            rect,
            offset,
            zoom_factor: 100,
            mask_label,
            minimized: false,
        };
        self.update_shotter_record(self.max_pin_id, config);

        let pin_label = format!("sspin-{}", self.max_pin_id);
        app_handle.emit_to(&pin_label, "show-pin", ()).unwrap();
        self.max_pin_id += 1;

        Ok(())
    }

    pub fn new_cache_pin(&mut self, x: i32, y: i32) -> Result<(), Box<dyn Error>> {
        self.build_pin_window(None, Some(PhysicalPosition { x, y }))?;
        Ok(())
    }

    pub fn close_cache_pin(&mut self) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_hander {
            Some(handle) => handle,
            None => return Err("AppHandle not initialized".into()),
        };

        let pin_label = format!("sspin-{}", self.max_pin_id);
        let pin_win = app_handle.get_webview_window(&pin_label);
        if let Some(win) = pin_win {
            win.close().unwrap();
        }

        Ok(())
    }

    pub fn get_pin_img(&self, id: u32) -> Option<DynamicImage> {
        if let Ok(img) = ShotterRecord::load_record_img(id) {
            return Some(img);
        } else {
            let record = self.shotter_recort.get_record(id);
            if let Some(record) = record {
                let image = {
                    let masks = self.masks.lock().unwrap();
                    masks.get(&record.mask_label).cloned()
                };

                if let Some(img) = image {
                    let cropped_img = image::imageops::crop_imm(
                        &img,
                        record.rect.0,
                        record.rect.1,
                        record.rect.2,
                        record.rect.3,
                    )
                    .to_image();
                    let dyn_img = DynamicImage::ImageRgba8(cropped_img);
                    ShotterRecord::save_record_img(id, dyn_img.clone());
                    return Some(dyn_img);
                }
            }
        }
        None
    }

    pub fn update_shotter_record(&mut self, id: u32, config: ShotterConfig) {
        if let Err(e) = self.shotter_recort.update_shotter(id, config) {
            log::error!("Failed to update shotter record {}: {}", id, e);
        }
    }

    pub fn restore_pin_wins(&mut self) {
        let mut max_id = 0u32;
        let records = self.shotter_recort.get_records();

        if let Some(records) = records {
            for (id_str, record) in records {
                if let Ok(id) = id_str.parse::<u32>() {
                    max_id = max_id.max(id);
                    self.build_pin_window(
                        Some(id), 
                        Some(PhysicalPosition { 
                            x: record.monitor_pos.0,
                            y: record.monitor_pos.1
                        })
                    ).unwrap_or_else(|e| {
                        log::error!("Failed to restore pin window {}: {}", id, e);
                    });
                }
            }
        }

        self.max_pin_id = max_id + 1; // Update max_pin_id to ensure new pins don't conflict with restored ones
    }
}
