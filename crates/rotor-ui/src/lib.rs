mod assets;
mod capture;
mod pin;
mod search_results;
mod searcher;
mod settings;
mod shortcut;
mod translator;
mod visual;
pub use assets::UiAssets;
pub use capture::{
    MaskAction, MaskCallback, MaskView, PreparedCapture, PreparedImage, prepare_capture,
    prepare_image, prepare_image_cancellable,
};
pub use pin::{
    PinActivation, PinBounds, PinBoundsSetter, PinCursorReader, PinInit, PinMinimizedReader,
    PinPointerCapture, PinPositionReader, PinView,
};
pub use searcher::SearchView;
pub use settings::{SettingsView, settings_title};
pub use translator::TranslatorView;
pub use visual::{Palette, configure_theme, surface_palette};
