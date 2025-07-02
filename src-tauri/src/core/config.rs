use std::error::Error;
use std::sync::{LazyLock, Mutex};
use std::{collections::HashMap, fs};
use toml;

use crate::util::{file_util, log_util};

pub type Config = HashMap<String, String>;

static DEFAULT_CONFIG: LazyLock<Config> = LazyLock::new(|| {
    HashMap::from([
        // ("power_boot".into(), "false".into()),
        ("language".into(), "0".into()),
        ("theme".into(), "0".into()),
        ("save_path".into(), "".into()),
        ("if_auto_change_save_path".into(), "true".into()),
        ("if_ask_save_path".into(), "true".into()),
        ("zoom_delta".into(), "2".into()),
        ("current_workspace".into(), "0".into()),
        ("shortcut_search".into(), "Shift+F".into()),
        ("shortcut_screenshot".into(), "Shift+C".into()),
        ("shortcut_pinwin_save".into(), "S".into()),
        ("shortcut_pinwin_close".into(), "Esc".into()),
        ("shortcut_pinwin_copy".into(), "Enter".into()),
        ("shortcut_pinwin_hide".into(), "H".into()),
    ])
});

pub struct AppConfig {
    config: Config,
}

impl AppConfig {
    fn new() -> AppConfig {
        let mut config = HashMap::new();

        if let Some(user_data_path) = file_util::get_userdata_path() {
            let path = user_data_path.join("config.toml");
            let config_str = fs::read_to_string(path).unwrap_or_else(|e| {
                log_util::log_warn(format!("AppConfig: can not read config file: {:?}", e));
                return String::new();
            });

            match toml::from_str::<Config>(&config_str) {
                Ok(c) => config = c,
                Err(e) => log_util::log_warn(format!("AppConfig: config format error: {:?}", e)),
            }
        }

        AppConfig { config }
    }

    fn save(&self) -> Result<(), Box<dyn Error>> {
        if let Some(user_data_path) = file_util::get_userdata_path() {
            let path = user_data_path.join("config.toml");
            let config_str = toml::to_string_pretty(&self.config)?;
            fs::write(path, config_str)?;
            return Ok(());
        }
        Err(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to get user data path",
        )))
    }

    pub fn global() -> &'static Mutex<AppConfig> {
        &INSTANCE
    }

    pub fn set(&mut self, k: String, v: String) -> Result<(), Box<dyn Error>> {
        self.config.insert(k, v);
        self.save()?;
        Ok(())
    }

    pub fn get(&self, k: &String) -> Option<&String> {
        if self.config.contains_key(k) {
            return self.config.get(k);
        }
        return DEFAULT_CONFIG.get(k);
    }

    pub fn get_all(&self) -> Config {
        let mut merged = HashMap::new();
        for (key, value) in self.config.iter() {
            merged.insert(key.clone(), value.clone());
        }
        for (key, value) in DEFAULT_CONFIG.iter() {
            merged.entry(key.clone()).or_insert_with(|| value.clone());
        }
        return merged;
    }
}

static INSTANCE: LazyLock<Mutex<AppConfig>> = LazyLock::new(|| Mutex::new(AppConfig::new()));
