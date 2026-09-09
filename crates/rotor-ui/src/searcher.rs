use super::search_results::{MAX_RESULTS, SearchResults};
use base64::prelude::*;
use gpui_kit::{
    component::{
        ActiveTheme, Disableable, IconName,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        scroll::Scrollbar,
    },
    prelude::*,
    *,
};
use rotor_runtime::{IndexState, OperationId, RuntimeEvent, Services};
use std::{collections::HashMap, ops::Range, sync::Arc, time::Duration};

const SEARCH_HEADER_HEIGHT: f32 = 50.;
const SEARCH_ROW_HEIGHT: f32 = 60.;

pub struct SearchView {
    services: Arc<Services>,
    input: Entity<InputState>,
    results: SearchResults,
    icons: HashMap<String, Arc<Image>>,
    scroll: UniformListScrollHandle,
    opening: Option<OperationId>,
    index_request: Option<OperationId>,
    index_state: IndexState,
    index_state_changed: bool,
    message: String,
    suppress_enter: bool,
    hovered: Option<usize>,
    hover_generation: usize,
    _input_events: Subscription,
    _activation: Subscription,
}
impl SearchView {
    pub fn new(services: Arc<Services>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        window.set_window_title(search_title(&services.settings()));
        services.update_search();
        let initial = services.search(String::new());
        let index_state = if initial.is_ok() {
            IndexState::Loading
        } else {
            IndexState::Unavailable
        };
        let message = initial.err().unwrap_or_default();
        let index_request = services.request_index_status().ok();
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
            index_request,
            index_state,
            index_state_changed: false,
            message,
            suppress_enter: false,
            hovered: None,
            hover_generation: 0,
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
            self.hovered = None;
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
            RuntimeEvent::IndexState(state) => {
                self.index_state = *state;
                self.index_state_changed = true;
            }
            RuntimeEvent::IndexStatus { id, result } if self.index_request == Some(*id) => {
                self.index_request = None;
                match result {
                    Ok(status) if !self.index_state_changed => self.index_state = status.state,
                    Err(error) => self.message = error.clone(),
                    _ => {}
                }
            }
            RuntimeEvent::Search(batch) => {
                if !self.results.accept(batch) {
                    return;
                }
                if !batch.append {
                    self.hovered = None;
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
            RuntimeEvent::SettingsSaved {
                result: Ok(config), ..
            } => {
                window.set_window_title(search_title(config));
            }
            _ => return,
        }
        cx.notify();
    }
    fn resize(&self, window: &mut Window, cx: &App) {
        let desired = px(SEARCH_HEADER_HEIGHT
            + 2.
            + SEARCH_ROW_HEIGHT * self.results.items.len().clamp(1, 7) as f32);
        let available = window
            .display(cx)
            .map(|display| {
                (display.visible_bounds().bottom() - window.bounds().top()).max(px(112.))
            })
            .unwrap_or(desired);
        window.resize(size(window.viewport_size().width, desired.min(available)));
    }

    fn rows(
        &mut self,
        range: Range<usize>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        // Request the next page only when the viewport reaches the loaded tail.
        // Defer mutations until the virtual list has finished laying out its rows.
        let count = self.results.items.len();
        if range.end >= count
            && count > 0
            && count < MAX_RESULTS
            && !self.results.loading
            && !self.results.exhausted
            && self.message.is_empty()
        {
            let query = self.input.read(cx).value();
            cx.defer_in(window, move |this, window, cx| {
                if this.results.items.len() == count
                    && this.input.read(cx).value() == query
                    && this.message.is_empty()
                {
                    this.search(true, window, cx);
                }
            });
        }
        let chinese = rotor_common::i18n::language_for_config(&self.services.settings()) == "zh-CN";
        let hover_color = if cx.theme().is_dark() {
            rgb(0x15212a).into()
        } else {
            cx.theme().list_hover
        };
        range
            .map(|index| {
                let item = &self.results.items[index];
                let (display_name, is_app) = result_label(&item.file_name);
                let icon = self.icons.get(&item.file_path).cloned();
                div()
                    .id(("result", index))
                    .relative()
                    .overflow_hidden()
                    .h(px(SEARCH_ROW_HEIGHT))
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(12.))
                    .cursor_pointer()
                    .when(index == self.results.selected, |row| {
                        row.bg(if cx.theme().is_dark() {
                            rgb(0x15212a).into()
                        } else {
                            cx.theme().list_active
                        })
                        .text_color(cx.theme().foreground)
                    })
                    .when(self.hovered == Some(index), |row| row.bg(hover_color))
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        if *hovered && this.hovered != Some(index) {
                            this.hovered = Some(index);
                            this.hover_generation = this.hover_generation.wrapping_add(1);
                        } else if !*hovered && this.hovered == Some(index) {
                            this.hovered = None;
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .size(px(34.))
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
                            .gap(px(4.))
                            .min_w_0()
                            .flex_1()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .min_w_0()
                                    .child(
                                        div()
                                            .min_w_0()
                                            .text_size(px(16.))
                                            .truncate()
                                            .child(display_name.to_owned()),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .rounded_full()
                                            .px(px(6.))
                                            .text_size(px(11.))
                                            .line_height(px(16.))
                                            .bg(rgb(if is_app { 0x306acb } else { 0x09966e }))
                                            .text_color(rgb(if is_app {
                                                0xb9d7ff
                                            } else {
                                                0xa0e8d4
                                            }))
                                            .child(match (is_app, chinese) {
                                                (true, true) => "应用",
                                                (true, false) => "App",
                                                (false, true) => "文件",
                                                (false, false) => "File",
                                            }),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(if cx.theme().is_dark() {
                                        rgb(0xbfc1c3).into()
                                    } else {
                                        cx.theme().muted_foreground
                                    })
                                    .truncate()
                                    .child(item.path.clone()),
                            ),
                    )
                    .when(self.hovered == Some(index), |row| {
                        let width = if cfg!(target_os = "windows") {
                            104.
                        } else {
                            56.
                        };
                        row.child(
                            div()
                                .absolute()
                                .top_0()
                                .h_full()
                                .w(px(width))
                                .px_2()
                                .flex()
                                .items_center()
                                .justify_end()
                                .gap_2()
                                .bg(hover_color)
                                .when(cfg!(target_os = "windows"), |bar| {
                                    bar.child(
                                        Button::new(("open-admin", index))
                                            .ghost()
                                            .icon(IconName::CircleUser)
                                            .tooltip(if chinese {
                                                "管理员打开"
                                            } else {
                                                "Open as administrator"
                                            })
                                            .disabled(self.opening.is_some())
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                cx.stop_propagation();
                                                this.results.selected = index;
                                                this.open(false, true, cx);
                                            })),
                                    )
                                })
                                .child(
                                    Button::new(("open-folder", index))
                                        .ghost()
                                        .icon(IconName::FolderOpen)
                                        .tooltip(if chinese {
                                            "打开目录"
                                        } else {
                                            "Open folder"
                                        })
                                        .disabled(self.opening.is_some())
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            cx.stop_propagation();
                                            this.results.selected = index;
                                            this.open(true, false, cx);
                                        })),
                                )
                                .with_animation(
                                    ("reveal-actions", self.hover_generation),
                                    Animation::new(Duration::from_millis(160)),
                                    move |bar, progress| {
                                        let eased = 1. - (1. - progress).powi(3);
                                        bar.right(px(-width * (1. - eased)))
                                    },
                                ),
                        )
                    })
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
        } else if self.index_state != IndexState::Ready {
            let (zh, en) = match self.index_state {
                IndexState::Unavailable => ("文件索引未启用", "File indexing is disabled"),
                IndexState::Unbuild => ("文件索引尚未构建", "File index has not been built"),
                IndexState::Building => ("正在构建文件索引…", "Building the file index…"),
                IndexState::Loading => ("正在加载文件索引…", "Loading the file index…"),
                IndexState::Released => (
                    "索引已释放，输入后重新加载",
                    "Index released; type to load it again",
                ),
                IndexState::Error => (
                    "索引失败，请在设置中重建",
                    "Index failed; rebuild it in Settings",
                ),
                IndexState::Ready => unreachable!(),
            };
            if chinese { zh } else { en }.into()
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
            String::new()
        };
        div()
            .id("searcher")
            .flex()
            .flex_col()
            .size_full()
            .rounded(px(10.))
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .text_sm()
            .bg(if cx.theme().is_dark() {
                rgb(0x111111).into()
            } else {
                cx.theme().background
            })
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
                div()
                    .mx(px(12.))
                    .h(px(SEARCH_HEADER_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .border_b_2()
                    .border_color(rgb(0x6099ed))
                    .child(
                        Input::new(&self.input)
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .h(px(42.))
                            .text_size(px(18.))
                            .prefix(IconName::Search)
                            .aria_label(if chinese {
                                "搜索文件"
                            } else {
                                "Search files"
                            }),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .child(
                        uniform_list("results", count, move |range, window, cx| {
                            weak.update(cx, |this, cx| this.rows(range, window, cx))
                                .unwrap_or_default()
                        })
                        .track_scroll(&self.scroll)
                        .size_full(),
                    )
                    .child(Scrollbar::vertical(&self.scroll))
                    .when(
                        count == 0
                            || !self.message.is_empty()
                            || self.index_state == IndexState::Error,
                        |list| {
                            list.child(
                                div()
                                    .id("search-status")
                                    .absolute()
                                    .bottom_0()
                                    .left_0()
                                    .w_full()
                                    .px_3()
                                    .py_2()
                                    .max_h(px(60.))
                                    .overflow_y_scroll()
                                    .text_size(px(12.))
                                    .bg(if cx.theme().is_dark() {
                                        rgb(0x111111).into()
                                    } else {
                                        cx.theme().background
                                    })
                                    .text_color(
                                        if self.message.is_empty()
                                            && self.index_state != IndexState::Error
                                        {
                                            cx.theme().muted_foreground
                                        } else {
                                            cx.theme().danger
                                        },
                                    )
                                    .child(status),
                            )
                        },
                    ),
            )
    }
}

fn search_title(config: &rotor_common::Config) -> &'static str {
    if rotor_common::i18n::language_for_config(config) == "zh-CN" {
        "Rotor · 文件搜索"
    } else {
        "Rotor · File search"
    }
}

// Only simplify application suffixes in the label; opening always uses the
// original result path, and ordinary file extensions remain visible.
fn result_label(name: &str) -> (&str, bool) {
    if let Some((stem, extension)) = name.rsplit_once('.')
        && !stem.is_empty()
        && ["exe", "lnk", "app"]
            .iter()
            .any(|app| extension.eq_ignore_ascii_case(app))
    {
        (stem, true)
    } else {
        (name, false)
    }
}
