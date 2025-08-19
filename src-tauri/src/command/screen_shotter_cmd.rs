use image::DynamicImage;
use log::warn;
use tauri_plugin_dialog::DialogExt;

use crate::core::application::Application;
use crate::core::config::AppConfig;
use crate::module::screen_shotter::ScreenShotter;
use crate::util::{img_util, sys_util};

#[tauri::command]
pub async fn get_screen_img(label: String) -> tauri::ipc::Response {
    let masks_arc = {
        let mut app = Application::global().lock().unwrap();
        app.get_module("screenshot")
            .and_then(|s| s.as_any().downcast_ref::<ScreenShotter>())
            .map(|screenshot| screenshot.masks.clone())
    };

    let image = if let Some(masks_arc) = masks_arc {
        let masks = masks_arc.lock().unwrap();
        masks.get(&label).cloned()
    } else {
        None
    };

    tauri::ipc::Response::new(image.unwrap_or_default().to_vec())
}

#[tauri::command]
pub async fn get_screen_rects(label: String, window: tauri::WebviewWindow) -> Vec<(i32, i32, i32, u32, u32)> {
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
        if rect_right > mon_pos.x && rect_x < mon_right && 
           rect_bottom > mon_pos.y && rect_y < mon_bottom {
            
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
            
            rects.push((relative_x, relative_y, rect_z, clipped_width, clipped_height));
        }
    }

    let masks_arc = {
        let mut app = Application::global().lock().unwrap();
        app.get_module("screenshot")
            .and_then(|s| s.as_any().downcast_ref::<ScreenShotter>())
            .map(|screenshot| screenshot.masks.clone())
    };

    let image = if let Some(masks_arc) = masks_arc {
        let masks = masks_arc.lock().unwrap();
        masks.get(&label).cloned()
    } else {
        None
    };

    if let Some(image) = image {
        let rects2 = img_util::detect_rect(&image);
        for rect in rects2 {
            let x = (rect.0 as f64 / scale_factor) as i32 + mon_pos.x;
            let y = (rect.1 as f64 / scale_factor) as i32 + mon_pos.y;
            rects.push((x, y, -1, (rect.2 as f64 / scale_factor) as u32, (rect.3 as f64 / scale_factor) as u32));
        }
    }

    rects
}

#[tauri::command]
pub async fn get_pin_img(id: String) -> tauri::ipc::Response {
    let mut app = Application::global().lock().unwrap();
    let mut img: Option<DynamicImage> = None;

    if let Some(ss) = app
        .get_module("screenshot")
        .and_then(|s| s.as_any().downcast_ref::<ScreenShotter>())
    {
        match id.parse::<u32>() {
            Ok(parsed_id) => img = ss.get_pin_img(parsed_id),
            Err(_) => {},
        }
    }

    if let Some(img) = img {
        return tauri::ipc::Response::new(img.to_rgba8().to_vec());
    }
    
    warn!("No image found for id: {}", id);
    tauri::ipc::Response::new(vec![])
}

#[tauri::command]
pub async fn new_pin(
    offset_x: String,
    offset_y: String,
    width: String,
    height: String,
    webview_window: tauri::WebviewWindow,
) {
    let mut app = Application::global().lock().unwrap();
    let screenshot = app.get_module("screenshot");

    if let Some(monitor) = webview_window.current_monitor().ok().flatten() {
        let monitor_pos = monitor.position();
        let rect = (
            offset_x.parse::<u32>().unwrap(),
            offset_y.parse::<u32>().unwrap(),
            width.parse::<u32>().unwrap(),
            height.parse::<u32>().unwrap(),
        );

        if let Some(s) = screenshot {
            if let Some(screenshot) = s.as_any_mut().downcast_mut::<ScreenShotter>() {
                screenshot
                    .new_pin(
                        monitor_pos.x,
                        monitor_pos.y,
                        rect,
                        webview_window.label().to_string(),
                    )
                    .unwrap();
            }
        }
    } else {
        log::error!("Unable to get current monitor");
    }
}

#[tauri::command]
pub async fn close_cache_pin() {
    let mut app = Application::global().lock().unwrap();
    let screenshot = app.get_module("screenshot");

    if let Some(s) = screenshot {
        if let Some(screenshot) = s.as_any_mut().downcast_mut::<ScreenShotter>() {
            screenshot.close_cache_pin().unwrap();
        }
    }
}

#[tauri::command]
pub async fn update_pin_state(id: u32, x: i32, y: i32, zoom: u32, minimized: bool) {
    let mut app = Application::global().lock().unwrap();
    let screenshot = app.get_module("screenshot");

    if let Some(s) = screenshot {
        if let Some(screenshot) = s.as_any_mut().downcast_mut::<ScreenShotter>() {
            if let Some(mut record) = screenshot.shotter_recort.get_record(id).cloned() {
                record.minimized = minimized;
                record.pos_x = x - record.rect.0 as i32;
                record.pos_y = y - record.rect.1 as i32;
                record.zoom_factor = zoom;
                screenshot.update_shotter_record(id, record);
            }
        }
    }
}

#[tauri::command]
pub async fn delete_pin_record(id: u32) {
    let mut app = Application::global().lock().unwrap();
    let screenshot = app.get_module("screenshot");

    if let Some(s) = screenshot {
        if let Some(screenshot) = s.as_any_mut().downcast_mut::<ScreenShotter>() {
            if let Err(e) = screenshot.shotter_recort.del_shotter(id) {
                log::error!("Failed to delete pin record {}: {}", id, e);
            }
        }
    }
}

#[tauri::command]
pub async fn save_img(img_buf: Vec<u8>, app: tauri::AppHandle) -> bool {
    let mut app_config = AppConfig::global().lock().unwrap();
    let save_path = app_config.get(&"save_path".to_string()).cloned().unwrap_or_default();
    let if_auto_change = app_config.get(&"if_auto_change_save_path".to_string()).cloned().unwrap_or("true".to_string());
    let if_ask_path = app_config.get(&"if_ask_save_path".to_string()).cloned().unwrap_or("true".to_string());

    let file_name = chrono::Local::now()
        .format("Rotor_%Y-%m-%d-%H-%M-%S.png")
        .to_string();

    let file_path: Option<std::path::PathBuf> = if (if_ask_path == "true") ||  (save_path.is_empty()) {
        app
            .dialog()
            .file()
            .set_directory(save_path)
            .add_filter("PNG", &["png"])
            .set_file_name(file_name)
            .blocking_save_file()
            .map(|v| { v.into_path().unwrap() })
    } else {
        Some(std::path::PathBuf::from(save_path))
    };
    
    if let Some(file_path) = file_path {
        if if_auto_change == "true" {
            app_config.set("save_path".to_string(), file_path.to_string_lossy().to_string()).unwrap();
        }
        let cursor = std::io::Cursor::new(img_buf);
        if let Ok(img) = image::load(cursor, image::ImageFormat::Png) {
            img.save(file_path).unwrap();
            return true;
        }
    }
    drop(app_config);
    false
}
