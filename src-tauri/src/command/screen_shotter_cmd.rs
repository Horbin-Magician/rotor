use crate::core::application::Application;
use crate::module::screen_shotter::ScreenShotter;

#[tauri::command]
pub fn capture_screen(webview_window: tauri::WebviewWindow) -> tauri::ipc::Response {
    if let Some(monitor) = webview_window.current_monitor().unwrap() {
        let position = monitor.position();
        return tauri::ipc::Response::new(
            ScreenShotter::capture_screen(position.x, position.y)
        );
    }
    tauri::ipc::Response::new(Vec::new())
}

#[tauri::command]
pub fn new_pin(x: String, y: String, width: String, height: String) {
    println!("x:{}, y:{}, width:{}, height:{}", x, y, width, height); // TODO del
    let app = Application::global().lock().unwrap();
    let screenshot = app.get_module("screenshot");
    if let Some(s) = screenshot {
        if let Some(screenshot) = s.as_any().downcast_ref::<ScreenShotter>() {
            screenshot.new_pin();
        }
    }
}
