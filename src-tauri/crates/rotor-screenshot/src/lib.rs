mod capture_cache;
pub mod img_util;
mod monitor;
mod platform;
pub mod shotter_record;

use crate::capture_cache::CaptureCache;
use crate::monitor::{capture_all, current_configs, mask_label, sorted_configs, MonitorConfig};
use crate::platform::{disable_window_animation, prepare_overlay_window, raise_overlay_window};
use crate::shotter_record::{ShotterConfig, ShotterRecord};
use image::{DynamicImage, RgbaImage};
use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;
use tauri::{Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::Shortcut;
use xcap::Monitor;

use rotor_common::{i18n, AppConfig};
use rotor_platform::sys_util;

pub fn focus_mask_window_at_cursor(app_handle: &tauri::AppHandle) {
    let cursor_position = match sys_util::get_cursor_position() {
        Ok(position) => position,
        Err(err) => {
            log::warn!("Failed to get cursor position for mask focus: {err}");
            return;
        }
    };

    let monitor = match Monitor::from_point(cursor_position.0, cursor_position.1) {
        Ok(monitor) => monitor,
        Err(err) => {
            log::warn!(
                "Failed to locate monitor at cursor ({}, {}): {err}",
                cursor_position.0,
                cursor_position.1
            );
            return;
        }
    };

    let monitor_id = match monitor.id() {
        Ok(id) => id,
        Err(err) => {
            log::warn!("Failed to get monitor id for mask focus: {err}");
            return;
        }
    };

    let label = mask_label(monitor_id);
    let Some(window) = app_handle.get_webview_window(&label) else {
        log::debug!("Mask window {label} is not available for focus");
        return;
    };

    if let Err(err) = window.set_focus() {
        log::warn!("Failed to focus mask window {label}: {err}");
    }

    if let Err(err) = raise_overlay_window(&window) {
        log::warn!("Failed to raise screenshot overlay window {label}: {err}");
    }
}

pub struct ScreenShotter {
    app_handle: Option<tauri::AppHandle>,
    capture_cache: CaptureCache,
    shotter_record: ShotterRecord,
    max_pin_id: u32,
    current_monitors: Vec<MonitorConfig>,
}

impl ScreenShotter {
    pub fn flag(&self) -> &str {
        "screenshot"
    }

    pub fn new() -> Self {
        Self {
            app_handle: None,
            capture_cache: CaptureCache::new(),
            shotter_record: ShotterRecord::new(),
            max_pin_id: 0,
            current_monitors: Vec::new(),
        }
    }

    pub fn init(&mut self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        self.app_handle = Some(app.clone());
        self.update_monitor_config()?;
        self.build_mask_windows()?;
        self.restore_pin_wins();
        Ok(())
    }

    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        self.check_and_rebuild_mask_windows()?;
        self.capture_cache.clear();

        let captures = capture_all(Monitor::all()?);
        if captures.is_empty() {
            return Err("No screenshot images captured".into());
        }

        self.capture_cache.replace_all(captures);
        self.app_handle()?.emit("show-mask", ())?;
        Ok(())
    }

    pub fn get_shortcut(&self) -> Option<Shortcut> {
        let app_config = AppConfig::lock_global();
        let shortcut = app_config.get("shortcut_screenshot")?.clone();
        drop(app_config);

        match Shortcut::from_str(&shortcut) {
            Ok(shortcut) => Some(shortcut),
            Err(error) => {
                log::warn!("Invalid screenshot shortcut `{shortcut}`: {error}");
                None
            }
        }
    }

    pub fn get_capture(&self, label: &str) -> Option<Arc<RgbaImage>> {
        self.capture_cache.get(label)
    }

    pub fn get_pin_record(&self, id: u32) -> Option<ShotterConfig> {
        self.shotter_record.get_record(id).cloned()
    }

    pub fn update_shotter_record(
        &mut self,
        id: u32,
        config: ShotterConfig,
    ) -> Result<(), Box<dyn Error>> {
        self.shotter_record.update_shotter(id, config)
    }

    pub fn delete_pin_record(&mut self, id: u32) -> Result<(), Box<dyn Error>> {
        self.shotter_record.del_shotter(id)
    }

    pub fn new_pin(
        &mut self,
        monitor_pos: (i32, i32),
        monitor_size: (u32, u32),
        rect: (u32, u32, u32, u32),
        offset: (i32, i32),
        mask_label: String,
    ) -> Result<(), Box<dyn Error>> {
        let config = ShotterConfig {
            monitor_pos,
            monitor_size,
            rect,
            offset,
            zoom_factor: 100,
            mask_label,
            minimized: false,
        };

        let pin_id = self.max_pin_id;
        self.build_pin_window(
            Some(pin_id),
            Some(PhysicalPosition {
                x: monitor_pos.0 + rect.0 as i32 + offset.0,
                y: monitor_pos.1 + rect.1 as i32 + offset.1,
            }),
        )?;
        self.update_shotter_record(pin_id, config)?;
        let pin_label = format!("sspin-{pin_id}");
        self.app_handle()?.emit_to(&pin_label, "show-pin", ())?;
        self.max_pin_id = self.max_pin_id.saturating_add(1);
        Ok(())
    }

    pub fn new_cache_pin(&mut self, x: i32, y: i32) -> Result<(), Box<dyn Error>> {
        self.build_pin_window(None, Some(PhysicalPosition { x, y }))
    }

    pub fn close_cache_pin(&mut self) -> Result<(), Box<dyn Error>> {
        let pin_label = format!("sspin-{}", self.max_pin_id);
        if let Some(win) = self.app_handle()?.get_webview_window(&pin_label) {
            if let Err(error) = win.close() {
                log::warn!("Failed to close cache pin window {pin_label}: {error}");
            }
        }
        Ok(())
    }

    pub fn get_pin_img(&self, id: u32) -> Option<DynamicImage> {
        if let Ok(img) = ShotterRecord::load_record_img(id) {
            return Some(img);
        }

        let record = self.shotter_record.get_record(id)?;
        let img = self.capture_cache.get(&record.mask_label)?;
        let dyn_img = DynamicImage::ImageRgba8(img.as_ref().clone());
        let _save_task = ShotterRecord::save_record_img(id, dyn_img.clone());
        Some(dyn_img)
    }

    pub fn restore_pin_wins(&mut self) {
        let mut max_id = 0u32;
        let mut invalid_ids = Vec::new();
        let records = self.shotter_record.get_records().clone();

        for (id_str, record) in records {
            let Ok(id) = id_str.parse::<u32>() else {
                continue;
            };

            if ShotterRecord::load_record_img(id).is_err() {
                invalid_ids.push(id);
                continue;
            }

            max_id = max_id.max(id);
            let position = PhysicalPosition {
                x: record.monitor_pos.0 + record.rect.0 as i32 + record.offset.0,
                y: record.monitor_pos.1 + record.rect.1 as i32 + record.offset.1,
            };

            if let Err(error) = self.build_pin_window(Some(id), Some(position)) {
                log::error!("Failed to restore pin window {id}: {error}");
            }
        }

        for id in invalid_ids {
            if let Err(error) = self.shotter_record.del_shotter(id) {
                log::warn!("Failed to remove invalid pin record {id}: {error}");
            }
        }

        self.max_pin_id = max_id.saturating_add(1);
    }

    fn app_handle(&self) -> Result<&tauri::AppHandle, Box<dyn Error>> {
        self.app_handle
            .as_ref()
            .ok_or_else(|| Box::<dyn Error>::from("AppHandle not initialized"))
    }

    fn monitors_have_changed(&self) -> Result<bool, Box<dyn Error>> {
        let current = sorted_configs(current_configs()?);
        let stored = sorted_configs(self.current_monitors.clone());
        Ok(current != stored)
    }

    fn update_monitor_config(&mut self) -> Result<(), Box<dyn Error>> {
        let old_monitors = self.current_monitors.clone();
        let new_monitors = current_configs()?;

        for old_monitor in old_monitors {
            if new_monitors
                .iter()
                .any(|new_monitor| new_monitor.id == old_monitor.id)
            {
                continue;
            }

            let label = mask_label(old_monitor.id);
            if let Some(window) = self.app_handle()?.get_webview_window(&label) {
                if let Err(error) = window.close() {
                    log::warn!("Failed to close mask window {label}: {error}");
                }
            }
        }

        self.current_monitors = new_monitors;
        Ok(())
    }

    fn check_and_rebuild_mask_windows(&mut self) -> Result<(), Box<dyn Error>> {
        if self.monitors_have_changed()? {
            self.update_monitor_config()?;
        }
        self.build_mask_windows()
    }

    fn build_mask_windows(&mut self) -> Result<(), Box<dyn Error>> {
        if self.current_monitors.is_empty() {
            self.update_monitor_config()?;
        }

        let app_handle = self.app_handle()?;
        for monitor in &self.current_monitors {
            let label = mask_label(monitor.id);
            let position = tauri::Position::Physical(tauri::PhysicalPosition {
                x: monitor.x,
                y: monitor.y,
            });

            if let Some(window) = app_handle.get_webview_window(&label) {
                if let Err(error) = window.set_position(position) {
                    log::warn!("Failed to update mask window {label} position: {error}");
                }
                continue;
            }

            let window = WebviewWindowBuilder::new(
                app_handle,
                &label,
                WebviewUrl::App("ScreenShotter/Mask".into()),
            )
            .always_on_top(true)
            .resizable(false)
            .visible(false)
            .accept_first_mouse(true)
            .shadow(false)
            .skip_taskbar(true)
            .transparent(true)
            .build()?;

            disable_window_animation(&window);
            prepare_overlay_window(&window)?;
            window.set_position(position)?;
        }

        Ok(())
    }

    fn build_pin_window(
        &self,
        id: Option<u32>,
        pos: Option<PhysicalPosition<i32>>,
    ) -> Result<(), Box<dyn Error>> {
        let app_handle = self.app_handle()?;
        let set_id = id.unwrap_or(self.max_pin_id);
        let label = format!("sspin-{set_id}");

        if app_handle.get_webview_window(&label).is_some() {
            return Ok(());
        }

        let (x, y) = get_logical_position(pos)?;
        let window = WebviewWindowBuilder::new(
            app_handle,
            &label,
            WebviewUrl::App("ScreenShotter/Pin".into()),
        )
        .title(i18n::t("pinWindowName"))
        .always_on_top(true)
        .resizable(false)
        .decorations(false)
        .position(x, y)
        .visible(false)
        .transparent(true)
        .build()?;

        disable_window_animation(&window);
        refocus_mask_after_pin_build(app_handle, pos);
        Ok(())
    }
}

