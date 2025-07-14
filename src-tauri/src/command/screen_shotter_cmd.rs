use crate::core::application::Application;
use crate::module::screen_shotter::ScreenShotter;

#[tauri::command]
pub fn capture_screen(webview_window: tauri::WebviewWindow) -> tauri::ipc::Response {
    let mut app = Application::global().lock().unwrap();
    let screenshot = app.get_module("screenshot");
    if let Some(s) = screenshot {
        if let Some(screenshot) = s.as_any().downcast_ref::<ScreenShotter>() {
            let image = screenshot.masks.get(webview_window.label());
            if let Some(image) = image {
                return tauri::ipc::Response::new(image.clone());
            }
        }
    }
    tauri::ipc::Response::new(Vec::new())
}

#[tauri::command]
pub fn new_pin(x: String, y: String, width: String, height: String, webview_window: tauri::WebviewWindow) {
    let mut app = Application::global().lock().unwrap();
    let screenshot = app.get_module("screenshot");
    if let Some(s) = screenshot {
        if let Some(screenshot) = s.as_any_mut().downcast_mut::<ScreenShotter>() {
            screenshot.new_pin(
                x.parse::<f64>().unwrap(),
                y.parse::<f64>().unwrap(),
                width.parse::<f64>().unwrap(),
                height.parse::<f64>().unwrap(),
                webview_window
            ).unwrap();
        }
    }
}
