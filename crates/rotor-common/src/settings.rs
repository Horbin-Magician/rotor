//! Typed reads over the string-keyed configuration map.
//!
//! The profile stays a `HashMap<String, String>` so unknown keys written by
//! other builds survive round trips. Everything that interprets a value goes
//! through this module instead of comparing raw strings at the point of use.
use crate::Config;

/// Stored key names. Use these instead of literals when reading or writing.
pub mod keys {
    pub const LANGUAGE: &str = "language";
    pub const THEME: &str = "theme";
    pub const ZOOM_DELTA: &str = "zoom_delta";
    pub const SAVE_PATH: &str = "save_path";
    pub const TRANSLATOR_ENGINE: &str = "translator_engine";
    pub const TRANSLATOR_CUSTOM_URL: &str = "translator_custom_url";
    pub const TRANSLATOR_CUSTOM_KEY: &str = "translator_custom_key";
    pub const TRANSLATOR_TARGET_LANG: &str = "translator_target_lang";
    pub const TRANSLATOR_DEEPSEEK_API_KEY: &str = "translator_deepseek_api_key";
    pub const TRANSLATOR_DEEPSEEK_MODEL: &str = "translator_deepseek_model";
    pub const AI_PROVIDER: &str = "ai_provider";
    pub const AI_CUSTOM_PROTOCOL: &str = "ai_custom_protocol";
    pub const QUICK_ACTIONS: &str = "quick_actions";
    pub const SEARCH_EXCLUDED_DIRS: &str = "search_excluded_dirs";
}

/// Stored as "0" (follow the system), "1" (Chinese) or "2" (English).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    System,
    Chinese,
    English,
}

impl Language {
    pub fn from_value(value: Option<&str>) -> Self {
        match value {
            Some("1") => Self::Chinese,
            Some("2") => Self::English,
            _ => Self::System,
        }
    }
}

/// Stored as "0" (follow the system), "1" (light) or "2" (dark).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    System,
    Light,
    Dark,
}

impl Theme {
    pub fn from_value(value: Option<&str>) -> Self {
        match value {
            Some("1") => Self::Light,
            Some("2") => Self::Dark,
            _ => Self::System,
        }
    }

    /// `Some(dark)` when the user pinned a mode; `None` follows the system.
    pub fn forced_dark(self) -> Option<bool> {
        match self {
            Self::System => None,
            Self::Light => Some(false),
            Self::Dark => Some(true),
        }
    }
}

/// Which translation backend a request uses.
///
/// Profiles written before the global AI provider settings store "deepseek";
/// it selects the AI engine like "ai" does. Any other value keeps the
/// historical Google fallback so a hand-edited profile still translates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TranslatorEngine {
    Google,
    Ai,
    Custom,
}

impl TranslatorEngine {
    pub const GOOGLE: &'static str = "google";
    pub const AI: &'static str = "ai";
    pub const CUSTOM: &'static str = "custom";

    pub fn from_value(value: Option<&str>) -> Self {
        match value {
            Some("ai" | "deepseek") => Self::Ai,
            Some("custom") => Self::Custom,
            _ => Self::Google,
        }
    }

    pub fn as_value(self) -> &'static str {
        match self {
            Self::Google => Self::GOOGLE,
            Self::Ai => Self::AI,
            Self::Custom => Self::CUSTOM,
        }
    }
}

/// Wire protocol spoken to an AI provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiProtocol {
    OpenAi,
    Anthropic,
}

impl AiProtocol {
    pub const OPENAI: &'static str = "openai";
    pub const ANTHROPIC: &'static str = "anthropic";

    /// `None` for values this build does not speak; callers report that
    /// instead of guessing a protocol.
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            Self::OPENAI => Some(Self::OpenAi),
            Self::ANTHROPIC => Some(Self::Anthropic),
            _ => None,
        }
    }

    pub fn as_value(self) -> &'static str {
        match self {
            Self::OpenAi => Self::OPENAI,
            Self::Anthropic => Self::ANTHROPIC,
        }
    }
}

/// Typed accessors for a configuration snapshot.
pub trait Settings {
    fn language(&self) -> Language;
    /// The interface language after resolving "follow the system".
    fn locale(&self) -> crate::i18n::Locale;
    fn theme(&self) -> Theme;
    fn translator_engine(&self) -> TranslatorEngine;
    /// The stored text for `key`, or an empty string when absent.
    fn text(&self, key: &str) -> &str;
}

impl Settings for Config {
    fn language(&self) -> Language {
        Language::from_value(self.get(keys::LANGUAGE).map(String::as_str))
    }

    fn locale(&self) -> crate::i18n::Locale {
        crate::i18n::Locale::from_config(self)
    }

    fn theme(&self) -> Theme {
        Theme::from_value(self.get(keys::THEME).map(String::as_str))
    }

    fn translator_engine(&self) -> TranslatorEngine {
        TranslatorEngine::from_value(self.get(keys::TRANSLATOR_ENGINE).map(String::as_str))
    }

    fn text(&self, key: &str) -> &str {
        self.get(key).map_or("", String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_values_map_to_typed_settings_with_documented_fallbacks() {
        let mut config = Config::new();
        assert_eq!(config.language(), Language::System);
        assert_eq!(config.theme(), Theme::System);
        assert_eq!(config.theme().forced_dark(), None);
        assert_eq!(config.translator_engine(), TranslatorEngine::Google);
        assert_eq!(config.text(keys::SAVE_PATH), "");

        config.insert(keys::LANGUAGE.into(), "1".into());
        config.insert(keys::THEME.into(), "2".into());
        config.insert(keys::TRANSLATOR_ENGINE.into(), "deepseek".into());
        assert_eq!(config.language(), Language::Chinese);
        assert_eq!(config.theme().forced_dark(), Some(true));
        assert_eq!(config.translator_engine(), TranslatorEngine::Ai);

        config.insert(keys::TRANSLATOR_ENGINE.into(), "typo".into());
        assert_eq!(config.translator_engine(), TranslatorEngine::Google);
        assert_eq!(TranslatorEngine::Custom.as_value(), "custom");
        assert_eq!(
            AiProtocol::from_value("anthropic"),
            Some(AiProtocol::Anthropic)
        );
        assert_eq!(AiProtocol::from_value("grpc"), None);
    }
}
