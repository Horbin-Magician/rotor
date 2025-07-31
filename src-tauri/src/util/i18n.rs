use std::collections::HashMap;
use crate::core::config::AppConfig;

pub struct I18n {
    translations: HashMap<String, HashMap<String, String>>,
}

impl I18n {
    pub fn new() -> Self {
        let mut translations = HashMap::new();
        
        // Chinese translations
        let mut zh_cn = HashMap::new();
        zh_cn.insert("appName".to_string(), "小云管家".to_string());
        zh_cn.insert("setting".to_string(), "设置".to_string());
        zh_cn.insert("quit".to_string(), "退出".to_string());
        zh_cn.insert("settingWindowTitle".to_string(), "设置".to_string());
        zh_cn.insert("pinWindowName".to_string(), "小云视窗".to_string());
        translations.insert("zh-CN".to_string(), zh_cn);
        
        // English translations
        let mut en_us = HashMap::new();
        en_us.insert("appName".to_string(), "Rotor".to_string());
        en_us.insert("setting".to_string(), "Settings".to_string());
        en_us.insert("quit".to_string(), "Quit".to_string());
        en_us.insert("settingWindowTitle".to_string(), "Settings".to_string());
        en_us.insert("pinWindowName".to_string(), "Rotor Pin Window".to_string());
        translations.insert("en-US".to_string(), en_us);
        
        I18n { translations }
    }
    
    pub fn t(&self, key: &str) -> String {
        let language = self.get_current_language();
        
        if let Some(lang_map) = self.translations.get(&language) {
            if let Some(translation) = lang_map.get(key) {
                return translation.clone();
            }
        }
        
        // Fallback to English if key not found in current language
        if language != "en-US" {
            if let Some(en_map) = self.translations.get("en-US") {
                if let Some(translation) = en_map.get(key) {
                    return translation.clone();
                }
            }
        }
        
        // Return key if no translation found
        key.to_string()
    }
    
    fn get_current_language(&self) -> String {
        let config = AppConfig::global().lock().unwrap();
        let language_setting = config.get(&"language".to_string()).unwrap_or(&"0".to_string()).clone();
        
        match language_setting.as_str() {
            "1" => "zh-CN".to_string(),
            "2" => "en-US".to_string(),
            _ => {
                // "0" means follow system, detect system language
                self.get_system_language()
            }
        }
    }
    
    fn get_system_language(&self) -> String {
        // Try to detect system language
        #[cfg(target_os = "macos")]
        {
            if let Ok(output) = std::process::Command::new("defaults")
                .args(&["read", "-g", "AppleLanguages"])
                .output()
            {
                let output_str = String::from_utf8_lossy(&output.stdout);
                if output_str.contains("zh-Hans") || output_str.contains("zh-CN") {
                    return "zh-CN".to_string();
                }
            }
        }
        
        #[cfg(target_os = "windows")]
        {
            if let Ok(output) = std::process::Command::new("powershell")
                .args(&["-Command", "Get-Culture | Select-Object -ExpandProperty Name"])
                .output()
            {
                let output_str = String::from_utf8_lossy(&output.stdout);
                if output_str.contains("zh-CN") {
                    return "zh-CN".to_string();
                }
            }
        }
        
        // Default to English
        "en-US".to_string()
    }
}

// Global instance
use std::sync::{LazyLock, Mutex};

static I18N_INSTANCE: LazyLock<Mutex<I18n>> = LazyLock::new(|| {
    Mutex::new(I18n::new())
});

pub fn t(key: &str) -> String {
    I18N_INSTANCE.lock().unwrap().t(key)
}
