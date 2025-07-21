use crate::core::application::Application;
use crate::module::screen_shotter::ScreenShotter;

#[tauri::command]
pub async fn capture_screen(webview_window: tauri::WebviewWindow) -> tauri::ipc::Response {
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
        masks.get(webview_window.label()).cloned()
    } else {
        None
    };

    let millis = chrono::Utc::now().timestamp_millis();
    println!("time return ipc request: {}", millis);

    tauri::ipc::Response::new(image.unwrap_or_default())
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
                    .new_pin(x, y, width.parse::<f64>().unwrap(), height.parse::<f64>().unwrap())
                    .unwrap();
            }
        }
    } else {
        println!("Unable to get current monitor"); // TODO
    }

    webview_window.close().unwrap();
}
