use gpui_kit::{
    component::{
        ActiveTheme, Disableable, IconName,
        button::{Button, ButtonCustomVariant, ButtonVariants},
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
    focus: FocusHandle,
    selection_mode: bool,
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
                if !this.selection_mode && !this.suppress_enter {
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
            focus: cx.focus_handle(),
            selection_mode: false,
            input_height: px(68.),
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
        view.update_labels(window, cx);
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
    fn update_labels(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.set_window_title(self.t("Rotor · 翻译", "Rotor · Translation"));
        let placeholder = self.t(
            "输入或粘贴要翻译的文本…",
            "Enter or paste text to translate…",
        );
        self.input.update(cx, |input, cx| {
            input.set_placeholder(placeholder, window, cx);
        });
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
            (measure(self.input.read(cx).value(), 4) + px(18.)).clamp(px(68.), px(110.));
        let result = if self.active.is_some() || !self.translated.is_empty() {
            (measure(self.translated.clone().into(), 14) + px(64.)).clamp(px(104.), px(320.))
                + px(8.)
        } else {
            px(0.)
        };
        let status = [&self.message, &self.warning]
            .into_iter()
            .filter(|text| !text.is_empty())
            .map(|text| measure(text.clone().into(), 3).clamp(px(23.), px(64.)) + px(8.))
            .sum::<Pixels>();
        let controls = if self.selection_mode {
            px(56.)
        } else {
            px(104.) + self.input_height
        };
        let desired = (controls + result + status).min(px(520.));
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
        self.selection_mode = false;
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
        self.selection_mode = true;
        self.suppress_enter = false;
        self.input
            .update(cx, |input, cx| input.set_value(text, window, cx));
        self.focus.focus(window, cx);
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
                self.message.clear();
            }
            Err(error) => {
                self.cancel();
                self.message = error;
            }
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
                self.update_labels(window, cx);
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
fn quiet_button(button: Button, cx: &App) -> Button {
    button.h(px(28.)).rounded_sm().custom(
        ButtonCustomVariant::new(cx)
            .color(cx.theme().background)
            .foreground(cx.theme().muted_foreground)
            .hover(cx.theme().list_hover)
            .active(cx.theme().muted),
    )
}

impl Render for TranslatorView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.active.is_some();
        let has_input = !self.input.read(cx).value().is_empty();
        div()
            .id("translator")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .overflow_y_scroll()
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
                    .h(px(32.))
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().text_color(cx.theme().primary).child(IconName::Globe))
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.t("翻译", "Translate")),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .when(
                                !self.selection_mode && (has_input || !self.translated.is_empty()),
                                |row| {
                                    row.child(
                                        quiet_button(
                                            Button::new("clear").label(self.t("清空", "Clear")),
                                            cx,
                                        )
                                        .on_click(
                                            cx.listener(|this, _, window, cx| {
                                                this.begin_input(window, cx)
                                            }),
                                        ),
                                    )
                                },
                            )
                            .child(
                                quiet_button(
                                    Button::new("close")
                                        .icon(IconName::WindowClose)
                                        .w(px(28.))
                                        .tooltip(self.t("关闭 · Esc", "Close · Esc"))
                                        .accessibility_label(
                                            self.t("关闭翻译", "Close translation"),
                                        ),
                                    cx,
                                )
                                .on_click(cx.listener(
                                    |this, _, window, _| {
                                        this.cancel();
                                        window.remove_window();
                                    },
                                )),
                            ),
                    ),
            )
            .when(!self.selection_mode, |root| {
                root.child(
                    div().flex_shrink_0().child(
                        Textarea::new(&self.input)
                            .h(self.input_height)
                            .rounded_md()
                            .text_size(px(14.))
                            .aria_label(
                                self.t(
                                    "输入或粘贴要翻译的文本",
                                    "Enter or paste text to translate",
                                ),
                            ),
                    ),
                )
            })
            .when(!self.selection_mode, |root| {
                root.child(
                    div()
                        .flex()
                        .h(px(32.))
                        .flex_shrink_0()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            crate::visual::caption(
                                self.t(
                                    "Enter 翻译 · Shift+Enter 换行",
                                    "Enter to translate · Shift+Enter for newline",
                                ),
                                cx,
                            )
                            .min_w_0(),
                        )
                        .child(div().flex_shrink_0().map(|row| {
                            if busy {
                                row.child(
                                    quiet_button(
                                        Button::new("cancel")
                                            .icon(IconName::Pause)
                                            .label(self.t("停止", "Stop")),
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, window, cx| {
                                            this.cancel();
                                            this.message.clear();
                                            this.resize(window, cx);
                                            cx.notify();
                                        },
                                    )),
                                )
                            } else {
                                row.child(
                                    Button::new("translate")
                                        .primary()
                                        .h(px(28.))
                                        .rounded_sm()
                                        .disabled(self.input.read(cx).value().trim().is_empty())
                                        .label(if self.message.is_empty() {
                                            self.t("翻译", "Translate")
                                        } else {
                                            self.t("重试", "Retry")
                                        })
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.submit(window, cx)
                                        })),
                                )
                            }
                        })),
                )
            })
            .when(busy || !self.translated.is_empty(), |root| {
                root.child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h(px(104.))
                        .overflow_hidden()
                        .rounded_md()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().muted)
                        .child(
                            div()
                                .flex()
                                .flex_shrink_0()
                                .h(px(36.))
                                .px_3()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .child(
                                    crate::visual::caption(
                                        if busy {
                                            self.t("正在翻译…", "Translating…").to_owned()
                                        } else {
                                            self.languages
                                                .as_ref()
                                                .map(|(from, to)| format!("{from} → {to}"))
                                                .unwrap_or_else(|| {
                                                    self.t("译文", "Translation").into()
                                                })
                                        },
                                        cx,
                                    )
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis(),
                                )
                                .when(!self.translated.is_empty(), |header| {
                                    header.child(
                                        quiet_button(
                                            Button::new("copy")
                                                .icon(if self.copied {
                                                    IconName::Check
                                                } else {
                                                    IconName::Copy
                                                })
                                                .label(if self.copied {
                                                    self.t("已复制", "Copied")
                                                } else {
                                                    self.t("复制", "Copy")
                                                }),
                                            cx,
                                        )
                                        .on_click(
                                            cx.listener(|this, _, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    this.translated.clone(),
                                                ));
                                                this.copied = true;
                                                cx.notify();
                                            }),
                                        ),
                                    )
                                }),
                        )
                        .child(
                            div()
                                .id("translation-result")
                                .flex_1()
                                .min_h_0()
                                .overflow_y_scroll()
                                .p_3()
                                .text_size(px(14.))
                                .line_height(px(23.))
                                .child(self.translated.clone()),
                        ),
                )
            })
            .when(!self.message.is_empty(), |root| {
                root.child(
                    div()
                        .id("translation-status")
                        .flex_shrink_0()
                        .max_h(px(64.))
                        .overflow_y_scroll()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(format!(
                            "{}{}",
                            self.t("翻译失败：", "Translation failed: "),
                            self.message
                        )),
                )
            })
            .when(!self.warning.is_empty(), |root| {
                root.child(
                    div()
                        .id("translation-warning")
                        .flex_shrink_0()
                        .max_h(px(64.))
                        .overflow_y_scroll()
                        .text_xs()
                        .text_color(cx.theme().warning)
                        .child(self.warning.clone()),
                )
            })
    }
}
