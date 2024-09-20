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
    if sys_util::run_as_admin() {return;}
    file_util::del_useless_files(); // del tmp and .fd files
    // slint::init_translations!(concat!("./assets/", "/lang/"));

    // start event loop
    let mut app = Application::new();
    app.run();
    while app.is_running() {
        slint::run_event_loop().unwrap_or_else(
            |err| log_util::log_error(format!("slint run_event_loop error: {:?}", err))
        );
        app.clean()
    }
}