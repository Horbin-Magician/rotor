// pub mod setting;
// pub mod searcher;
pub mod screen_shotter;
pub mod tray;

use std::{error::Error, vec};
use tauri_plugin_global_shortcut::Shortcut;

pub trait Module {
    fn flag(&self) -> &str;
    fn init(&mut self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>>;
    fn run(&self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>>;
    fn get_shortcut(&self) -> Option<Shortcut>;
}

pub fn get_all_modules() -> Vec<Box<dyn Module + Send>> {
    vec![
        Box::new(screen_shotter::ScreenShotter::new().unwrap()),
        Box::new(tray::Tray::new().unwrap()),
    ]
}
