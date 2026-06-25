use image::ImageFormat;
use std::io::Cursor;
use std::path::PathBuf;
use tauri::path::BaseDirectory;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

use rotor_common::AppConfig;
use rotor_platform::sys_util;
use rotor_runtime::Application;
use rotor_screenshot::img_util::{self, TextResult};
use rotor_screenshot::shotter_record::ShotterConfig;

struct SaveImageConfig {
    save_path: String,
    auto_update_save_path: bool,
    ask_save_path: bool,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinSelectionUpdate {
    id: u32,
    rect_x: u32,
    rect_y: u32,
    width: u32,
    height: u32,
    window_x: i32,
    window_y: i32,
    zoom: u32,
    minimized: bool,
}

fn lock_app() -> std::sync::MutexGuard<'static, Application> {
    Application::lock_global()
}

// Command for mask window
#[tauri::command]
pub async fn get_screen_rects(
    label: String,
    window: tauri::WebviewWindow,
) -> Vec<(i32, i32, i32, u32, u32)> {
    let Some(monitor) = window.current_monitor().ok().flatten() else {
        log::warn!("Unable to get current monitor for screen rects");
        return Vec::new();
    };
    let scale_factor = monitor.scale_factor();
    let mon_pos: tauri::LogicalPosition<i32> = monitor.position().to_logical(scale_factor);
    let mon_size: tauri::LogicalSize<i32> = monitor.size().to_logical(scale_factor);

    let raw_rects = match sys_util::get_all_window_rect() {
        Ok(rects) => rects,
        Err(error) => {
            log::warn!("Failed to get window rects: {error}");
            Vec::new()
        }
    };
    let mut rects = Vec::new();

    for rect in raw_rects {
        let (rect_x, rect_y, rect_z, rect_width, rect_height) = rect;

        // Calculate rect bounds
        let rect_right = rect_x + rect_width as i32;
        let rect_bottom = rect_y + rect_height as i32;

        // Calculate monitor bounds
        let mon_right = mon_pos.x + mon_size.width;
        let mon_bottom = mon_pos.y + mon_size.height;

        // Check if rect intersects with monitor
        if rect_right > mon_pos.x
            && rect_x < mon_right
            && rect_bottom > mon_pos.y
            && rect_y < mon_bottom
        {
            // Clip rect to monitor bounds
            let clipped_x = rect_x.max(mon_pos.x);
            let clipped_y = rect_y.max(mon_pos.y);
            let clipped_right = rect_right.min(mon_right);
            let clipped_bottom = rect_bottom.min(mon_bottom);

            // Calculate clipped dimensions
            let clipped_width = (clipped_right - clipped_x) as u32;
            let clipped_height = (clipped_bottom - clipped_y) as u32;

            // Convert to monitor-relative coordinates
            let relative_x = clipped_x - mon_pos.x;
            let relative_y = clipped_y - mon_pos.y;

            rects.push((
                relative_x,
                relative_y,
                rect_z,
                clipped_width,
                clipped_height,
            ));
        }
    }

    let image = {
        let app = lock_app();
        app.screenshot.get_capture(&label)
    };

    if let Some(image) = image {
        let rects2 = tokio::task::spawn_blocking(move || img_util::detect_rect(&image))
            .await
            .unwrap_or_default();
        for rect in rects2 {
            rects.push((
                (rect.0 as f64 / scale_factor) as i32,
                (rect.1 as f64 / scale_factor) as i32,
                -1,
                (rect.2 as f64 / scale_factor) as u32,
                (rect.3 as f64 / scale_factor) as u32,
            ));
        }
    }

    rects
}

#[tauri::command]
pub async fn change_current_mask(handle: tauri::AppHandle) {
    rotor_screenshot::focus_mask_window_at_cursor(&handle);
}

#[tauri::command]
pub async fn is_screenshot_session_current(session_id: u32) -> bool {
    lock_app()
        .screenshot
        .is_screenshot_session_current(session_id)
}

#[tauri::command]
pub async fn finish_screenshot_session() {
    if let Err(error) = lock_app().screenshot.finish_screenshot_session() {
        log::error!("Failed to finish screenshot session: {error}");
    }
}

#[tauri::command]
pub async fn cancel_screenshot_session() {
    if let Err(error) = lock_app().screenshot.cancel_screenshot_session() {
        log::error!("Failed to cancel screenshot session: {error}");
    }
}

#[tauri::command]
pub async fn new_pin(
    offset_x: f32,
    offset_y: f32,
    width: f32,
    height: f32,
    webview_window: tauri::WebviewWindow,
) {
    if let Some(monitor) = webview_window.current_monitor().ok().flatten() {
        let monitor_pos = monitor.position();
        let monitor_size = monitor.size();

        let rect = (
            offset_x.max(0.0).round() as u32,
            offset_y.max(0.0).round() as u32,
            width.max(0.0).round() as u32,
            height.max(0.0).round() as u32,
        );
        let monitor_position = (monitor_pos.x, monitor_pos.y);
        let monitor_size = (monitor_size.width, monitor_size.height);
        let offset = (0, 0); // Default offset, can be adjusted later

        if let Err(error) = lock_app().screenshot.new_pin(
            monitor_position,
            monitor_size,
            rect,
            offset,
            webview_window.label().to_string(),
        ) {
            log::error!("Failed to create pin: {error}");
        }
    } else {
        log::error!("Unable to get current monitor");
    }
}

#[tauri::command]
pub async fn new_cache_pin(window: tauri::WebviewWindow) {
    match window.outer_position() {
        Ok(pos) => {
            if let Err(error) = lock_app().screenshot.new_cache_pin(pos.x, pos.y) {
                log::error!("Failed to create cache pin: {error}");
            }
        }
        Err(error) => {
            log::error!("Failed to get mask window position: {error}");
        }
    }
}

