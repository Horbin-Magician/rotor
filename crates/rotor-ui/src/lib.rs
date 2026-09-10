mod assets;
mod capture;
mod pin;
mod search_results;
mod searcher;
mod settings;
mod shortcut;
mod translator;
#[cfg(target_os = "windows")]
mod tray_menu;
mod visual;
pub use assets::UiAssets;
pub use capture::{
    MaskAction, MaskCallback, MaskView, PreparedCapture, PreparedImage, prepare_capture,
    prepare_image,
};
pub use pin::{PinBounds, PinBoundsSetter, PinInit, PinPointerCapture, PinPositionReader, PinView};
pub use searcher::SearchView;
pub use settings::{SettingsView, settings_title};
pub use translator::TranslatorView;
#[cfg(target_os = "windows")]
pub use tray_menu::NativeMenuPainter;
pub use visual::configure_theme;
