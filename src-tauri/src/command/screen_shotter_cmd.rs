use tauri::path::BaseDirectory;
use tauri::Manager;
use tauri_plugin_dialog::DialogExt;

use rotor_common::AppConfig;
use rotor_platform::sys_util;
use rotor_runtime::Application;
use rotor_screenshot::img_util::{self, TextResult};
use rotor_screenshot::shotter_record::ShotterConfig;

// Common functions
pub async fn try_get_screen_img(
    label: &str,
) -> Option<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>> {
    let mut try_times = 1;
    while try_times <= 20 {
        // Get the masks arc without holding any locks
        let masks_arc = {
            Application::global()
                .lock()
                .unwrap()
                .screenshot
                .masks
                .clone()
        };
        {
            let masks = masks_arc.lock().unwrap();
            if let Some(m) = masks.get(label) {
                return Some(m.clone());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await; // Shorter delay between retries for faster response
        try_times += 1;
    }
    None
}

// Command for mask window
#[tauri::command]
pub async fn get_screen_rects(
    label: String,
    window: tauri::WebviewWindow,
) -> Vec<(i32, i32, i32, u32, u32)> {
    let monitor = window.current_monitor().unwrap().unwrap();
    let scale_factor = monitor.scale_factor();
    let mon_pos: tauri::LogicalPosition<i32> = monitor.position().to_logical(scale_factor);
    let mon_size: tauri::LogicalSize<i32> = monitor.size().to_logical(scale_factor);

    let raw_rects = sys_util::get_all_window_rect().unwrap();
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

    if let Some(image) = try_get_screen_img(&label).await {
        let rects2 = tokio::task::spawn_blocking(move || img_util::detect_rect(image))
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
pub async fn new_pin(
    offset_x: String,
    offset_y: String,
    width: String,
    height: String,
    webview_window: tauri::WebviewWindow,
) {
    if let Some(monitor) = webview_window.current_monitor().ok().flatten() {
        let monitor_pos = monitor.position();
        let monitor_size = monitor.size();
        let Ok(offset_x_val) = offset_x.parse::<f32>() else {
            log::error!("Failed to parse offset_x: {}", offset_x);
            return;
        };
        let Ok(offset_y_val) = offset_y.parse::<f32>() else {
            log::error!("Failed to parse offset_y: {}", offset_y);
            return;
        };
        let Ok(width_val) = width.parse::<f32>() else {
            log::error!("Failed to parse width: {}", width);
            return;
        };
        let Ok(height_val) = height.parse::<f32>() else {
            log::error!("Failed to parse height: {}", height);
            return;
        };

        let rect = (
            offset_x_val as u32,
            offset_y_val as u32,
            width_val as u32,
            height_val as u32,
        );
        let monitor_position = (monitor_pos.x, monitor_pos.y);
        let monitor_size = (monitor_size.width, monitor_size.height);
        let offset = (0, 0); // Default offset, can be adjusted later

        Application::global()
            .lock()
            .unwrap()
            .screenshot
            .new_pin(
                monitor_position,
                monitor_size,
                rect,
                offset,
                webview_window.label().to_string(),
            )
            .unwrap();
    } else {
        log::error!("Unable to get current monitor");
    }
}

#[tauri::command]
pub async fn new_cache_pin(window: tauri::WebviewWindow) {
    let mut app = Application::global().lock().unwrap();
    let _ = window.outer_position().map(|pos| {
        app.screenshot.new_cache_pin(pos.x, pos.y).unwrap();
    });
}

#[tauri::command]
pub async fn close_cache_pin() {
    Application::global()
        .lock()
        .unwrap()
        .screenshot
        .close_cache_pin()
        .unwrap();
}

// Command for pin window

#[tauri::command]
pub async fn get_pin_state(id: u32) -> Option<ShotterConfig> {
    Application::global()
        .lock()
        .unwrap()
        .screenshot
        .shotter_recort
        .get_record(id)
        .cloned()
}

#[tauri::command]
pub async fn update_pin_state(id: u32, x: i32, y: i32, zoom: u32, minimized: bool) {
    let mut app = Application::global().lock().unwrap();
    if let Some(mut record) = app.screenshot.shotter_recort.get_record(id).cloned() {
        record.minimized = minimized;
        // Calculate new offset based on current position and monitor position
        record.offset.0 = x - record.monitor_pos.0 - record.rect.0 as i32;
        record.offset.1 = y - record.monitor_pos.1 - record.rect.1 as i32;
        record.zoom_factor = zoom;
        app.screenshot.update_shotter_record(id, record);
    }
}

#[tauri::command]
pub async fn update_pin_selection(
    id: u32,
    rect_x: u32,
    rect_y: u32,
    width: u32,
    height: u32,
    window_x: i32,
    window_y: i32,
    zoom: u32,
    minimized: bool,
) {
    if width == 0 || height == 0 {
        log::warn!("Ignoring empty pin selection update for {}", id);
        return;
    }

    let mut app = Application::global().lock().unwrap();
    if let Some(mut record) = app.screenshot.shotter_recort.get_record(id).cloned() {
        record.rect = (rect_x, rect_y, width, height);
        record.offset.0 = window_x - record.monitor_pos.0 - rect_x as i32;
        record.offset.1 = window_y - record.monitor_pos.1 - rect_y as i32;
        record.zoom_factor = zoom;
        record.minimized = minimized;
        app.screenshot.update_shotter_record(id, record);
    }
}

#[tauri::command]
pub async fn delete_pin_record(id: u32) {
    let mut app = Application::global().lock().unwrap();
    if let Err(e) = app.screenshot.shotter_recort.del_shotter(id) {
        log::error!("Failed to delete pin record {}: {}", id, e);
    }
}

#[tauri::command]
pub async fn save_img(img_buf: Vec<u8>, app: tauri::AppHandle) -> bool {
    let Ok(mut app_config) = AppConfig::global().lock() else {
        log::error!("Failed to acquire config lock");
        return false;
    };
    let save_path = app_config
        .get(&"save_path".to_string())
        .cloned()
        .unwrap_or_default();
    let if_auto_change = app_config
        .get(&"if_auto_change_save_path".to_string())
        .cloned()
        .unwrap_or("true".to_string());
    let if_ask_path = app_config
        .get(&"if_ask_save_path".to_string())
        .cloned()
        .unwrap_or("true".to_string());

    let file_name = chrono::Local::now()
        .format("Rotor_%Y-%m-%d-%H-%M-%S.png")
        .to_string();

    let file_path: Option<std::path::PathBuf> = if (if_ask_path == "true") || (save_path.is_empty())
    {
        app.dialog()
            .file()
            .set_directory(save_path)
            .add_filter("PNG", &["png"])
            .set_file_name(file_name)
            .blocking_save_file()
            .and_then(|v| v.into_path().ok())
    } else {
        Some(std::path::PathBuf::from(save_path))
    };

    if let Some(file_path) = file_path {
        if if_auto_change == "true" {
            if let Some(parent) = file_path.parent() {
                if let Err(e) = app_config.set(
                    "save_path".to_string(),
                    parent.to_string_lossy().to_string(),
                ) {
                    log::warn!("Failed to update save path: {}", e);
                }
            }
        }
        let cursor = std::io::Cursor::new(img_buf);
        if let Ok(img) = image::load(cursor, image::ImageFormat::Png) {
            if let Err(e) = img.save(&file_path) {
                log::error!("Failed to save image: {}", e);
                return false;
            }
            return true;
        }
    }
    drop(app_config);
    false
}

#[tauri::command]
pub async fn img2text(img_buf: Vec<u8>, app: tauri::AppHandle) -> Vec<TextResult> {
    let cursor = std::io::Cursor::new(img_buf);
    if let Ok(img) = image::load(cursor, image::ImageFormat::Png) {
        let model_path = app.path().resolve("assets/model", BaseDirectory::Resource);
        if let Ok(path) = model_path {
            return img_util::img2text(&path, &img);
        } else {
            log::error!("Failed to resolve model path");
        }
    } else {
        log::error!("Failed to load image from buffer");
    }
    Vec::new()
}
