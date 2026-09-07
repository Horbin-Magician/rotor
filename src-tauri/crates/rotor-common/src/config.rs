use std::error::Error;
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use crate::file_path;

pub type Config = HashMap<String, String>;

#[cfg(target_os = "macos")]
pub const DEFAULT_QUICK_ACTIONS: &str = r#"[{"id":"terminal","name":"Terminal","shortcut":"Cmd+Shift+T","command":"open -a Terminal","enabled":true},{"id":"finder","name":"Finder","shortcut":"Cmd+Shift+E","command":"open -a Finder ~","enabled":true}]"#;
#[cfg(target_os = "windows")]
pub const DEFAULT_QUICK_ACTIONS: &str = r#"[{"id":"terminal","name":"Terminal","shortcut":"Ctrl+Shift+T","command":"start wt","enabled":true}]"#;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const DEFAULT_QUICK_ACTIONS: &str = r#"[{"id":"terminal","name":"Terminal","shortcut":"Ctrl+Shift+T","command":"x-terminal-emulator","enabled":true},{"id":"files","name":"Files","shortcut":"Ctrl+Shift+E","command":"xdg-open ~","enabled":true}]"#;

pub const DEFAULT_QUICK_ACTIONS_REVISION: &str = "2";
pub const DEFAULT_TRANSLATOR_DEEPSEEK_MODEL: &str = "deepseek-v4-flash";

#[cfg(target_os = "macos")]
pub const DEFAULT_SEARCH_SHORTCUT: &str = "Cmd+Shift+F";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SEARCH_SHORTCUT: &str = "Ctrl+Shift+F";

#[cfg(target_os = "macos")]
pub const DEFAULT_SCREENSHOT_SHORTCUT: &str = "Cmd+Shift+S";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SCREENSHOT_SHORTCUT: &str = "Ctrl+Shift+S";

#[cfg(target_os = "macos")]
pub const DEFAULT_TRANSLATE_SELECT_SHORTCUT: &str = "Cmd+Shift+D";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_TRANSLATE_SELECT_SHORTCUT: &str = "Ctrl+Shift+D";

#[cfg(target_os = "macos")]
pub const DEFAULT_TRANSLATE_INPUT_SHORTCUT: &str = "Cmd+Shift+W";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_TRANSLATE_INPUT_SHORTCUT: &str = "Ctrl+Shift+W";

#[cfg(target_os = "macos")]
pub const DEFAULT_SEARCH_EXCLUDED_DIRS: &str =
    "~/Library\nnode_modules\ntarget\ndist\nbuild\n.git\n.next\n.cache\ncoverage";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SEARCH_EXCLUDED_DIRS: &str =
    "node_modules\ntarget\ndist\nbuild\n.git\n.next\n.cache\ncoverage";

static DEFAULT_CONFIG: LazyLock<Config> = LazyLock::new(|| {
    HashMap::from([
        ("language".into(), "0".into()),
        ("theme".into(), "0".into()),
        ("save_path".into(), "".into()),
        ("if_auto_change_save_path".into(), "true".into()),
        ("if_ask_save_path".into(), "true".into()),
        ("zoom_delta".into(), "2".into()),
        ("current_workspace".into(), "0".into()),
        ("shortcut_search".into(), DEFAULT_SEARCH_SHORTCUT.into()),
        (
            "shortcut_screenshot".into(),
            DEFAULT_SCREENSHOT_SHORTCUT.into(),
        ),
        ("shortcut_pinwin_save".into(), "S".into()),
        ("shortcut_pinwin_close".into(), "Escape".into()),
        ("shortcut_pinwin_copy".into(), "Enter".into()),
        ("shortcut_pinwin_hide".into(), "H".into()),
        (
            "shortcut_translate_select".into(),
            DEFAULT_TRANSLATE_SELECT_SHORTCUT.into(),
        ),
        (
            "shortcut_translate_input".into(),
            DEFAULT_TRANSLATE_INPUT_SHORTCUT.into(),
        ),
        ("translator_engine".into(), "google".into()),
        ("translator_deepseek_api_key".into(), "".into()),
        (
            "translator_deepseek_model".into(),
            DEFAULT_TRANSLATOR_DEEPSEEK_MODEL.into(),
        ),
        ("translator_custom_url".into(), "".into()),
        ("translator_custom_key".into(), "".into()),
        ("translator_target_lang".into(), "auto".into()),
        ("quick_actions".into(), DEFAULT_QUICK_ACTIONS.into()),
        (
            "quick_actions_revision".into(),
            DEFAULT_QUICK_ACTIONS_REVISION.into(),
        ),
        (
            "search_excluded_dirs".into(),
            DEFAULT_SEARCH_EXCLUDED_DIRS.into(),
        ),
    ])
});

pub struct AppConfig {
    config: Config,
    path: Option<PathBuf>,
    load_error: Option<String>,
}

