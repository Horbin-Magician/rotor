mod application;
mod module;
mod util;

use i_slint_backend_selector;

use crate::application::Application;

fn main() {
    let _app = Application::new();

    i_slint_backend_selector::with_platform(|b| {
        b.set_event_loop_quit_on_last_window_closed(false);
        b.run_event_loop()
    }).unwrap();
}