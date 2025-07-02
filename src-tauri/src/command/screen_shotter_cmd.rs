use xcap::Monitor;

#[tauri::command]
pub fn capture_screen(webview_window: tauri::WebviewWindow) -> tauri::ipc::Response {
    if let Some(monitor) = webview_window.current_monitor().unwrap() {
        let position = monitor.position();
        if let Ok(monitor) = Monitor::from_point(position.x, position.y) {
            let monitor_img = monitor.capture_image().unwrap_or_default();
            return tauri::ipc::Response::new(monitor_img.to_vec());
        }
    }
    tauri::ipc::Response::new(Vec::new())
}
