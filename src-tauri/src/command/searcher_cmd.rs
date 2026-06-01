use rotor_runtime::Application;
use rotor_searcher::file_data::SearchIndexStatus;

#[tauri::command]
pub async fn searcher_find(query: String) {
    Application::lock_global().searcher.find(query);
}

#[tauri::command]
pub async fn searcher_release() {
    Application::lock_global().searcher.release();
}

#[tauri::command]
pub async fn searcher_index_status() -> Result<SearchIndexStatus, String> {
    let search_index_reader = {
        let app_state = Application::lock_global();
        app_state.searcher.index_status_reader()
    };

    tauri::async_runtime::spawn_blocking(move || search_index_reader.index_status())
        .await
        .map_err(|error| format!("Failed to collect search index status: {error}"))
}

#[tauri::command]
pub fn open_file(file_path: String) -> Result<(), String> {
    rotor_platform::file_util::open_file(file_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_file_as_admin(file_path: String) -> Result<(), String> {
    rotor_platform::file_util::open_file_as_admin(file_path).map_err(|e| e.to_string())
}