fn get_logical_position(pos: Option<PhysicalPosition<i32>>) -> Result<(f64, f64), Box<dyn Error>> {
    let Some(pos) = pos else {
        return Ok((0.0, 0.0));
    };

    let Ok(monitor) = Monitor::from_point(pos.x, pos.y) else {
        log::warn!(
            "Failed to locate monitor at ({}, {}) for pin window position",
            pos.x,
            pos.y
        );
        return Ok((0.0, 0.0));
    };

    let scale_factor = match monitor.scale_factor() {
        Ok(scale_factor) => f64::from(scale_factor),
        Err(error) => {
            log::warn!("Failed to get monitor scale factor for pin window position: {error}");
            return Ok((0.0, 0.0));
        }
    };

    Ok((pos.x as f64 / scale_factor, pos.y as f64 / scale_factor))
}

fn refocus_mask_after_pin_build(app_handle: &tauri::AppHandle, pos: Option<PhysicalPosition<i32>>) {
    #[cfg(target_os = "windows")]
    {
        let Some(pos) = pos else {
            return;
        };

        let Ok(monitor) = Monitor::from_point(pos.x, pos.y) else {
            return;
        };

        let Ok(monitor_id) = monitor.id() else {
            return;
        };

        if let Some(window) = app_handle.get_webview_window(&mask_label(monitor_id)) {
            let _ = window.set_focus();
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = app_handle;
        let _ = pos;
    }
}
