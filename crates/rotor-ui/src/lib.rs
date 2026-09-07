use gpui_kit::{
    component::{Disableable, button::Button},
    prelude::*,
    *,
};
use rotor_common::Config;
use rotor_runtime::{IndexState, OperationId, RuntimeEvent, SearchIndexStatus, Services};
use std::sync::Arc;

pub struct SettingsView {
    config: Config,
    services: Arc<Services>,
    index_state: IndexState,
    index_status: Option<SearchIndexStatus>,
    pending: Option<OperationId>,
    message: String,
}

impl SettingsView {
    pub fn show_message(&mut self, message: String, cx: &mut Context<Self>) {
        self.message = message;
        cx.notify();
    }
    pub fn new(config: Config, services: Arc<Services>) -> Self {
        Self {
            config,
            services,
            index_state: IndexState::Unavailable,
            index_status: None,
            pending: None,
            message: String::new(),
        }
    }

    pub fn handle_event(&mut self, event: RuntimeEvent, cx: &mut Context<Self>) {
        match event {
            RuntimeEvent::SettingsSaved { id, result } => {
                if self.pending == Some(id) {
                    self.pending = None;
                }
                match result {
                    Ok(config) => {
                        self.config = config;
                        self.message = "设置已保存".into();
                    }
                    Err(error) => self.message = error,
                }
            }
            RuntimeEvent::IndexState(state) => self.index_state = state,
            RuntimeEvent::IndexStatus { result, .. } => match result {
                Ok(status) => {
                    self.index_state = status.state;
                    self.index_status = Some(status);
                }
                Err(error) => self.message = error,
            },
            _ => return,
        }
        cx.notify();
    }

    fn set_theme(&mut self, value: &str, cx: &mut Context<Self>) {
        match self
            .services
            .save_settings(vec![("theme".into(), value.into())])
        {
            Ok(id) => {
                self.pending = Some(id);
                self.message = "正在保存…".into();
            }
            Err(error) => self.message = error,
        }
        cx.notify();
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("system-theme")
                            .label("跟随系统")
                            .disabled(self.pending.is_some())
                            .on_click(cx.listener(|this, _, _, cx| this.set_theme("0", cx))),
                    )
                    .child(
                        Button::new("light-theme")
                            .label("浅色")
                            .disabled(self.pending.is_some())
                            .on_click(cx.listener(|this, _, _, cx| this.set_theme("1", cx))),
                    )
                    .child(
                        Button::new("dark-theme")
                            .label("深色")
                            .disabled(self.pending.is_some())
                            .on_click(cx.listener(|this, _, _, cx| this.set_theme("2", cx))),
                    ),
            )
            .child(format!(
                "索引状态：{}",
                match self.index_state {
                    IndexState::Unavailable => "未启用",
                    IndexState::Unbuild => "待构建",
                    IndexState::Building => "构建中",
                    IndexState::Released => "已释放",
                    IndexState::Loading => "加载中",
                    IndexState::Ready => "就绪",
                    IndexState::Error => "失败",
                }
            ))
            .children(self.index_status.as_ref().map(|status| {
                format!(
                    "{} 项 · {} 个磁盘",
                    status.index_item_count, status.volume_count
                )
            }))
            .child(
                Button::new("refresh-index")
                    .label("刷新索引状态")
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Err(error) = this.services.request_index_status() {
                            this.message = error;
                            cx.notify();
                        }
                    })),
            )
            .child(self.message.clone())
            .child(
                Button::new("close")
                    .label("关闭")
                    .on_click(|_, window, _| window.remove_window()),
            )
    }
}
