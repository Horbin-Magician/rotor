pub mod config;
pub mod file_path;
pub mod i18n;
pub mod persistence;
pub mod resources;

pub use config::AppConfig as ConfigService;
pub use config::{AppConfig, Config, DEFAULT_QUICK_ACTIONS, DEFAULT_QUICK_ACTIONS_REVISION};
pub use resources::ResourceLocator;
