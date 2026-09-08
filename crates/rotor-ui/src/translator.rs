use gpui_kit::{
    component::{
        ActiveTheme, Disableable, IconName,
        button::{Button, ButtonVariants},
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
    input_height: Pixels,
    active: Option<OperationId>,
    translated: String,
    message: String,
    warning: String,
    languages: Option<(String, String)>,
    copied: bool,
    suppress_enter: bool,
    _input_events: Subscription,
    _activation: Subscription,
}
impl TranslatorView {
    pub fn new(services: Arc<Services>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| TextareaState::new(window, cx).submit_on_enter(true));
        let input_events = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
            if matches!(event, InputEvent::Change) {
                this.resize(window, cx);
                cx.notify();
            }
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
        let mut view = Self {
            config: services.settings(),
            services,
            input,
            input_height: px(44.),
            active: None,
            translated: String::new(),
            message: String::new(),
            warning: String::new(),
            languages: None,
            copied: false,
            suppress_enter: false,
            _input_events: input_events,
            _activation: activation,
        };
        window.set_window_title(view.t("Rotor · 翻译", "Rotor · Translation"));
        view.resize(window, cx);
        view
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
    fn resize(&mut self, window: &mut Window, cx: &App) {
        let width = (window.viewport_size().width - px(56.)).max(px(80.));
        let measure = |text: SharedString, max_lines: usize| {
            if text.is_empty() {
                return px(0.);
            }
            window
                .text_system()
                .shape_text(
                    text.clone(),
                    px(14.),
                    &[TextRun {
                        len: text.len(),
                        font: font(cx.theme().font_family.clone()),
                        ..Default::default()
                    }],
                    Some(width),
                    Some(max_lines),
                )
                .map(|lines| {
                    lines
                        .iter()
                        .map(|line| line.size(px(23.)).height)
                        .sum::<Pixels>()
                })
                .unwrap_or(px(max_lines as f32 * 23.))
        };
        self.input_height =
            (measure(self.input.read(cx).value(), 4) + px(18.)).clamp(px(44.), px(100.));
        let result = if self.active.is_some() || !self.translated.is_empty() {
            (measure(self.translated.clone().into(), 14) + px(56.)).clamp(px(88.), px(320.))
                + px(8.)
        } else {
            px(0.)
        };
        let status = [&self.message, &self.warning]
            .into_iter()
            .filter(|text| !text.is_empty())
            .map(|text| measure(text.clone().into(), 3).clamp(px(23.), px(64.)) + px(8.))
            .sum::<Pixels>();
        let desired = (px(92.) + self.input_height + result + status).min(px(520.));
        let available = window
            .display(cx)
            .map(|display| {
                (display.visible_bounds().bottom() - window.bounds().top()).max(px(136.))
            })
            .unwrap_or(desired);
        let height = desired.min(available);
        if (window.viewport_size().height - height).abs() > px(1.) {
            window.resize(size(window.viewport_size().width, height));
        }
    }
    pub fn begin_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel();
        self.input.update(cx, |input, cx| {
            input.set_value(String::new(), window, cx);
            input.focus(window, cx);
        });
        self.translated.clear();
        self.message.clear();
        self.warning.clear();
        self.languages = None;
        self.copied = false;
        self.suppress_enter = false;
        self.resize(window, cx);
        cx.notify();
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
                self.languages = None;
                self.copied = false;
                self.message = self.t("正在翻译…", "Translating…").into();
            }
            Err(error) => self.message = error,
        }
        self.resize(window, cx);
        cx.notify();
    }
    pub fn handle_event(
        &mut self,
        event: &RuntimeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            RuntimeEvent::Translation { id, event } if self.active == Some(*id) => match event {
                TranslateStreamEvent::Started { from, to, .. } => {
                    self.languages = Some((from.clone(), to.clone()));
                }
                TranslateStreamEvent::Delta { content } => {
                    self.translated.push_str(content);
                    self.copied = false;
                }
            },
            RuntimeEvent::TranslationFinished { id, result } if self.active == Some(*id) => {
                self.active = None;
                match result {
                    Ok(result) => {
                        self.translated = result.translated.clone();
                        self.languages = Some((result.from.clone(), result.to.clone()));
                        self.copied = false;
                        self.message.clear();
                    }
                    Err(error) => self.message = error.clone(),
                }
            }
            RuntimeEvent::SettingsSaved {
                result: Ok(config), ..
            } => {
                self.config = config.clone();
                window.set_window_title(self.t("Rotor · 翻译", "Rotor · Translation"));
            }
            _ => return,
        }
        self.resize(window, cx);
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
            .p_3()
            .gap_2()
            .text_sm()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
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
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(IconName::Globe)
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.t("翻译", "Translate")),
                            ),
                    )
                    .child(crate::visual::caption(
                        self.t(
                            "Enter 提交 · Shift+Enter 换行",
                            "Enter to translate · Shift+Enter for a new line",
                        ),
                        cx,
                    )),
            )
            .child(Textarea::new(&self.input).h(self.input_height))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .flex_shrink_0()
                    .gap_2()
                    .child(
                        Button::new("translate")
                            .primary()
                            .disabled(self.input.read(cx).value().trim().is_empty())
                            .label(self.t("翻译", "Translate"))
                            .on_click(cx.listener(|this, _, window, cx| this.submit(window, cx))),
                    )
                    .child(
                        Button::new("cancel")
                            .icon(IconName::Pause)
                            .label(self.t("停止", "Stop"))
                            .disabled(self.active.is_none())
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.cancel();
                                this.message.clear();
                                this.resize(window, cx);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("copy")
                            .icon(if self.copied {
                                IconName::Check
                            } else {
                                IconName::Copy
                            })
                            .label(if self.copied {
                                self.t("已复制", "Copied")
                            } else {
                                self.t("复制结果", "Copy result")
                            })
                            .disabled(self.translated.is_empty())
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    this.translated.clone(),
                                ));
                                this.copied = true;
                                cx.notify();
                            })),
                    ),
            )
            .when(
                self.active.is_some() || !self.translated.is_empty(),
                |root| {
                    root.child(
                        div()
                            .id("translation-result")
                            .flex_1()
                            .min_h_0()
                            .p_4()
                            .rounded_lg()
                            .bg(cx.theme().muted)
                            .overflow_y_scroll()
                            .children(self.languages.as_ref().map(|(from, to)| {
                                crate::visual::caption(format!("{from} → {to}"), cx).mb_2()
                            }))
                            .child(div().text_size(px(14.)).line_height(relative(1.65)).child(
                                if self.translated.is_empty() {
                                    self.t("译文将显示在这里", "Your translation will appear here")
                                        .to_owned()
                                } else {
                                    self.translated.clone()
                                },
                            )),
                    )
                },
            )
            .when(!self.message.is_empty(), |root| {
                root.child(
                    div()
                        .id("translation-status")
                        .max_h(px(64.))
                        .overflow_y_scroll()
                        .text_xs()
                        .text_color(if self.active.is_some() {
                            cx.theme().muted_foreground
                        } else {
                            cx.theme().danger
                        })
                        .child(self.message.clone()),
                )
            })
            .when(!self.warning.is_empty(), |root| {
                root.child(
                    div()
                        .id("translation-warning")
                        .max_h(px(64.))
                        .overflow_y_scroll()
                        .text_xs()
                        .text_color(cx.theme().warning)
                        .child(self.warning.clone()),
                )
            })
    }
}
