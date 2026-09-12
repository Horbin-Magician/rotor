use super::*;
use rotor_runtime::{ChatMessage, TranslateStreamEvent};

impl SearchView {
    pub(super) fn on_toggle_ai(
        &mut self,
        _: &ToggleAi,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            self.toggle_ai(window, cx);
        }
        cx.stop_propagation();
    }

    pub(super) fn toggle_ai(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ai_mode = !self.ai_mode;
        self.pending_enter = false;
        if self.ai_mode {
            self.results.reset();
            self.icons.clear();
        } else {
            self.stop_chat();
            self.search(false, window, cx);
        }
        self.resize(window, cx);
        window.set_window_title(if self.ai_mode {
            "Rotor · AI"
        } else {
            search_title(&self.services.settings())
        });
        self.input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    pub(super) fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return;
        }
        let value = self.input.read(cx).value().to_string();
        if value.trim().is_empty() {
            return;
        }
        if !self.ai_mode {
            if self.results.loading {
                self.pending_enter = true;
                return;
            }
            if !self.results.items.is_empty() {
                self.open(false, false, window, cx);
                return;
            }
            // Only a completed, empty search may automatically send user text.
            if !self.results.exhausted || !self.message.is_empty() {
                return;
            }
            self.toggle_ai(window, cx);
        }
        if self.chat_request.is_some() {
            return;
        }
        let mut messages = self.conversation.clone();
        messages.push(ChatMessage {
            assistant: false,
            content: value,
        });
        match self.services.chat(messages.clone()) {
            Ok(id) => {
                self.chat_request = Some(id);
                self.chat_error.clear();
                self.chat_draft = None;
                self.conversation = messages;
                self.conversation.push(ChatMessage {
                    assistant: true,
                    content: String::new(),
                });
                self.input
                    .update(cx, |input, cx| input.set_value("", window, cx));
                self.chat_scroll.scroll_to_bottom();
            }
            Err(error) => self.chat_error = error,
        }
        self.resize(window, cx);
        cx.notify();
    }

    fn stop_chat(&mut self) {
        if let Some(id) = self.chat_request.take() {
            self.services.cancel_chat(Some(id));
            if self
                .conversation
                .last()
                .is_some_and(|message| message.content.is_empty())
            {
                self.conversation.pop();
                self.chat_draft = self.conversation.pop().map(|message| message.content);
            }
        }
    }

    pub(super) fn handle_chat_event(
        &mut self,
        event: &RuntimeEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let follow = self.chat_scroll.max_offset().y + self.chat_scroll.offset().y < px(48.);
        match event {
            RuntimeEvent::Chat {
                id,
                event: TranslateStreamEvent::Delta { content },
            } if self.chat_request == Some(*id) => {
                if let Some(message) = self.conversation.last_mut() {
                    message.content.push_str(content);
                }
            }
            RuntimeEvent::ChatFinished { id, result } if self.chat_request == Some(*id) => {
                self.chat_request = None;
                match result {
                    Ok(text) => {
                        if let Some(message) = self.conversation.last_mut() {
                            message.content = text.clone();
                        }
                    }
                    Err(error) => {
                        self.chat_error = error.clone();
                        self.conversation.pop();
                        self.chat_draft = self.conversation.pop().map(|message| message.content);
                    }
                }
            }
            _ => return false,
        }
        if follow {
            self.chat_scroll.scroll_to_bottom();
        }
        cx.notify();
        true
    }

    pub(super) fn render_chat(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let chinese = rotor_common::i18n::language_for_config(&self.services.settings()) == "zh-CN";
        let dark = cx.theme().is_dark();
        let assistant_background = rgb(if dark { 0x1c1c1c } else { 0xf5f5f5 });
        let user_background = rgb(if dark { 0x14212a } else { 0xe9f4fe });
        let messages = self
            .conversation
            .iter()
            .enumerate()
            .map(|(index, message)| {
                let text = if message.content.is_empty() {
                    if chinese {
                        "正在思考…"
                    } else {
                        "Thinking…"
                    }
                    .to_owned()
                } else {
                    message.content.clone()
                };
                div()
                    .w_full()
                    .flex()
                    .when(!message.assistant, |row| row.justify_end())
                    .child(
                        div()
                            .max_w(relative(0.9))
                            .min_w_0()
                            .rounded_lg()
                            .p_3()
                            .bg(if message.assistant {
                                assistant_background
                            } else {
                                user_background
                            })
                            .child(
                                gpui_kit::component::text::TextView::markdown(
                                    ("chat-text", index),
                                    text,
                                )
                                .selectable(true),
                            ),
                    )
            })
            .collect::<Vec<_>>();

        div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .id("chat-messages")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.chat_scroll)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .p_3()
                            .children(messages)
                            .when_some(self.chat_draft.clone(), |list, draft| {
                                list.child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_2()
                                        .rounded_lg()
                                        .p_3()
                                        .bg(assistant_background)
                                        .child(draft)
                                        .child(
                                            Button::new("retry-chat")
                                                .ghost()
                                                .label(if chinese { "重试" } else { "Retry" })
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    if let Some(draft) = this.chat_draft.clone() {
                                                        this.input.update(cx, |input, cx| {
                                                            input.set_value(draft, window, cx)
                                                        });
                                                        this.submit(window, cx);
                                                    }
                                                })),
                                        ),
                                )
                            })
                            .when(!self.chat_error.is_empty(), |list| {
                                list.child(
                                    div()
                                        .p_3()
                                        .rounded_lg()
                                        .bg(assistant_background)
                                        .text_color(cx.theme().danger)
                                        .child(self.chat_error.clone()),
                                )
                            }),
                    ),
            )
            .when(
                !self.conversation.is_empty()
                    || self.chat_draft.is_some()
                    || !self.chat_error.is_empty(),
                |list| list.child(Scrollbar::vertical(&self.chat_scroll)),
            )
    }
}

