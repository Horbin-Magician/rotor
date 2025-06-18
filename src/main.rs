#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use slint::BackendSelector;

mod core;
#[cfg(target_os = "windows")] // TODO: enable for macOS
mod ui;
mod module;
mod util;

use crate::util::log_util;
#[cfg(target_os = "windows")] // TODO: enable for macOS
use crate::util::sys_util;
use crate::util::file_util;
use crate::core::application::Application;

fn main() {
    let renderer_name = "skia-software".to_string();
    let selector = BackendSelector::new().renderer_name(renderer_name.clone());
    selector.select()
        .unwrap_or_else(|e| log_util::log_error(format!("Error selecting backend: {:?}", e)));

    #[cfg(target_os = "windows")]
    // init before event loop
    if sys_util::run_as_admin()
        .unwrap_or_else( |e| {
            log_util::log_error(format!("run_as_admin error: {:?}", e));
            true
        })
    { return; }
    
    // del tmp and .fd files
    file_util::del_useless_files()
        .unwrap_or_else(|e| log_util::log_error(format!("del_useless_files error: {:?}", e)));
    
    // start event loop
    Application::new()
        .map(|mut app| app.run())
        .unwrap_or_else(|_| log_util::log_error("Application::new error".to_string()));
}