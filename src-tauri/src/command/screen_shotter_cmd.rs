use crate::core::application::Application;
use crate::module::screen_shotter::ScreenShotter;

#[tauri::command]
pub async fn get_screen_img(label: String) -> tauri::ipc::Response {
    let millis = chrono::Utc::now().timestamp_millis();
    println!("time get ipc request: {}", millis);

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

    tauri::ipc::Response::new(image.unwrap_or_default())
}

#[tauri::command]
pub async fn get_screen_img_rect(
    x: String,
    y: String,
    width: String,
    height: String,
    webview_window: tauri::WebviewWindow
) -> tauri::ipc::Response {
    let (masks_arc, pin_mask_label) = {
        let mut app = Application::global().lock().unwrap();
        if let Some(ss) = app.get_module("screenshot")
            .and_then(|s| s.as_any().downcast_ref::<ScreenShotter>())
        {
            (Some(ss.masks.clone()), Some(ss.pin_mask_label.clone()))
        } else {
            (None, None)
        }
    };

    let masks_arc = match masks_arc {
        Some(a) => a,
        None => return tauri::ipc::Response::new(vec![]),
    };
    let pin_mask_label = match pin_mask_label {
        Some(l) => l,
        None => return tauri::ipc::Response::new(vec![]),
    };

    let x: usize = match x.parse() { Ok(v) => v, _ => return tauri::ipc::Response::new(vec![]) };
    let y: usize = match y.parse() { Ok(v) => v, _ => return tauri::ipc::Response::new(vec![]) };
    let width: usize = match width.parse() { Ok(v) => v, _ => return tauri::ipc::Response::new(vec![]) };
    let height: usize = match height.parse() { Ok(v) => v, _ => return tauri::ipc::Response::new(vec![]) };
    let img_width = webview_window.current_monitor().unwrap().unwrap().size().width as usize;

    if x + width > img_width || height == 0 {
        return tauri::ipc::Response::new(vec![]);
    }

    let image = {
        let masks = masks_arc.lock().await;
        masks.get(&pin_mask_label).cloned()
    };

    if let Some(img) = image {
        let mut cropped_image = Vec::with_capacity(width * height * 4);
        for row in img.chunks_exact(img_width * 4).skip(y).take(height) {
            let start = x * 4;
            let end = (x + width) * 4;
            if row.len() >= end {
                cropped_image.extend_from_slice(&row[start..end]);
            }
        }
        return tauri::ipc::Response::new(cropped_image);
    } else {
        return tauri::ipc::Response::new(vec![]);
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
    println!("new_pin called with offset_x: {}, offset_y: {}, width: {}, height: {}", offset_x, offset_y, width, height);

    let mut app = Application::global().lock().unwrap();
    let screenshot = app.get_module("screenshot");

    if let Some(monitor) = webview_window.current_monitor().ok().flatten() {
        let monitor_pos = monitor.position();
        let x = monitor_pos.x as f64 + offset_x.parse::<f64>().unwrap();
        let y = monitor_pos.y as f64 + offset_y.parse::<f64>().unwrap();

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
        println!("Unable to get current monitor"); // TODO
    }

    webview_window.close().unwrap();
}