impl Drop for SearchView {
    fn drop(&mut self) {
        if let Some(id) = self.chat_request.take() {
            self.services.cancel_chat(Some(id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SearchView;
    use gpui_kit::component::Root;
    use gpui_kit::{AppContext, TestAppContext};
    use rotor_common::ConfigService;
    use rotor_runtime::{OperationId, RuntimeEvent, Services, TranslateStreamEvent};
    use rotor_runtime::{QueryId, SearchBatch, ServiceOptions};
    use std::sync::Arc;
    use std::sync::Mutex;

    #[gpui::test]
    fn keyboard_modes_empty_results_and_stale_replies(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let config = ConfigService::load_from(directory.path()).unwrap();
        let (services, _events) = Services::new(
            Arc::new(Mutex::new(config)),
            None,
            ServiceOptions { index_files: false },
        )
        .unwrap();
        let services = Arc::new(services);
        cx.update(gpui_kit::component::init);
        let mut view = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let search = cx.new(|cx| SearchView::new(services.clone(), window, cx));
            view = Some(search.clone());
            Root::new(search, window, cx)
        });
        let view = view.unwrap();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_keystrokes("tab");
        view.read_with(cx, |view, _| assert!(view.ai_mode));
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.input
                    .update(cx, |input, cx| input.set_value("Hello AI", window, cx));
            });
            window.draw(cx).clear(cx);
        });
        cx.simulate_keystrokes("enter");
        cx.update(|_, cx| {
            view.update(cx, |view, _| {
                assert!(view.chat_request.is_some());
                assert_eq!(view.conversation[0].content, "Hello AI");
                view.stop_chat();
                view.conversation.clear();
                view.chat_draft = None;
            });
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_keystrokes("tab");
        view.read_with(cx, |view, _| assert!(!view.ai_mode));
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.input.update(cx, |input, cx| {
                    input.set_value("Explain ownership", window, cx)
                });
                view.results
                    .begin(QueryId(12), "Explain ownership".into(), false);
                view.message.clear();
                view.submit(window, cx);
                assert!(view.pending_enter);
                assert!(!view.ai_mode);
                view.handle_event(
                    &RuntimeEvent::Search(SearchBatch {
                        id: QueryId(11),
                        query: "Explain ownership".into(),
                        items: vec![],
                        append: false,
                    }),
                    window,
                    cx,
                );
                assert!(!view.ai_mode);
                view.handle_event(
                    &RuntimeEvent::Search(SearchBatch {
                        id: QueryId(12),
                        query: "Explain ownership".into(),
                        items: vec![],
                        append: false,
                    }),
                    window,
                    cx,
                );
                assert!(view.ai_mode);
                let id = view.chat_request.unwrap();
                assert_eq!(view.conversation[0].content, "Explain ownership");
                assert!(!view.handle_chat_event(
                    &RuntimeEvent::Chat {
                        id: OperationId(id.0 + 100),
                        event: TranslateStreamEvent::Delta {
                            content: "stale".into()
                        },
                    },
                    cx
                ));
                assert!(view.conversation[1].content.is_empty());
                view.handle_chat_event(
                    &RuntimeEvent::Chat {
                        id,
                        event: TranslateStreamEvent::Delta {
                            content: "**Ownership** manages memory.".into(),
                        },
                    },
                    cx,
                );
                view.stop_chat();
                assert_eq!(
                    view.conversation[1].content,
                    "**Ownership** manages memory."
                );
                assert!(!view.handle_chat_event(
                    &RuntimeEvent::ChatFinished {
                        id,
                        result: Ok("late".into())
                    },
                    cx
                ));
            });
            window.draw(cx).clear(cx);
        });
        cx.update(|window, cx| {
            // Finish the pending AI expansion, then switch with a nonempty
            // conversation. Native bounds must stay put until search is drawn.
            window.simulate_next_frame(cx);
            let expanded = window.bounds().size;
            view.update(cx, |view, cx| {
                view.toggle_ai(window, cx);
                assert!(!view.ai_mode);
                assert!(!view.conversation.is_empty());
            });
            window.simulate_next_frame(cx);
            assert_eq!(window.bounds().size, expanded);
            window.draw(cx).clear(cx);

            // A second toggle before the queued resize invalidates that resize.
            view.update(cx, |view, cx| view.toggle_ai(window, cx));
            window.simulate_next_frame(cx);
            assert_eq!(window.bounds().size, expanded);
            window.draw(cx).clear(cx);
            window.simulate_next_frame(cx);

            view.update(cx, |view, cx| view.toggle_ai(window, cx));
            window.draw(cx).clear(cx);
            window.simulate_next_frame(cx);
            assert_eq!(window.bounds().size.height, gpui_kit::px(50.));
            view.read_with(cx, |view, _| {
                assert!(!view.ai_mode);
                assert!(!view.conversation.is_empty());
            });
        });
        services.shutdown();
    }
}
