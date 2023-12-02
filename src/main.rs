#![windows_subsystem = "windows"]

mod core;
mod module;

use i_slint_backend_selector;

use crate::core::application::admin_runner::AdminRunner;
use crate::core::application::powerboot::PowerBoot;
use crate::core::application::Application;

fn main() {
    if AdminRunner::run_as_admin() == true {return;}
    PowerBoot::set_process_auto_run().unwrap(); // TODO: check the setting

    let _app = Application::new();

    i_slint_backend_selector::with_platform(|b| {
        b.set_event_loop_quit_on_last_window_closed(false);
        b.run_event_loop()
    }).unwrap();
}