impl AppConfig {
    fn new() -> AppConfig {
        let directory = file_path::get_userdata_path();
        match directory.as_deref().map(Self::load_from) {
            Some(Ok(config)) => config,
            result => {
                let message = match result {
                    Some(Err(error)) => error.to_string(),
                    _ => "data directory unavailable".to_string(),
                };
                log::error!(
                    "Configuration is read-only until its load error is resolved: {message}"
                );
                Self {
                    config: HashMap::new(),
                    path: directory.map(|path| path.join("config.toml")),
                    load_error: Some(message),
                }
            }
        }
    }

    /// Explicit-path service used by the native shell and isolated tests.
    /// Corrupt or unreadable files are never silently replaced with defaults.
    pub fn load_from(directory: &Path) -> Result<Self, Box<dyn Error>> {
        let path = directory.join("config.toml");
        let config = match fs::read_to_string(&path) {
            Ok(contents) => toml::from_str::<Config>(&contents)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(error) => return Err(error.into()),
        };
        Ok(Self {
            config,
            path: Some(path),
            load_error: None,
        })
    }

    fn save_candidate(&self, config: &Config) -> Result<(), Box<dyn Error>> {
        if let Some(error) = &self.load_error {
            return Err(
                std::io::Error::other(format!("configuration load failed: {error}")).into(),
            );
        }
        let path = self
            .path
            .as_deref()
            .ok_or_else(|| std::io::Error::other("configuration path unavailable"))?;
        crate::persistence::atomic_write_private(path, toml::to_string_pretty(config)?.as_bytes())?;
        Ok(())
    }

    pub fn global() -> &'static Mutex<AppConfig> {
        &INSTANCE
    }

    pub fn shared_global() -> Arc<Mutex<AppConfig>> {
        Arc::clone(&INSTANCE)
    }

    pub fn lock_global() -> MutexGuard<'static, AppConfig> {
        INSTANCE.lock().unwrap_or_else(|poisoned| {
            log::error!("AppConfig lock poisoned; recovering inner state");
            poisoned.into_inner()
        })
    }

    pub fn set(&mut self, k: String, v: String) -> Result<(), Box<dyn Error>> {
        self.set_many([(k, v)])
    }

    pub fn set_many(
        &mut self,
        entries: impl IntoIterator<Item = (String, String)>,
    ) -> Result<(), Box<dyn Error>> {
        let mut candidate = self.config.clone();
        candidate.extend(entries);
        self.save_candidate(&candidate)?;
        self.config = candidate;
        Ok(())
    }

    pub fn get(&self, k: &str) -> Option<&String> {
        if self.config.contains_key(k) {
            return self.config.get(k);
        }
        DEFAULT_CONFIG.get(k)
    }

    pub fn get_user(&self, k: &str) -> Option<&String> {
        self.config.get(k)
    }

    pub fn get_all(&self) -> Config {
        let mut merged = DEFAULT_CONFIG.clone();
        merged.extend(self.config.clone());
        merged
    }
}

static INSTANCE: LazyLock<Arc<Mutex<AppConfig>>> =
    LazyLock::new(|| Arc::new(Mutex::new(AppConfig::new())));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn successful_transaction_preserves_unknown_keys_and_reloads() {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("config.toml"),
            "future_key = \"preserve\"\nlanguage = \"0\"\n",
        )
        .unwrap();
        let mut config = AppConfig::load_from(directory.path()).unwrap();
        config
            .set_many([
                ("language".into(), "1".into()),
                ("theme".into(), "2".into()),
            ])
            .unwrap();
        let restored = AppConfig::load_from(directory.path()).unwrap();
        assert_eq!(
            restored.get_user("future_key").map(String::as_str),
            Some("preserve")
        );
        assert_eq!(restored.get_user("language").map(String::as_str), Some("1"));
        assert_eq!(restored.get_user("theme").map(String::as_str), Some("2"));
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn failed_replace_keeps_memory_and_cleans_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let mut config = AppConfig::load_from(directory.path()).unwrap();
        // A directory at the final file name reliably fails on both platforms.
        fs::create_dir(directory.path().join("config.toml")).unwrap();
        assert!(config
            .set_many([
                ("language".into(), "1".into()),
                ("theme".into(), "2".into())
            ])
            .is_err());
        assert_eq!(config.get_user("language"), None);
        assert_eq!(config.get_user("theme"), None);
        assert!(directory.path().join("config.toml").is_dir());
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn corrupt_configuration_is_not_overwritten_on_load() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("config.toml");
        fs::write(&path, "broken = [").unwrap();
        assert!(AppConfig::load_from(directory.path()).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "broken = [");
    }

    #[test]
    fn get_all_user_values_override_defaults() {
        let config = AppConfig {
            config: HashMap::from([("language".to_string(), "1".to_string())]),
            path: None,
            load_error: None,
        };

        assert_eq!(
            config.get_all().get("language").map(String::as_str),
            Some("1")
        );
    }

    #[test]
    fn get_all_fills_missing_values_from_defaults() {
        let config = AppConfig {
            config: HashMap::new(),
            path: None,
            load_error: None,
        };

        assert_eq!(
            config.get_all().get("zoom_delta").map(String::as_str),
            Some("2")
        );
        assert_eq!(
            config
                .get_all()
                .get("translator_deepseek_model")
                .map(String::as_str),
            Some("deepseek-v4-flash")
        );
    }
}
