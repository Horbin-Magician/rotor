use gpui_kit::{
    component::{
        Disableable,
        button::Button,
        input::{InputEvent, Textarea, TextareaState},
    },
    prelude::*,
    *,
};
use rotor_common::Config;
use rotor_runtime::{OperationId, RuntimeEvent, Services, TranslateStreamEvent};
use std::sync::Arc;

pub struct TranslatorView {
    services: Arc<Services>,
    config: Config,
    input: Entity<TextareaState>,
    active: Option<OperationId>,
    translated: String,
    message: String,
    warning: String,
    suppress_enter: bool,
    _input_events: Subscription,
    _activation: Subscription,
}
impl TranslatorView {
    pub fn new(services: Arc<Services>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextareaState::new(window, cx).submit_on_enter(true));
        let input_events = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
            if matches!(event, InputEvent::PressEnter { shift: false, .. }) {
                if !this.suppress_enter {
                    this.submit(window, cx);
                }
                this.suppress_enter = false;
            }
        });
        let activation = cx.observe_window_activation(window, |this, window, _| {
            if !window.is_window_active() {
                this.cancel();
                window.remove_window();
            }
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        Self {
            config: services.settings(),
            services,
            input,
            active: None,
            translated: String::new(),
            message: String::new(),
            warning: String::new(),
            suppress_enter: false,
            _input_events: input_events,
            _activation: activation,
        }
    }
    fn t(&self, zh: &'static str, en: &'static str) -> &'static str {
        if rotor_common::i18n::language_for_config(&self.config) == "zh-CN" {
            zh
        } else {
            en
        }
    }
    fn cancel(&mut self) {
        if let Some(id) = self.active.take() {
            self.services.cancel_translation_request(id);
        }
    }
    pub fn translate_text(
        &mut self,
        text: String,
        warning: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input
            .update(cx, |input, cx| input.set_value(text, window, cx));
        self.warning = warning.unwrap_or_default();
        self.submit(window, cx);
    }
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return;
        }
        let value = self.input.read(cx).value().to_string();
        if value.trim().is_empty() {
            return;
        }
        match self.services.translate(value) {
            Ok(id) => {
                self.active = Some(id);
                self.translated.clear();
                self.message = self.t("正在翻译…", "Translating…").into();
            }
            Err(error) => self.message = error,
        }
        cx.notify();
    }
    pub fn handle_event(&mut self, event: &RuntimeEvent, cx: &mut Context<Self>) {
        match event {
            RuntimeEvent::Translation { id, event } if self.active == Some(*id) => {
                if let TranslateStreamEvent::Delta { content } = event {
                    self.translated.push_str(content);
                }
            }
            RuntimeEvent::TranslationFinished { id, result } if self.active == Some(*id) => {
                self.active = None;
                match result {
                    Ok(result) => {
                        self.translated = result.translated.clone();
                        self.message.clear();
                    }
                    Err(error) => self.message = error.clone(),
                }
            }
            RuntimeEvent::SettingsSaved {
                result: Ok(config), ..
            } => self.config = config.clone(),
            _ => return,
        }
        cx.notify();
    }
}
impl Drop for TranslatorView {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl Render for TranslatorView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("translator")
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_3()
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let composing = this.input.update(cx, |input, cx| {
                    input.marked_text_range(window, cx).is_some()
                });
                if event.keystroke.key == "enter" {
                    this.suppress_enter = composing;
                }
                if event.keystroke.key == "escape" && !composing {
                    this.cancel();
                    window.remove_window();
                    cx.stop_propagation();
                }
            }))
            .child(self.t(
                "翻译 · Enter 提交，Shift+Enter 换行",
                "Translate · Enter to submit, Shift+Enter for a new line",
            ))
            .child(Textarea::new(&self.input).h(px(100.)))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("translate")
                            .label(self.t("翻译", "Translate"))
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                    )
                    .child(
                        Button::new("cancel")
                            .label(self.t("停止", "Stop"))
                            .disabled(self.active.is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.cancel();
                                this.message.clear();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("copy")
                            .label(self.t("复制结果", "Copy result"))
                            .disabled(self.translated.is_empty())
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    this.translated.clone(),
                                ))
                            })),
                    ),
            )
            .child(
                div()
                    .id("translation-result")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(self.translated.clone()),
            )
            .child(self.message.clone())
            .child(self.warning.clone())
    }
}
