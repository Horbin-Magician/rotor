mod capture;
mod pin;
mod search_results;
mod searcher;
mod settings;
mod shortcut;
mod translator;
mod visual;
pub use capture::{
    MaskAction, MaskCallback, MaskView, PreparedCapture, PreparedImage, prepare_capture,
    prepare_image,
};
pub use pin::{PinBounds, PinBoundsSetter, PinInit, PinPointerCapture, PinPositionReader, PinView};
pub use searcher::SearchView;
pub use settings::{SettingsView, settings_title};
pub use translator::TranslatorView;
