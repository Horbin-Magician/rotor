#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod module;

// use i_slint_backend_winit;
// use i_slint_backend_winit::winit::platform::windows::WindowBuilderExtWindows;

use crate::core::util::sys_util;
use crate::core::application::Application;

fn main() {
    // let mut backend = i_slint_backend_winit::Backend::new().unwrap();
    // backend.window_builder_hook = Some(Box::new(|builder| {
    //     builder
    //         .with_skip_taskbar(true)
    // }));
    // slint::platform::set_platform(Box::new(backend)).unwrap();

    if sys_util::run_as_admin() == true {return;}

    let _app = Application::new();

    slint::run_event_loop_until_quit().unwrap();
}