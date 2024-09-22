#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod ui;
mod module;
mod util;

use crate::util::log_util;
use crate::util::sys_util;
use crate::util::file_util;
use crate::core::application::Application;

fn main() {
    // init before event loop
    if sys_util::run_as_admin()
        .unwrap_or_else( |e| {
            log_util::log_error(format!("run_as_admin error: {:?}", e));
            true
        }
    ) {return;}

    file_util::del_useless_files()
        .unwrap_or_else(|e| log_util::log_error(format!("del_useless_files error: {:?}", e))); // del tmp and .fd files
    
    // start event loop
    if let Ok(mut app) = Application::new() {
        app.run();
        while app.is_running() {
            slint::run_event_loop()
                .unwrap_or_else(|e| log_util::log_error(format!("slint run_event_loop error: {:?}", e)));
            app.clean();
        }
    } else {
        log_util::log_error("Application::new error".to_string());
    }
}