use toml;
use std::{fs, env};
use once_cell::sync::Lazy;
use std::sync::Mutex;
use serde::{Serialize, Deserialize};

use crate::core::util::sys_util;

#[derive(Serialize, Deserialize, Debug)]
pub struct AppConfig {
    #[serde(default = "default_false")]
    power_boot: bool,
    #[serde(default = "default_u8")]
    language: u8,
    #[serde(default = "default_u8")]
    theme: u8,
}

fn default_false() -> bool { false }
fn default_u8() -> u8 { 0 }

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
        sys_util::set_power_boot(power_boot).unwrap();
        self.save();
    }

    pub fn get_power_boot(&self) -> bool {
        self.power_boot
    }
}

static INSTANCE: Lazy<Mutex<AppConfig>> = Lazy::new(|| {
    Mutex::new(AppConfig::new())
});