use toml;
use std::{collections::HashMap, env, fs};
use once_cell::sync::Lazy;
use std::sync::Mutex;
use serde::{Serialize, Deserialize};
use global_hotkey::hotkey::HotKey;

use crate::core::util::sys_util;

#[derive(Serialize, Deserialize, Debug)]
pub struct AppConfig {
    #[serde(default = "default_false")]
    power_boot: bool,
    #[serde(default = "default_u8")]
    language: u8,
    #[serde(default = "default_u8")]
    theme: u8,
    #[serde(default = "default_string")]
    save_path: String,
    #[serde(default = "default_shortcuts")]
    shortcuts: HashMap<String, String>,
}

fn default_false() -> bool { false }
fn default_u8() -> u8 { 0 }
fn default_string() -> String { String::new() }
fn default_shortcuts() -> HashMap<String, String> { 
    let mut shortcuts = HashMap::new();
    shortcuts.insert("search".into(), "Shift+F".into());
    shortcuts.insert("screenshot".into(), "Shift+C".into());
    shortcuts.insert("pinwin_save".into(), "S".into());
    shortcuts.insert("pinwin_close".into(), "Esc".into());
    shortcuts.insert("pinwin_copy".into(), "Enter".into());
    shortcuts.insert("pinwin_hide".into(), "H".into());
    shortcuts
}

impl AppConfig {
    fn new() -> AppConfig {
        let path = env::current_exe().unwrap().parent().unwrap().join("userdata").join("config.toml");
        let config_str = fs::read_to_string(path).unwrap_or_else(|_| String::new());
        match toml::from_str::<AppConfig>(&config_str) {
            Ok(config) => config,
            Err(e) => panic!("[ERROR] AppConfig read config: {:?}", e),
        }
    }

    fn save(&self) {
        let path = env::current_exe().unwrap().parent().unwrap().join("userdata").join("config.toml");
        let config_str = toml::to_string_pretty(self).unwrap();
        fs::write(path, config_str).unwrap();
    }

    pub fn global() -> &'static Mutex<AppConfig> {
        &INSTANCE
    }

    pub fn set_power_boot(&mut self, power_boot: bool) {
        self.power_boot = power_boot;
        let _ = sys_util::set_power_boot(power_boot);
        self.save();
    }

    pub fn get_power_boot(&self) -> bool {
        self.power_boot
    }

    pub fn set_theme(&mut self, theme: u8) {
        self.theme = theme;
        self.save();
    }

    pub fn get_theme(&self) -> u8 {
        self.theme
    }

    // pub fn set_save_path(&mut self, path: String) {
    //     self.save_path = path;
    //     self.save();
    // }

    pub fn get_save_path(&self) -> String {
        self.save_path.clone()
    }

    pub fn set_shortcut(&mut self, key: String, value: String) {
        self.shortcuts.insert(key, value);
        self.save();
    }

    pub fn get_shortcut(&self, key: &str) -> Option<&String> {
        self.shortcuts.get(key)
    }

    pub fn get_hotkey_from_str(&self, key: &str) -> Option<HotKey> {
        let shortcut = self.get_shortcut(key);
        if let Some(shortcut) = shortcut {
            if let Ok(hotkey) = shortcut.parse::<HotKey>() {
                return Some(hotkey);
            }
        }
        None
    }
}

static INSTANCE: Lazy<Mutex<AppConfig>> = Lazy::new(|| {
    Mutex::new(AppConfig::new())
});