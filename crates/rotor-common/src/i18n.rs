pub fn language_for_config(config: &crate::Config) -> &'static str {
    match config.get("language").map(String::as_str) {
        Some("1") => "zh-CN",
        Some("2") => "en-US",
        _ if sys_locale::get_locale()
            .is_some_and(|locale| locale.to_ascii_lowercase().starts_with("zh")) =>
        {
            "zh-CN"
        }
        _ => "en-US",
    }
}
