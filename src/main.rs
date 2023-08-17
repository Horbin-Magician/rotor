mod application;
mod module;
mod setting;
mod util;

use slint::ComponentHandle;
use i_slint_backend_selector;

use crate::module::searcher::Searcher;
use crate::setting::Setting;
use crate::application::Application;


fn main() {
    let searcher = Searcher::new();
    let setting = Setting::new();

    let _app = Application::new(setting.setting_win.as_weak(), searcher.search_win.as_weak());

    i_slint_backend_selector::with_platform(|b| {
        b.set_event_loop_quit_on_last_window_closed(false);
        b.run_event_loop()
    }).unwrap();
}