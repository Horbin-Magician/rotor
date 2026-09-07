use gpui_kit::{component::button::Button, prelude::*, *};
use rotor_common::Config;

pub struct SettingsView {
    config: Config,
}

impl SettingsView {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let language = match self.config.get("language").map(String::as_str) {
            Some("1") => "简体中文",
            Some("2") => "English",
            _ => "跟随系统",
        };
        let theme = match self.config.get("theme").map(String::as_str) {
            Some("1") => "浅色",
            Some("2") => "深色",
            _ => "跟随系统",
        };
        div()
            .flex()
            .flex_col()
            .p_6()
            .gap_4()
            .size_full()
            .child(div().text_2xl().child("Rotor"))
            .child("常规设置")
            .child(format!("语言：{language}"))
            .child(format!("主题：{theme}"))
            .child(
                Button::new("close")
                    .label("关闭")
                    .on_click(|_, window, _| window.remove_window()),
            )
    }
}
