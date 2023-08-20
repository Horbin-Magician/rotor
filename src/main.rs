// #![windows_subsystem = "windows"]

mod core;
mod module;

use i_slint_backend_selector;

use crate::core::application::Application;

fn main() {
    slint::platform::set_platform(Box::new(i_slint_backend_winit::Backend::new())).unwrap();

    let _app = Application::new();

    i_slint_backend_selector::with_platform(|b| {
        b.set_event_loop_quit_on_last_window_closed(false);
        b.run_event_loop()
    }).unwrap();
}