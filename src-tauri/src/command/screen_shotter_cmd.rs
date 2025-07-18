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
pub fn new_pin(
    x: String,
    y: String,
    width: String,
    height: String,
    webview_window: tauri::WebviewWindow,
) {
    let mut app = Application::global().lock().unwrap();
    let screenshot = app.get_module("screenshot");
    if let Some(s) = screenshot {
        if let Some(screenshot) = s.as_any_mut().downcast_mut::<ScreenShotter>() {
            screenshot
                .new_pin(
                    x.parse::<f64>().unwrap(),
                    y.parse::<f64>().unwrap(),
                    width.parse::<f64>().unwrap(),
                    height.parse::<f64>().unwrap(),
                    webview_window,
                )
                .unwrap();
        }
    }
}
