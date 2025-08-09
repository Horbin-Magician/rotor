use crate::core::application::Application;
use crate::module::searcher::Searcher;

#[tauri::command]
pub async fn searcher_find(query: String) {
    let mut app = Application::global().lock().unwrap();
    app.get_module("searcher")
        .and_then(|s| s.as_any().downcast_ref::<Searcher>())
        .map(|searcher| searcher.find(query));
}

#[tauri::command]
pub async fn searcher_release() {
    let mut app = Application::global().lock().unwrap();
    app.get_module("searcher")
        .and_then(|s| s.as_any().downcast_ref::<Searcher>())
        .map(|searcher| searcher.release());
}
