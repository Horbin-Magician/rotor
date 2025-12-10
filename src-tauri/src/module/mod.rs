pub mod ai;
pub mod screen_shotter;
pub mod searcher;
pub mod tray;

use std::any::Any;
use std::{error::Error, vec};
use tauri_plugin_global_shortcut::Shortcut;

pub trait Module {
    fn flag(&self) -> &str;
    fn init(&mut self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>>;
    fn run(&mut self) -> Result<(), Box<dyn Error>>;
    fn get_shortcut(&self) -> Option<Shortcut>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub fn get_all_modules() -> Vec<Box<dyn Module + Send>> {
    vec![
        Box::new(tray::Tray::new().unwrap()),
        Box::new(screen_shotter::ScreenShotter::new().unwrap()),
        Box::new(searcher::Searcher::new().unwrap()),
    ]
}
