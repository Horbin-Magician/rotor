pub mod application;
pub mod quick;
pub mod screenshot_data;
pub mod tray;

pub use application::{handle_global_hotkey_event, Application, ShortcutRegistrationNotice};
pub use quick::{Quick, QuickAction};
pub use screenshot_data::fetch_screenshot_data;
