// pub mod setting;
// pub mod searcher;
pub mod screen_shotter;

use tauri_plugin_global_shortcut::Shortcut;

pub trait Module {
    fn flag(&self) -> &str;
    fn run(&self, app: &tauri::AppHandle);
    fn get_shortcut(&self) -> Option<Shortcut>;
}
