//! Interface language resolution. Strings themselves live with the views,
//! which pick between a Chinese and an English literal through [`Locale`].
use crate::{Config, Language, Settings};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locale {
    Chinese,
    English,
}

impl Locale {
    /// The stored preference, following the system locale when unset.
    pub fn from_config(config: &Config) -> Self {
        match config.language() {
            Language::Chinese => Self::Chinese,
            Language::English => Self::English,
            Language::System if system_prefers_chinese() => Self::Chinese,
            Language::System => Self::English,
        }
    }

    pub fn is_chinese(self) -> bool {
        self == Self::Chinese
    }

    /// Choose the literal for this locale.
    pub fn pick<'a>(self, zh: &'a str, en: &'a str) -> &'a str {
        match self {
            Self::Chinese => zh,
            Self::English => en,
        }
    }

    /// BCP 47 tag for callers that talk to the system or a service.
    pub fn tag(self) -> &'static str {
        self.pick("zh-CN", "en-US")
    }
}

/// The OS locale does not change while the process runs; query it once
/// instead of on every rendered label.
fn system_prefers_chinese() -> bool {
    static CHINESE: OnceLock<bool> = OnceLock::new();
    *CHINESE.get_or_init(|| {
        sys_locale::get_locale().is_some_and(|locale| locale.to_ascii_lowercase().starts_with("zh"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_language_choices_override_the_system_locale() {
        let mut config = Config::new();
        config.insert("language".into(), "1".into());
        assert_eq!(Locale::from_config(&config), Locale::Chinese);
        assert_eq!(config.locale().pick("中", "en"), "中");
        config.insert("language".into(), "2".into());
        assert_eq!(config.locale(), Locale::English);
        assert_eq!(config.locale().tag(), "en-US");
        config.insert("language".into(), "0".into());
        assert!(matches!(config.locale(), Locale::Chinese | Locale::English));
    }
}
