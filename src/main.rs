#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod module;

use crate::core::util::sys_util;
use crate::core::application::Application;

fn main() {
    if sys_util::run_as_admin() == true {return;}

    let _app = Application::new();

    slint::run_event_loop_until_quit().unwrap();
}