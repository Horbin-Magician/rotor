use tauri_plugin_dialog::DialogExt;
use image::imageops::crop_imm;

use crate::core::application::Application;
use crate::core::config::AppConfig;
use crate::module::screen_shotter::ScreenShotter;
use crate::module::screen_shotter::shotter_record::{ShotterRecord, ShotterConfig};
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
        let masks = masks_arc.lock().await;
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
        let masks = masks_arc.lock().await;
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
pub async fn get_screen_img_rect(
    x: String,
    y: String,
    width: String,
    height: String,
    mask_label: String
) -> tauri::ipc::Response {
    let masks_arc= {
        let mut app = Application::global().lock().unwrap();
        if let Some(ss) = app
            .get_module("screenshot")
            .and_then(|s| s.as_any().downcast_ref::<ScreenShotter>())
        {
            Some(ss.masks.clone())
        } else {
            None
        }
    };

    let masks_arc = match masks_arc {
        Some(a) => a,
        None => return tauri::ipc::Response::new(vec![]),
    };

    let x = match x.parse() {
        Ok(v) => v,
        _ => return tauri::ipc::Response::new(vec![]),
    };
    let y = match y.parse() {
        Ok(v) => v,
        _ => return tauri::ipc::Response::new(vec![]),
    };
    let width = match width.parse() {
        Ok(v) => v,
        _ => return tauri::ipc::Response::new(vec![]),
    };
    let height = match height.parse() {
        Ok(v) => v,
        _ => return tauri::ipc::Response::new(vec![]),
    };

    let image = {
        let masks = masks_arc.lock().await;
        masks.get(&mask_label).cloned()
    };

    if let Some(img) = image {
        let cropped_img = crop_imm(&img, x, y, width, height);
        tauri::ipc::Response::new(cropped_img.to_image().to_vec())
    } else {
        tauri::ipc::Response::new(vec![])
    }
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
        let monitor_pos_lg = monitor_pos.to_logical::<f64>(monitor.scale_factor());
        let x = monitor_pos_lg.x + offset_x.parse::<f64>().unwrap();
        let y = monitor_pos_lg.y + offset_y.parse::<f64>().unwrap();

        if let Some(s) = screenshot {
            if let Some(screenshot) = s.as_any_mut().downcast_mut::<ScreenShotter>() {
                screenshot
                    .new_pin(
                        x,
                        y,
                        width.parse::<f64>().unwrap(),
                        height.parse::<f64>().unwrap(),
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
