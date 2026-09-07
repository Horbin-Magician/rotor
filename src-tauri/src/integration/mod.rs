pub mod application;
pub mod screenshot;
mod screenshot_data;
mod screenshot_platform;
mod searcher;
mod translator;
mod tray;

pub use application::{handle_global_hotkey_event, Application, ShortcutRegistrationNotice};
pub use screenshot_data::{fetch_screenshot_data, resolve_screenshot_image, ScreenshotImage};
