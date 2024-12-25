use slint::Weak;
use toml;
use std::error::Error;
use std::{collections::HashMap, fs};
use std::sync::{LazyLock, Mutex};
use serde::{Serialize, Deserialize};
use global_hotkey::hotkey::HotKey;
use slint::select_bundled_translation;

use crate::util::{file_util, log_util};
use crate::util::sys_util;
use crate::ui::{SearchWindow, SettingWindow, ToolbarWindow};

#[derive(Serialize, Deserialize, Debug)]
pub struct Config {
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
    #[serde(default = "default_zoom_delta")]
    zoom_delta: u8,
}

fn default_false() -> bool { false }
fn default_u8() -> u8 { 0 }
fn default_zoom_delta() -> u8 { 2 }
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

pub struct AppConfig {
    config: Config,
    pub search_win: Option<Weak<SearchWindow>>,
    pub setting_win: Option<Weak<SettingWindow>>,
    pub toolbar_win: Option<Weak<ToolbarWindow>>,
}

impl AppConfig {
    fn new() -> AppConfig {
        let path = file_util::get_userdata_path().join("config.toml");
        let config_str = fs::read_to_string(path)
            .unwrap_or_else(|_| String::new());

        let config = match toml::from_str::<Config>(&config_str) {
            Ok(config) => config,
            Err(e) => panic!("[ERROR] AppConfig read config: {:?}", e),
        };

        AppConfig::select_language(config.language).unwrap_or_else(|e| println!("select_language error: {:?}", e));

        AppConfig {
            config,
            search_win: None,
            setting_win: None,
            toolbar_win: None,
        }
    }

    fn save(&self) -> Result<(), Box<dyn Error>> {
        let path = file_util::get_userdata_path().join("config.toml");
        let config_str = toml::to_string_pretty(&self.config)?;
        fs::write(path, config_str)?;
        Ok(())
    }

    pub fn global() -> &'static Mutex<AppConfig> {
        &INSTANCE
    }

    pub fn set_power_boot(&mut self, power_boot: bool) -> Result<(), Box<dyn Error>> {
        self.config.power_boot = power_boot;
        sys_util::set_power_boot(power_boot)?;
        self.save()?;
        Ok(())
    }

    pub fn get_power_boot(&self) -> bool {
        self.config.power_boot
    }

    pub fn select_language(language: u8) -> Result<(), Box<dyn Error>> {
        if language == 0 {
            let locale_name =  sys_util::get_user_default_locale_name();
            let short_name = if &locale_name[..2] == "zh" {""} else {&locale_name[..2]};
            select_bundled_translation(short_name)?;
        } else if language == 1 {
            select_bundled_translation("")?;
        } else if language == 2 {
            select_bundled_translation("en")?;
        }
        Ok(())
    }

    pub fn set_language(&mut self, language: u8) {
        self.config.language = language;
        AppConfig::select_language(language).unwrap_or_else(|e| println!("select_language error: {:?}", e));
        self.save()
            .unwrap_or_else(|err| log_util::log_error(format!("AppConfig save error: {:?}", err)));
    }

    pub fn get_language(&self) -> u8 {
        self.config.language
    }

    pub fn set_theme(&mut self, theme: u8) {
        if let Some(search_win) = &self.search_win {
            if let Some(search_win) = search_win.upgrade() {
                search_win.invoke_change_theme(theme as i32);
            }
        }

        if let Some(setting_win) = &self.setting_win {
            if let Some(setting_win) = setting_win.upgrade() {
                setting_win.invoke_change_theme(theme as i32);
            }
        }

        if let Some(toolbar_win) = &self.toolbar_win {
            if let Some(toolbar_win) = toolbar_win.upgrade() {
                toolbar_win.invoke_change_theme(theme as i32);
            }
        }

        self.config.theme = theme;
        self.save()
            .unwrap_or_else(|err| log_util::log_error(format!("AppConfig save error: {:?}", err)));
    }

    pub fn get_theme(&self) -> u8 {
        self.config.theme
    }

    // pub fn set_save_path(&mut self, path: String) {
    //     self.save_path = path;
    //     self.save();
    // }

    pub fn get_save_path(&self) -> String {
        self.config.save_path.clone()
    }

    pub fn set_shortcut(&mut self, key: String, value: String) {
        self.config.shortcuts.insert(key, value);
        self.save()
            .unwrap_or_else(|err| log_util::log_error(format!("AppConfig save error: {:?}", err)));
    }

    pub fn get_shortcut(&self, key: &str) -> Option<&String> {
        self.config.shortcuts.get(key)
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

    pub fn set_zoom_delta(&mut self, delta: u8) -> Result<(), Box<dyn Error>> {
        self.config.zoom_delta = delta;
        self.save()?;
        Ok(())
    }

    pub fn get_zoom_delta(&self) -> u8 {
        self.config.zoom_delta
    }
}

static INSTANCE: LazyLock<Mutex<AppConfig>> = LazyLock::new(|| {
    Mutex::new(AppConfig::new())
});