//! Interface language resolution. Strings themselves live with the views.
use crate::{Language, Settings};

/// Resolve the stored language preference to a BCP 47 tag, following the
/// system locale when the user has not chosen one.
pub fn language_for_config(config: &crate::Config) -> &'static str {
    match config.language() {
        Language::Chinese => "zh-CN",
        Language::English => "en-US",
        Language::System if system_prefers_chinese() => "zh-CN",
        Language::System => "en-US",
    }
}

fn system_prefers_chinese() -> bool {
    sys_locale::get_locale().is_some_and(|locale| locale.to_ascii_lowercase().starts_with("zh"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_language_choices_override_the_system_locale() {
        let mut config = crate::Config::new();
        config.insert("language".into(), "1".into());
        assert_eq!(language_for_config(&config), "zh-CN");
        config.insert("language".into(), "2".into());
        assert_eq!(language_for_config(&config), "en-US");
        config.insert("language".into(), "0".into());
        assert!(matches!(language_for_config(&config), "zh-CN" | "en-US"));
    }
}
