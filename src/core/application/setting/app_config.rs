use toml;
use std::fs;
use serde::{Serialize, Deserialize};

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
    pub fn get() -> Option<AppConfig> {
        let path = "config.toml";
        let config_str = fs::read_to_string(path).unwrap_or_else(|_| String::new());
        match toml::from_str::<AppConfig>(&config_str) {
            Ok(config) => Some(config),
            Err(e) => None,
        }
    }
}