#![windows_subsystem = "windows"]

mod core;
mod module;

use i_slint_backend_selector;

use crate::core::util::sys_util;
use crate::core::application::Application;

fn main() {
    if sys_util::run_as_admin() == true {return;}

    let _app = Application::new();

    i_slint_backend_selector::with_platform(|b| {
        b.set_event_loop_quit_on_last_window_closed(false);
        b.run_event_loop()
    }).unwrap();
}