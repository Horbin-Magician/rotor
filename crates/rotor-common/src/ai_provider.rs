//! Global AI provider settings shared by features, independent of the UI.
use crate::Config;

pub const PROVIDERS: &[&str] = &["deepseek", "openai", "anthropic", "custom"];

pub fn default_base_url(provider: &str) -> &'static str {
    match provider {
        "deepseek" => "https://api.deepseek.com",
        "openai" => "https://api.openai.com/v1",
        "anthropic" => "https://api.anthropic.com/v1",
        _ => "",
    }
}

/// Supply defaults without rewriting profiles or removing unknown/legacy keys.
/// An explicitly saved empty key must never revive a legacy credential.
pub fn apply_defaults(config: &mut Config) {
    config
        .entry("ai_provider".into())
        .or_insert_with(|| "deepseek".into());
    config
        .entry("ai_custom_protocol".into())
        .or_insert_with(|| "openai".into());
    for provider in PROVIDERS {
        for (suffix, default) in [
            ("base_url", default_base_url(provider).to_owned()),
            (
                "api_key",
                if *provider == "deepseek" {
                    config
                        .get("translator_deepseek_api_key")
                        .cloned()
                        .unwrap_or_default()
                } else {
                    String::new()
                },
            ),
            (
                "model",
                if *provider == "deepseek" {
                    config
                        .get("translator_deepseek_model")
                        .cloned()
                        .unwrap_or_else(|| crate::config::DEFAULT_TRANSLATOR_DEEPSEEK_MODEL.into())
                } else {
                    String::new()
                },
            ),
            ("max_tokens", "4096".into()),
        ] {
            config
                .entry(format!("ai_{provider}_{suffix}"))
                .or_insert(default);
        }
    }
}

#[derive(Clone)]
pub struct AiProviderConfig {
    pub provider: String,
    pub protocol: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub max_tokens: String,
}

impl AiProviderConfig {
    pub fn from_config(config: &Config) -> Self {
        let mut config = config.clone();
        apply_defaults(&mut config);
        let provider = config["ai_provider"].clone();
        let value = |suffix| {
            config
                .get(&format!("ai_{provider}_{suffix}"))
                .cloned()
                .unwrap_or_default()
        };
        Self {
            protocol: if provider == "custom" {
                config["ai_custom_protocol"].clone()
            } else if provider == "anthropic" {
                "anthropic".into()
            } else {
                "openai".into()
            },
            base_url: value("base_url"),
            api_key: value("api_key"),
            model: value("model"),
            max_tokens: value("max_tokens"),
            provider,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_defaults_preserve_unknown_fields_and_explicit_empty_credentials() {
        let mut config = Config::from([
            (
                "translator_deepseek_api_key".into(),
                "fixture-secret".into(),
            ),
            ("translator_deepseek_model".into(), "fixture-model".into()),
            ("future_setting".into(), "keep".into()),
        ]);
        apply_defaults(&mut config);
        assert_eq!(config["ai_deepseek_api_key"], "fixture-secret");
        assert_eq!(config["ai_deepseek_model"], "fixture-model");
        config.insert("ai_deepseek_api_key".into(), String::new());
        apply_defaults(&mut config);
        assert_eq!(config["ai_deepseek_api_key"], "");
        assert_eq!(config["future_setting"], "keep");
        assert_eq!(config["translator_deepseek_api_key"], "fixture-secret");
    }
}