#[tauri::command]
pub async fn close_cache_pin() {
    if let Err(error) = lock_app().screenshot.close_cache_pin() {
        log::error!("Failed to close cache pin: {error}");
    }
}

#[tauri::command]
pub async fn clear_screenshot_cache() {
    lock_app().screenshot.clear_captures();
}

// Command for pin window

#[tauri::command]
pub async fn get_pin_state(id: u32) -> Option<ShotterConfig> {
    lock_app().screenshot.get_pin_record(id)
}

#[tauri::command]
pub async fn update_pin_state(id: u32, x: i32, y: i32, zoom: u32, minimized: bool) {
    let mut app = lock_app();
    if let Some(mut record) = app.screenshot.get_pin_record(id) {
        record.minimized = minimized;
        // Calculate new offset based on current position and monitor position
        record.offset.0 = x - record.monitor_pos.0 - record.rect.0 as i32;
        record.offset.1 = y - record.monitor_pos.1 - record.rect.1 as i32;
        record.zoom_factor = zoom;
        if let Err(error) = app.screenshot.update_shotter_record(id, record) {
            log::error!("Failed to update pin state {id}: {error}");
        }
    }
}

#[tauri::command]
pub async fn update_pin_selection(selection: PinSelectionUpdate) {
    if selection.width == 0 || selection.height == 0 {
        log::warn!("Ignoring empty pin selection update for {}", selection.id);
        return;
    }

    let mut app = lock_app();
    if let Some(mut record) = app.screenshot.get_pin_record(selection.id) {
        record.rect = (
            selection.rect_x,
            selection.rect_y,
            selection.width,
            selection.height,
        );
        record.offset.0 = selection.window_x - record.monitor_pos.0 - selection.rect_x as i32;
        record.offset.1 = selection.window_y - record.monitor_pos.1 - selection.rect_y as i32;
        record.zoom_factor = selection.zoom;
        record.minimized = selection.minimized;
        if let Err(error) = app.screenshot.update_shotter_record(selection.id, record) {
            log::error!("Failed to update pin selection {}: {error}", selection.id);
        }
    }
}

#[tauri::command]
pub async fn delete_pin_record(id: u32) {
    if let Err(e) = lock_app().screenshot.delete_pin_record(id) {
        log::error!("Failed to delete pin record {}: {}", id, e);
    }
}

#[tauri::command]
pub async fn save_img(img_buf: Vec<u8>, app: tauri::AppHandle) -> bool {
    let config = {
        let app_config = AppConfig::lock_global();

        SaveImageConfig {
            save_path: app_config.get("save_path").cloned().unwrap_or_default(),
            auto_update_save_path: app_config
                .get("if_auto_change_save_path")
                .is_none_or(|value| value == "true"),
            ask_save_path: app_config
                .get("if_ask_save_path")
                .is_none_or(|value| value == "true"),
        }
    };

    let file_name = chrono::Local::now()
        .format("Rotor_%Y-%m-%d-%H-%M-%S.png")
        .to_string();

    let file_path = if config.ask_save_path || config.save_path.is_empty() {
        // Run the native dialog off the async runtime so it does not block a worker
        // thread while waiting for user interaction.
        let app = app.clone();
        let save_path = config.save_path.clone();
        tauri::async_runtime::spawn_blocking(move || {
            app.dialog()
                .file()
                .set_directory(&save_path)
                .add_filter("PNG", &["png"])
                .set_file_name(file_name)
                .blocking_save_file()
                .and_then(|v| v.into_path().ok())
        })
        .await
        .unwrap_or(None)
    } else {
        Some(PathBuf::from(&config.save_path).join(file_name))
    };

    if let Some(file_path) = file_path {
        if !is_png(&img_buf) {
            log::error!("Refusing to save non-PNG screenshot data");
            return false;
        }

        if config.auto_update_save_path {
            update_save_path_from_file(&file_path);
        }

        if let Err(error) = std::fs::write(&file_path, &img_buf) {
            log::error!("Failed to save image: {error}");
            return false;
        }

        return true;
    }
    false
}

fn is_png(bytes: &[u8]) -> bool {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    bytes.starts_with(PNG_SIGNATURE)
}

fn update_save_path_from_file(file_path: &std::path::Path) {
    let Some(parent) = file_path.parent() else {
        return;
    };

    let mut app_config = AppConfig::lock_global();
    if let Err(error) = app_config.set(
        "save_path".to_string(),
        parent.to_string_lossy().to_string(),
    ) {
        log::warn!("Failed to update save path: {error}");
    }
}

#[tauri::command]
pub async fn img2text(img_buf: Vec<u8>, app: tauri::AppHandle) -> Vec<TextResult> {
    let model_path = match app.path().resolve("assets/model", BaseDirectory::Resource) {
        Ok(path) => path,
        Err(error) => {
            log::error!("Failed to resolve model path: {error}");
            return Vec::new();
        }
    };

    tokio::task::spawn_blocking(move || {
        let cursor = Cursor::new(img_buf);
        let img = match image::load(cursor, ImageFormat::Png) {
            Ok(img) => img,
            Err(error) => {
                log::error!("Failed to load image from buffer: {error}");
                return Vec::new();
            }
        };

        img_util::img2text(&model_path, &img).unwrap_or_else(|error| {
            log::error!("Failed to run OCR: {error}");
            Vec::new()
        })
    })
    .await
    .unwrap_or_else(|error| {
        log::error!("OCR task failed: {error}");
        Vec::new()
    })
}
