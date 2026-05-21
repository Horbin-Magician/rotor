use rotor_runtime::Application;

#[tauri::command]
pub async fn searcher_find(query: String) {
    Application::global().lock().unwrap().searcher.find(query);
}

#[tauri::command]
pub async fn searcher_release() {
    Application::global().lock().unwrap().searcher.release();
}

#[tauri::command]
pub fn open_file(file_path: String) -> Result<(), String> {
    rotor_platform::file_util::open_file(file_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_file_as_admin(file_path: String) -> Result<(), String> {
    rotor_platform::file_util::open_file_as_admin(file_path).map_err(|e| e.to_string())
}
