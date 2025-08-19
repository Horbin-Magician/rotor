pub mod shotter_record;

use crate::{core::config::AppConfig, module::screen_shotter::shotter_record::{ShotterRecord, ShotterConfig}};
use crate::module::Module;
use std::any::Any;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;
use image::{DynamicImage, RgbaImage};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::Shortcut;
use std::sync::Mutex;
use xcap::Monitor;

use crate::util::i18n;
#[cfg(target_os = "windows")]
use crate::util::sys_util;

pub struct ScreenShotter {
    app_hander: Option<tauri::AppHandle>,
    pub masks: Arc<Mutex<HashMap<String, RgbaImage>>>,
    pub shotter_recort: shotter_record::ShotterRecord,
    max_pin_id: u32,
}

impl Module for ScreenShotter {
    fn flag(&self) -> &str {
        "screenshot"
    }

    fn init(&mut self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        self.app_hander = Some(app.clone());
        self.build_mask_windows()?; // Pre-build mask window for faster response
        self.restore_pin_wins();
        Ok(())
    }

    fn run(&mut self) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_hander {
            Some(handle) => handle,
            None => return Err("AppHandle not initialized".into()),
        };

        // Capture screen
        for monitor in Monitor::all()? {
            let masks_clone = Arc::clone(&self.masks);
            let label = format!("ssmask-{}", monitor.id()?);
            tauri::async_runtime::spawn(async move {
                let masks_clone = Arc::clone(&masks_clone);
                let mut masks = masks_clone.lock().unwrap();

                if let Ok(monitor) = Monitor::from_point(0, 0) {
                    if let Ok(img) = monitor.capture_image() {
                        masks.clear();
                        masks.insert(label, img);
                    }
                }
            });
        }

        app_handle.emit("show-mask", ()).unwrap();
        self.build_pin_window(None)?; // Pre-build pin window for faster response after mask is shown

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
        })
    }

    pub fn build_mask_windows(&mut self) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_hander {
            Some(handle) => handle,
            None => return Err("AppHandle not initialized".into()),
        };
        
        for monitor in Monitor::all()? {
            let label = format!("ssmask-{}", monitor.id()?);

            let win_builder = WebviewWindowBuilder::new(
                app_handle,
                &label,
                WebviewUrl::App("ScreenShotter/Mask".into()),
            )
            .position(monitor.x()? as f64, monitor.y()? as f64)
            .always_on_top(true)
            .resizable(false)
            .visible(false)
            .skip_taskbar(true);

            let window = win_builder.build()?;
            #[cfg(target_os = "windows")]
            {
                window.hwnd().map(|hwnd| {
                    sys_util::forbid_window_animation(hwnd);
                }).ok();
            }
            #[cfg(target_os = "macos")]
            {
                use cocoa::appkit::{NSWindow};
                use cocoa::base::id;
                let ns_window = window.ns_window().unwrap() as id;
                unsafe {
                    ns_window.setLevel_(i32::MAX as i64); // 更高层级
                }
            }
        }

        Ok(())
    }

    pub fn build_pin_window(&self, id: Option<u32>) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_hander {
            Some(handle) => handle,
            None => return Err("AppHandle not initialized".into()),
        };

        let mut set_id = self.max_pin_id;
        let mut pos_x = 0.0;
        let mut pos_y = 0.0;
        let mut width = 100.0;
        let mut height = 100.0;
        let mut minimized = false;
        if let Some(id) = id {
            if let Some(record) = self.shotter_recort.get_record(id) {
                set_id = id;
                pos_x = (record.pos_x + record.rect.0 as i32) as f64;
                pos_y = (record.pos_y + record.rect.1 as i32) as f64;
                width = record.rect.2 as f64;
                height = record.rect.3 as f64;
                minimized = record.minimized;
            }
        }
        let label = format!("sspin-{}", set_id);

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
            .visible(false);

            let window = win_builder.build()?;
            window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
                width: width as u32,
                height: height as u32,
            }))?;
            window.set_position(tauri::Position::Physical(tauri::PhysicalPosition {
                x: pos_x as i32,
                y: pos_y as i32,
            }))?;
            if minimized {
                let _ = window.minimize();
            }

            #[cfg(target_os = "windows")]
            window.hwnd().map(|hwnd| {
                sys_util::forbid_window_animation(hwnd);
            }).ok();
        }

        Ok(())
    }

    pub fn new_pin(
        &mut self,
        pos_x: i32,
        pos_y: i32,
        rect: (u32, u32, u32, u32),
        mask_label: String,
    ) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_hander {
            Some(handle) => handle.clone(),
            None => return Err("AppHandle not initialized".into()),
        };

        let pin_label = format!("sspin-{}", self.max_pin_id);
        let x = pos_x + rect.0 as i32;
        let y = pos_y + rect.1 as i32;

        let config = ShotterConfig {pos_x, pos_y, rect, zoom_factor: 100, mask_label, minimized: false};
        self.update_shotter_record(self.max_pin_id, config);

        app_handle.emit_to(&pin_label, "show-pin", (x, y, rect.2, rect.3)).unwrap();
        self.max_pin_id += 1;

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
                        record.rect.3
                    ).to_image();
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
            for (id_str, _config) in records {
                if let Ok(id) = id_str.parse::<u32>() {
                    max_id = max_id.max(id);
                    let _ = self.build_pin_window(Some(id));
                }
            }
        }

        self.max_pin_id = max_id + 1; // Update max_pin_id to ensure new pins don't conflict with restored ones
    }
}
