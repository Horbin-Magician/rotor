use std::error::Error;
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::{collections::HashMap, fs};

use crate::file_path;

pub type Config = HashMap<String, String>;

#[cfg(target_os = "macos")]
pub const DEFAULT_QUICK_ACTIONS: &str = r#"[{"id":"terminal","name":"Terminal","shortcut":"Cmd+Shift+T","command":"open -a Terminal","enabled":true},{"id":"finder","name":"Finder","shortcut":"Cmd+Shift+E","command":"open -a Finder ~","enabled":true}]"#;
#[cfg(target_os = "windows")]
pub const DEFAULT_QUICK_ACTIONS: &str = r#"[{"id":"terminal","name":"Terminal","shortcut":"Ctrl+Shift+T","command":"start \"\" cmd.exe","enabled":true},{"id":"explorer","name":"Explorer","shortcut":"Ctrl+Shift+E","command":"start \"\" explorer.exe","enabled":true}]"#;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const DEFAULT_QUICK_ACTIONS: &str = r#"[{"id":"terminal","name":"Terminal","shortcut":"Ctrl+Shift+T","command":"x-terminal-emulator","enabled":true},{"id":"files","name":"Files","shortcut":"Ctrl+Shift+E","command":"xdg-open ~","enabled":true}]"#;

pub const DEFAULT_QUICK_ACTIONS_REVISION: &str = "2";

#[cfg(target_os = "macos")]
pub const DEFAULT_SEARCH_SHORTCUT: &str = "Cmd+Shift+F";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SEARCH_SHORTCUT: &str = "Ctrl+Shift+F";

#[cfg(target_os = "macos")]
pub const DEFAULT_SCREENSHOT_SHORTCUT: &str = "Cmd+Shift+S";
#[cfg(not(target_os = "macos"))]
pub const DEFAULT_SCREENSHOT_SHORTCUT: &str = "Ctrl+Shift+S";

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
        ("shortcut_screenshot".into(), DEFAULT_SCREENSHOT_SHORTCUT.into()),
        ("shortcut_pinwin_save".into(), "S".into()),
        ("shortcut_pinwin_close".into(), "Escape".into()),
        ("shortcut_pinwin_copy".into(), "Enter".into()),
        ("shortcut_pinwin_hide".into(), "H".into()),
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
}

impl AppConfig {
    fn new() -> AppConfig {
        let mut config = HashMap::new();

        if let Some(user_data_path) = file_path::get_userdata_path() {
            let path = user_data_path.join("config.toml");
            let config_str = fs::read_to_string(path).unwrap_or_else(|e| {
                log::warn!("AppConfig: can not read config file: {e}");
                String::new()
            });

            match toml::from_str::<Config>(&config_str) {
                Ok(c) => config = c,
                Err(e) => log::warn!("AppConfig: config format error: {e}"),
            }
        }

        AppConfig { config }
    }

    fn save(&self) -> Result<(), Box<dyn Error>> {
        if let Some(user_data_path) = file_path::get_userdata_path() {
            fs::create_dir_all(&user_data_path)?;
            let path = user_data_path.join("config.toml");
            let config_str = toml::to_string_pretty(&self.config)?;
            fs::write(path, config_str)?;
            return Ok(());
        }
        Err(Box::new(std::io::Error::other("Failed to save config")))
    }

    pub fn global() -> &'static Mutex<AppConfig> {
        &INSTANCE
    }

    pub fn lock_global() -> MutexGuard<'static, AppConfig> {
        INSTANCE.lock().unwrap_or_else(|poisoned| {
            log::error!("AppConfig lock poisoned; recovering inner state");
            poisoned.into_inner()
        })
    }

    pub fn set(&mut self, k: String, v: String) -> Result<(), Box<dyn Error>> {
        self.config.insert(k, v);
        self.save()?;
        Ok(())
    }

    pub fn set_many(
        &mut self,
        entries: impl IntoIterator<Item = (String, String)>,
    ) -> Result<(), Box<dyn Error>> {
        self.config.extend(entries);
        self.save()?;
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
        let mut merged = HashMap::new();
        for (key, value) in self.config.iter() {
            merged.insert(key.clone(), value.clone());
        }
        for (key, value) in DEFAULT_CONFIG.iter() {
            merged.entry(key.clone()).or_insert_with(|| value.clone());
        }
        merged
    }
}

static INSTANCE: LazyLock<Mutex<AppConfig>> = LazyLock::new(|| Mutex::new(AppConfig::new()));
