mod capture;
mod pin;
mod search_results;
mod searcher;
mod settings;
mod translator;
pub use capture::{
    MaskAction, MaskCallback, MaskView, PreparedCapture, PreparedImage, prepare_capture,
    prepare_image,
};
pub use pin::{PinInit, PinPositionReader, PinView};
pub use searcher::SearchView;
pub use settings::{SettingsView, settings_title};
pub use translator::TranslatorView;
