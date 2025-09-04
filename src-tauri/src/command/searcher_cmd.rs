use crate::core::application::Application;
use crate::module::searcher::Searcher;

use crate::util::file_util;

#[tauri::command]
pub async fn searcher_find(query: String) {
    let mut app = Application::global().lock().unwrap();
    if let Some(searcher) = app
        .get_module("searcher")
        .and_then(|s| s.as_any().downcast_ref::<Searcher>())
    {
        searcher.find(query);
    }
}

#[tauri::command]
pub async fn searcher_release() {
    let mut app = Application::global().lock().unwrap();
    if let Some(searcher) = app
        .get_module("searcher")
        .and_then(|s| s.as_any().downcast_ref::<Searcher>())
    {
        searcher.release();
    }
}

#[tauri::command]
pub fn open_file(file_path: String) -> Result<(), String> {
    file_util::open_file(file_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn open_file_as_admin(file_path: String) -> Result<(), String> {
    file_util::open_file_as_admin(file_path).map_err(|e| e.to_string())
}
