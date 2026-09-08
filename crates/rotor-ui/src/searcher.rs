use super::search_results::{MAX_RESULTS, SearchResults};
use base64::prelude::*;
use gpui_kit::{
    component::{
        ActiveTheme, Disableable, IconName,
        button::Button,
        input::{Input, InputEvent, InputState},
    },
    prelude::*,
    *,
};
use rotor_runtime::{OperationId, RuntimeEvent, Services};
use std::{collections::HashMap, ops::Range, sync::Arc};

pub struct SearchView {
    services: Arc<Services>,
    input: Entity<InputState>,
    results: SearchResults,
    icons: HashMap<String, Arc<Image>>,
    scroll: UniformListScrollHandle,
    opening: Option<OperationId>,
    message: String,
    suppress_enter: bool,
    _input_events: Subscription,
    _activation: Subscription,
}
impl SearchView {
    pub fn new(services: Arc<Services>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        services.update_search();
        let _ = services.search(String::new());
        let input = cx.new(|cx| InputState::new(window, cx));
        let input_events =
            cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
                InputEvent::Change => this.search(false, window, cx),
                InputEvent::PressEnter { .. } => {
                    if !this.suppress_enter {
                        this.open(false, false, cx);
                    }
                    this.suppress_enter = false;
                }
                _ => {}
            });
        let activation = cx.observe_window_activation(window, |this, window, _| {
            if !window.is_window_active() && this.opening.is_none() {
                window.remove_window();
            }
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        Self {
            services,
            input,
            results: SearchResults::default(),
            icons: HashMap::new(),
            scroll: UniformListScrollHandle::new(),
            opening: None,
            message: String::new(),
            suppress_enter: false,
            _input_events: input_events,
            _activation: activation,
        }
    }
    fn search(&mut self, append: bool, window: &mut Window, cx: &mut Context<Self>) {
        if append
            && (self.results.loading
                || self.results.exhausted
                || self.results.items.len() >= MAX_RESULTS)
        {
            return;
        }
        let query = self.input.read(cx).value().to_string();
        if !append {
            self.results.reset();
            self.icons.clear();
            self.resize(window, cx);
            self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        }
        match self.services.search(query.clone()) {
            Ok(id) if !query.is_empty() => {
                self.results.begin(id, query, append);
                self.message.clear();
            }
            Ok(_) => self.message.clear(),
            Err(error) => self.message = error,
        }
        cx.notify();
    }
    fn open(&mut self, folder: bool, admin: bool, cx: &mut Context<Self>) {
        if self.opening.is_some() {
            return;
        }
        let Some(item) = self.results.items.get(self.results.selected) else {
            return;
        };
        let path = if folder {
            item.path.clone()
        } else {
            item.file_path.clone()
        };
        match self.services.open_file(path, admin) {
            Ok(id) => self.opening = Some(id),
            Err(error) => self.message = error,
        }
        cx.notify();
    }
    pub fn handle_event(
        &mut self,
        event: &RuntimeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            RuntimeEvent::Search(batch) => {
                if !self.results.accept(batch) {
                    return;
                }
                self.icons.retain(|path, _| {
                    self.results
                        .items
                        .iter()
                        .any(|item| &item.file_path == path)
                });
                for item in &self.results.items {
                    if !self.icons.contains_key(&item.file_path)
                        && let Some(bytes) = item
                            .icon_data
                            .as_ref()
                            .and_then(|value| BASE64_STANDARD.decode(value).ok())
                    {
                        self.icons.insert(
                            item.file_path.clone(),
                            Arc::new(Image::from_bytes(ImageFormat::Png, bytes)),
                        );
                    }
                }
                self.resize(window, cx);
            }
            RuntimeEvent::FileOpened { id, result } if self.opening == Some(*id) => {
                self.opening = None;
                match result {
                    Ok(()) => window.remove_window(),
                    Err(error) => self.message = error.clone(),
                }
            }
            _ => return,
        }
        cx.notify();
    }
    fn resize(&self, window: &mut Window, cx: &App) {
        let desired = px(140. + 58. * self.results.items.len().min(6) as f32);
        let available = window
            .display(cx)
            .map(|display| {
                (display.visible_bounds().bottom() - window.bounds().top()).max(px(140.))
            })
            .unwrap_or(desired);
        window.resize(size(window.viewport_size().width, desired.min(available)));
    }

    fn rows(&mut self, range: Range<usize>, cx: &mut Context<Self>) -> Vec<AnyElement> {
        range
            .map(|index| {
                let item = &self.results.items[index];
                let icon = self.icons.get(&item.file_path).cloned();
                div()
                    .id(("result", index))
                    .h(px(58.))
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_3()
                    .px_3()
                    .rounded_lg()
                    .when(index == self.results.selected, |row| {
                        row.bg(cx.theme().list_active)
                            .text_color(cx.theme().foreground)
                    })
                    .when(index != self.results.selected, |row| {
                        row.hover(|row| row.bg(cx.theme().list_hover))
                    })
                    .child(
                        div()
                            .size(px(32.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(icon.is_none(), |icon| icon.child(IconName::File))
                            .children(icon.map(|icon| img(icon).size_full())),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::MEDIUM)
                                    .truncate()
                                    .child(item.file_name.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .truncate()
                                    .child(item.path.clone()),
                            ),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.results.selected = index;
                        this.open(false, false, cx);
                    }))
                    .into_any_element()
            })
            .collect()
    }
}
impl Render for SearchView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let chinese = rotor_common::i18n::language_for_config(&self.services.settings()) == "zh-CN";
        let weak = cx.weak_entity();
        let count = self.results.items.len();
        let status = if !self.message.is_empty() {
            self.message.clone()
        } else if self.opening.is_some() {
            if chinese {
                "正在打开…"
            } else {
                "Opening…"
            }
            .into()
        } else if self.results.loading {
            if chinese {
                "搜索中…"
            } else {
                "Searching…"
            }
            .into()
        } else if self.input.read(cx).value().is_empty() {
            if chinese {
                "输入文件名开始搜索"
            } else {
                "Type a file name to search"
            }
            .into()
        } else if count == 0 {
            if chinese {
                "未找到匹配文件"
            } else {
                "No matching files"
            }
            .into()
        } else {
            format!("{} {}", count, if chinese { "个结果" } else { "results" })
        };
        div()
            .id("searcher")
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
                if composing {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "escape" => {
                        window.remove_window();
                        cx.stop_propagation();
                    }
                    "up" => {
                        this.results.selected = this.results.selected.saturating_sub(1);
                        cx.stop_propagation();
                    }
                    "down" => {
                        this.results.selected = (this.results.selected + 1)
                            .min(this.results.items.len().saturating_sub(1));
                        cx.stop_propagation();
                    }
                    _ => return,
                }
                this.scroll
                    .scroll_to_item(this.results.selected, ScrollStrategy::Nearest);
                cx.notify();
            }))
            .child(
                Input::new(&self.input)
                    .prefix(IconName::Search)
                    .aria_label(if chinese {
                        "搜索文件"
                    } else {
                        "Search files"
                    }),
            )
            .child(
                uniform_list("results", count, move |range, _, cx| {
                    weak.update(cx, |this, cx| this.rows(range, cx))
                        .unwrap_or_default()
                })
                .track_scroll(&self.scroll)
                .flex_1()
                .min_h_0(),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .flex_shrink_0()
                    .gap_2()
                    .child(
                        Button::new("open-folder")
                            .compact()
                            .icon(IconName::FolderOpen)
                            .label(if chinese {
                                "打开目录"
                            } else {
                                "Open folder"
                            })
                            .disabled(count == 0 || self.opening.is_some())
                            .on_click(cx.listener(|this, _, _, cx| this.open(true, false, cx))),
                    )
                    .when(cfg!(target_os = "windows"), |row| {
                        row.child(
                            Button::new("open-admin")
                                .compact()
                                .label(if chinese {
                                    "管理员打开"
                                } else {
                                    "Open as administrator"
                                })
                                .disabled(count == 0 || self.opening.is_some())
                                .on_click(cx.listener(|this, _, _, cx| this.open(false, true, cx))),
                        )
                    })
                    .child(
                        Button::new("load-more")
                            .compact()
                            .label(if chinese { "加载更多" } else { "Load more" })
                            .disabled(
                                self.results.loading
                                    || self.results.exhausted
                                    || count == 0
                                    || count >= MAX_RESULTS,
                            )
                            .on_click(
                                cx.listener(|this, _, window, cx| this.search(true, window, cx)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap_2()
                    .flex_shrink_0()
                    .child(
                        div()
                            .id("search-status")
                            .text_xs()
                            .max_h(px(48.))
                            .overflow_y_scroll()
                            .text_color(if self.message.is_empty() {
                                cx.theme().muted_foreground
                            } else {
                                cx.theme().danger
                            })
                            .child(status),
                    )
                    .child(crate::visual::caption(
                        if chinese {
                            "↑↓ 选择 · Enter 打开 · Esc 关闭"
                        } else {
                            "↑↓ Select · Enter Open · Esc Close"
                        },
                        cx,
                    )),
            )
    }
}
