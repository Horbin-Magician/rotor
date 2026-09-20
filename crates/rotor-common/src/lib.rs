pub mod ai_provider;
pub mod config;
pub mod file_path;
pub mod i18n;
pub mod persistence;
pub mod resources;
pub mod settings;
pub mod startup_flags;

pub use config::AppConfig as ConfigService;
pub use config::{AppConfig, Config, DEFAULT_QUICK_ACTIONS};
pub use i18n::Locale;
pub use resources::ResourceLocator;
pub use settings::{AiProtocol, Language, Settings, Theme, TranslatorEngine};
pub mod native_app;
