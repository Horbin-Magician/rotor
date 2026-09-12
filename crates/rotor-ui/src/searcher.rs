mod chat;
use super::search_results::{MAX_RESULTS, SearchResults};
use gpui_kit::{
    component::{
        ActiveTheme, Disableable, Icon,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState},
        scroll::Scrollbar,
    },
    prelude::*,
    *,
};
use rotor_runtime::{IndexState, OperationId, RuntimeEvent, Services};
use std::{collections::HashMap, ops::Range, sync::Arc, time::Duration};

actions!(rotor_search, [ToggleAi]);
struct SearchKeybindings;
impl Global for SearchKeybindings {}

const SEARCH_HEADER_HEIGHT: f32 = 50.;
const SEARCH_ROW_HEIGHT: f32 = 60.;

pub struct SearchView {
    ai_mode: bool,
    conversation: Vec<rotor_runtime::ChatMessage>,
    chat_request: Option<OperationId>,
    chat_error: String,
    chat_draft: Option<String>,
    pending_enter: bool,
    chat_scroll: ScrollHandle,
    services: Arc<Services>,
    input: Entity<InputState>,
    results: SearchResults,
    icons: HashMap<String, Arc<RenderImage>>,
    scroll: UniformListScrollHandle,
    opening: Option<OperationId>,
    index_request: Option<OperationId>,
    index_state: IndexState,
    index_state_changed: bool,
    message: String,
    suppress_enter: bool,
    hovered: Option<usize>,
    hover_generation: usize,
    pointer: Option<Point<Pixels>>,
    blur_task: Option<Task<()>>,
    resize_pending: bool,
    _input_events: Subscription,
    _activation: Subscription,
}
impl SearchView {
    pub fn new(services: Arc<Services>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        if !cx.has_global::<SearchKeybindings>() {
            cx.bind_keys([KeyBinding::new("tab", ToggleAi, Some("RotorSearch"))]);
            cx.set_global(SearchKeybindings);
        }
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
                        this.submit(window, cx);
                    }
                    this.suppress_enter = false;
                }
                _ => {}
            });
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            this.blur_task = None;
            if window.is_window_active() {
                this.input.update(cx, |input, cx| input.focus(window, cx));
            } else {
                // Brief native focus transfers should not dismiss the launcher.
                this.blur_task = Some(cx.spawn_in(window, async move |view, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    let _ = view.update_in(cx, |_, window, _| {
                        if !window.is_window_active() {
                            window.remove_window();
                        }
                    });
                }));
            }
        });
        input.update(cx, |input, cx| input.focus(window, cx));
        Self {
            ai_mode: false,
            conversation: Vec::new(),
            chat_request: None,
            chat_error: String::new(),
            chat_draft: None,
            pending_enter: false,
            chat_scroll: ScrollHandle::new(),
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
            pointer: None,
            blur_task: None,
            resize_pending: false,
            _input_events: input_events,
            _activation: activation,
        }
    }
    fn search(&mut self, append: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.ai_mode {
            return;
        }
        if !append {
            self.pending_enter = false;
        }
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
            self.results.selected = 0;
            if query.trim().is_empty() {
                self.results.reset();
                self.icons.clear();
                self.resize(window, cx);
            }
            self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        }
        match self.services.search(query.clone()) {
            Ok(id) if !query.trim().is_empty() => {
                self.results.begin(id, query, append);
                self.message.clear();
            }
            Ok(_) => self.message.clear(),
            Err(error) => {
                self.results.reset();
                self.icons.clear();
                self.resize(window, cx);
                self.message = error;
            }
        }
        cx.notify();
    }
    fn open(&mut self, folder: bool, admin: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.opening.is_some() || self.results.replacing {
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
            Ok(id) => {
                self.opening = Some(id);
                window.remove_window();
            }
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
        if self.handle_chat_event(event, cx) {
            self.resize(window, cx);
            return;
        }
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
                for item in &mut self.results.items {
                    // The render cache owns the converted pixels. Release the
                    // source after this event instead of retaining both copies.
                    if let Some(icon) = item.icon.take()
                        && !self.icons.contains_key(&item.file_path)
                        && let Ok(prepared) = crate::capture::prepare_image(icon)
                    {
                        self.icons.insert(item.file_path.clone(), prepared.render);
                    }
                }
                self.resize(window, cx);
                if self.pending_enter {
                    self.pending_enter = false;
                    self.submit(window, cx);
                }
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
                window.set_window_title(if self.ai_mode {
                    "Rotor · AI"
                } else {
                    search_title(config)
                });
            }
            _ => return,
        }
        cx.notify();
    }
    fn resize(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.resize_pending = true;
        cx.notify();
    }

    fn schedule_resize_after_render(&mut self, window: &Window, cx: &Context<Self>) {
        if !std::mem::take(&mut self.resize_pending) {
            return;
        }
        // Paint the new mode before resizing the native surface; otherwise the
        // compositor can stretch the previous chat frame during the transition.
        // Register during render: GPUI runs next-frame callbacks BEFORE drawing
        // that next frame. Registering in the input handler would be too early.
        let view = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = view.update(cx, |view, cx| {
                // A newer mode/search change must be painted first as well.
                if !view.resize_pending {
                    view.apply_resize(window, cx);
                }
            });
        });
    }

    fn apply_resize(&self, window: &mut Window, cx: &App) {
        let desired = px(if self.ai_mode {
            if self.conversation.is_empty()
                && self.chat_draft.is_none()
                && self.chat_error.is_empty()
            {
                SEARCH_HEADER_HEIGHT
            } else {
                search_height(7)
            }
        } else {
            search_height(self.results.items.len())
        });
        let available = window
            .display(cx)
            .map(|display| {
                (display.visible_bounds().bottom() - window.bounds().top())
                    .max(px(SEARCH_HEADER_HEIGHT))
            })
            .unwrap_or(desired);
        let current = window.viewport_size();
        let target = size(current.width, desired.min(available));
        if target != current {
            window.resize(target);
        }
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
        let dark = cx.theme().is_dark();
        let selected_color = rgb(if dark { 0x14212a } else { 0xe9f4fe });
        let foreground = rgb(if dark { 0xf6f6f6 } else { 0x121212 });
        let secondary = rgb(if dark { 0xcccccc } else { 0x666666 });
        range
            .map(|index| {
                let item = &self.results.items[index];
                let (display_name, is_app) = result_label(&item.file_name, item.alias.as_deref());
                let icon = self.icons.get(&item.file_path).cloned();
                div()
                    .id(("result", index))
                    .relative()
                    .overflow_hidden()
                    .h(px(SEARCH_ROW_HEIGHT))
                    .w_full()
                    .flex()
                    .items_center()
                    .when(index == self.results.selected, |row| row.bg(selected_color))
                    .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                        if this.pointer != Some(event.position) {
                            this.pointer = Some(event.position);
                            this.results.selected = index;
                            cx.notify();
                        }
                    }))
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
                            .w(px(if icon.is_some() { 54. } else { 20. }))
                            .h_full()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .children(icon.map(|icon| img(icon).size(px(34.)))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .pr(px(if self.hovered == Some(index) {
                                80.
                            } else {
                                10.
                            }))
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
                                            .text_size(px(14.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(foreground)
                                            .truncate()
                                            .child(display_name.to_owned()),
                                    )
                                    .child(
                                        div()
                                            .flex_shrink_0()
                                            .rounded_full()
                                            .px(px(6.))
                                            .text_size(px(10.))
                                            .line_height(px(16.))
                                            .bg(rgb(if is_app { 0x3b82f6 } else { 0x10b981 }))
                                            .text_color(rgb(0xffffff))
                                            .opacity(0.8)
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
                                    .text_size(px(12.))
                                    .text_color(secondary)
                                    .truncate()
                                    .child(item.path.clone()),
                            ),
                    )
                    .when(self.hovered == Some(index), |row| {
                        let width = if cfg!(target_os = "windows") {
                            76.
                        } else {
                            38.
                        };
                        row.child(
                            div()
                                .absolute()
                                .top_0()
                                .h_full()
                                .w(px(width))
                                .flex()
                                .items_center()
                                .justify_end()
                                .when(cfg!(target_os = "windows"), |bar| {
                                    bar.child(
                                        Button::new(("open-admin", index))
                                            .text()
                                            .w(px(38.))
                                            .h_full()
                                            .p_0()
                                            .rounded_none()
                                            .child(
                                                Icon::default()
                                                    .path("search/admin.svg")
                                                    .size(px(20.)),
                                            )
                                            .tooltip(if chinese {
                                                "管理员打开"
                                            } else {
                                                "Open as administrator"
                                            })
                                            .disabled(
                                                self.opening.is_some() || self.results.replacing,
                                            )
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                cx.stop_propagation();
                                                this.results.selected = index;
                                                this.open(false, true, window, cx);
                                            })),
                                    )
                                })
                                .child(
                                    Button::new(("open-folder", index))
                                        .text()
                                        .w(px(38.))
                                        .h_full()
                                        .p_0()
                                        .rounded_none()
                                        .child(
                                            Icon::default().path("search/folder.svg").size(px(20.)),
                                        )
                                        .tooltip(if chinese {
                                            "打开目录"
                                        } else {
                                            "Open folder"
                                        })
                                        .disabled(self.opening.is_some() || self.results.replacing)
                                        .on_click(cx.listener(move |this, _, window, cx| {
                                            cx.stop_propagation();
                                            this.results.selected = index;
                                            this.open(true, false, window, cx);
                                        })),
                                )
                                .with_animation(
                                    ("reveal-actions", self.hover_generation),
                                    Animation::new(Duration::from_millis(300)),
                                    move |bar, progress| {
                                        let eased = 1. - (1. - progress).powi(3);
                                        bar.right(px(-width * (1. - eased)))
                                    },
                                ),
                        )
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.results.selected = index;
                        this.open(false, false, window, cx);
                    }))
                    .into_any_element()
            })
            .collect()
    }
}
impl Render for SearchView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.schedule_resize_after_render(window, cx);
        let chinese = rotor_common::i18n::language_for_config(&self.services.settings()) == "zh-CN";
        let weak = cx.weak_entity();
        let count = self.results.items.len();
        let dark = cx.theme().is_dark();
        let action_error = if self.ai_mode {
            !self.chat_error.is_empty()
        } else {
            !self.message.is_empty()
        };
        let indexing = !self.ai_mode
            && matches!(
                self.index_state,
                IndexState::Unbuild
                    | IndexState::Building
                    | IndexState::Loading
                    | IndexState::Released
            )
            && !action_error;
        div()
            .id("searcher")
            .key_context("RotorSearch")
            .on_action(cx.listener(Self::on_toggle_ai))
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .text_sm()
            .bg(rgb(if dark { 0x121212 } else { 0xffffff }))
            .text_color(rgb(if dark { 0xf6f6f6 } else { 0x121212 }))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.pointer = Some(window.mouse_position());
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
                    "up" if !this.ai_mode => {
                        this.results.selected = this.results.selected.saturating_sub(1);
                        cx.stop_propagation();
                    }
                    "down" if !this.ai_mode => {
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
                    .relative()
                    .mx(px(12.))
                    .h(px(SEARCH_HEADER_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .size(px(24.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                Icon::default()
                                    .path(if self.ai_mode {
                                        "search/ai.svg"
                                    } else {
                                        "search/search.svg"
                                    })
                                    .size(px(if self.ai_mode { 20. } else { 24. }))
                                    .text_color(rgb(if dark { 0xcccccc } else { 0x666666 })),
                            ),
                    )
                    .child(
                        Input::new(&self.input)
                            .appearance(false)
                            .bordered(false)
                            .focus_bordered(false)
                            .h_full()
                            .flex_1()
                            .min_w_0()
                            .p_0()
                            .text_size(px(16.))
                            .aria_label(match (self.ai_mode, chinese) {
                                (true, true) => "向 AI 提问",
                                (true, false) => "Ask AI",
                                (false, true) => "搜索文件",
                                (false, false) => "Search files",
                            }),
                    )
                    .when(self.input.read(cx).value().is_empty(), |header| {
                        header.child(
                            div()
                                .absolute()
                                .left(px(32.))
                                .top_0()
                                .h_full()
                                .flex()
                                .items_center()
                                .text_size(px(16.))
                                .text_color(rgb(if dark { 0x666666 } else { 0x999999 }))
                                .child(match (self.ai_mode, chinese) {
                                    (true, true) => "输入问题，回车发送…",
                                    (true, false) => "Ask anything, press Enter…",
                                    (false, true) => "搜索文件，Tab 切换 AI…",
                                    (false, false) => "Search files · Tab for AI…",
                                }),
                        )
                    })
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .left_0()
                            .w_full()
                            .h(px(2.))
                            .flex()
                            .justify_center()
                            .child(
                                div()
                                    .h_full()
                                    .w_full()
                                    .rounded_full()
                                    .bg(rgb(if action_error { 0xff4d4f } else { 0x54a4db }))
                                    .map(|line| {
                                        if !indexing {
                                            return line.into_any_element();
                                        }
                                        line.with_animation(
                                            "indexing",
                                            Animation::new(Duration::from_millis(1150)).repeat(),
                                            |line, progress| {
                                                let pulse = (1.
                                                    - (progress * std::f32::consts::TAU).cos())
                                                    / 2.;
                                                line.w(relative(0.38 + 0.62 * pulse))
                                            },
                                        )
                                        .into_any_element()
                                    }),
                            ),
                    ),
            )
            .when(self.ai_mode, |view| view.child(self.render_chat(cx)))
            .when(!self.ai_mode, |view| {
                view.child(
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
                        .when(count > 0, |list| {
                            list.child(Scrollbar::vertical(&self.scroll))
                        }),
                )
            })
            .into_any_element()
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
fn result_label<'a>(name: &'a str, alias: Option<&'a str>) -> (&'a str, bool) {
    let is_app = name.rsplit_once('.').is_some_and(|(_, extension)| {
        ["exe", "lnk", "app"]
            .iter()
            .any(|app| extension.eq_ignore_ascii_case(app))
    });
    let title = alias.filter(|alias| !alias.is_empty()).unwrap_or(name);
    let label = if is_app {
        title
            .rsplit_once('.')
            .filter(|(stem, _)| !stem.is_empty())
            .map_or(title, |(stem, _)| stem)
    } else {
        title
    };
    (label, is_app)
}

fn search_height(count: usize) -> f32 {
    SEARCH_HEADER_HEIGHT + SEARCH_ROW_HEIGHT * count.min(7) as f32
}

#[cfg(test)]
mod tests {
    use super::{result_label, search_height};

    #[test]
    fn empty_search_collapses_and_results_grow_to_seven_rows() {
        assert_eq!(search_height(0), 50.);
        assert_eq!(search_height(1), 110.);
        assert_eq!(search_height(7), 470.);
        assert_eq!(search_height(100), 470.);
    }

    #[test]
    fn labels_prefer_aliases_but_classify_by_original_extension() {
        assert_eq!(result_label("Editor.EXE", None), ("Editor", true));
        assert_eq!(
            result_label("Editor.lnk", Some("编辑器.lnk")),
            ("编辑器", true)
        );
        assert_eq!(result_label("Editor.exe", Some("编辑器")), ("编辑器", true));
        assert_eq!(
            result_label("notes.txt", Some("笔记.txt")),
            ("笔记.txt", false)
        );
        assert_eq!(result_label("notes.txt", Some("")), ("notes.txt", false));
    }
}
