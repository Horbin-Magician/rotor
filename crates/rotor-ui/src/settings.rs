use gpui_kit::{
    component::{
        ActiveTheme, Disableable, Selectable,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
    },
    prelude::*,
    *,
};
use rotor_common::Config;
use rotor_runtime::{IndexState, OperationId, RuntimeEvent, SearchIndexStatus, Services};
use std::sync::Arc;
mod action_change;
mod actions;
mod automatic;
mod autosave;
mod overview;
mod updates;

pub fn settings_title(config: &Config) -> &'static str {
    if rotor_common::native_app::PRODUCTION {
        text(config, "Rotor 设置", "Rotor Settings")
    } else {
        text(
            config,
            "Rotor 设置（开发版）",
            "Rotor Settings (Development)",
        )
    }
}
fn text(config: &Config, zh: &'static str, en: &'static str) -> &'static str {
    if rotor_common::i18n::language_for_config(config) == "zh-CN" {
        zh
    } else {
        en
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Overview,
    General,
    Search,
    Pin,
    Translation,
    Shortcuts,
    Quick,
    Updates,
}
struct Field {
    key: &'static str,
    section: Section,
    label: (&'static str, &'static str),
    state: Entity<InputState>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum CloseTarget {
    Window,
    Application,
}
pub struct SettingsView {
    update: Arc<rotor_runtime::UpdateSnapshot>,
    config: Config,
    services: Arc<Services>,
    section: Section,
    fields: Vec<Field>,
    excluded: Entity<TextareaState>,
    index_state: IndexState,
    index_status: Option<SearchIndexStatus>,
    index_request: Option<OperationId>,
    pending: Option<OperationId>,
    autosave: autosave::FieldSaves,
    _field_observers: Vec<Subscription>,
    composition_check: Option<Task<()>>,
    close_request: Option<CloseTarget>,
    last_close_target: CloseTarget,
    manual_failed: bool,
    choosing_path: bool,
    message: String,
    actions: Vec<actions::ActionFields>,
    editing_action: Option<String>,
    recording: Option<actions::Recording>,
    pending_keys: Vec<String>,
    pending_run: Option<OperationId>,
    focus: FocusHandle,
    _activation: Subscription,
    overview: Option<rotor_runtime::Overview>,
    overview_request: Option<OperationId>,
    startup_request: Option<OperationId>,
}
impl SettingsView {
    pub fn show_message(&mut self, message: String, cx: &mut Context<Self>) {
        self.message = message;
        cx.notify();
    }
    pub fn new(
        config: Config,
        services: Arc<Services>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let definitions = [
            (
                "shortcut_search",
                Section::Shortcuts,
                ("搜索快捷键", "Search shortcut"),
                false,
            ),
            (
                "shortcut_screenshot",
                Section::Shortcuts,
                ("截图快捷键", "Screenshot shortcut"),
                false,
            ),
            (
                "shortcut_translate_select",
                Section::Shortcuts,
                ("划词翻译快捷键", "Selection translation shortcut"),
                false,
            ),
            (
                "shortcut_translate_input",
                Section::Shortcuts,
                ("输入翻译快捷键", "Input translation shortcut"),
                false,
            ),
            (
                "shortcut_pinwin_save",
                Section::Shortcuts,
                ("贴图保存", "Pin save"),
                false,
            ),
            (
                "shortcut_pinwin_copy",
                Section::Shortcuts,
                ("贴图复制", "Pin copy"),
                false,
            ),
            (
                "shortcut_pinwin_close",
                Section::Shortcuts,
                ("贴图关闭", "Pin close"),
                false,
            ),
            (
                "shortcut_pinwin_hide",
                Section::Shortcuts,
                ("贴图隐藏", "Pin hide"),
                false,
            ),
            (
                "save_path",
                Section::Pin,
                ("默认保存目录", "Default save directory"),
                false,
            ),
            (
                "translator_deepseek_api_key",
                Section::Translation,
                ("DeepSeek API 密钥", "DeepSeek API key"),
                true,
            ),
            (
                "translator_deepseek_model",
                Section::Translation,
                ("DeepSeek 模型", "DeepSeek model"),
                false,
            ),
            (
                "translator_custom_url",
                Section::Translation,
                ("自定义 URL 模板", "Custom URL template"),
                false,
            ),
            (
                "translator_custom_key",
                Section::Translation,
                ("自定义 API 密钥", "Custom API key"),
                true,
            ),
        ];
        let fields: Vec<Field> = definitions
            .into_iter()
            .map(|(key, section, label, secret)| {
                let value = config.get(key).cloned().unwrap_or_default();
                Field {
                    key,
                    section,
                    label,
                    state: cx.new(|cx| {
                        InputState::new(window, cx)
                            .default_value(value)
                            .masked(secret)
                    }),
                }
            })
            .collect();
        let excluded = cx.new(|cx| {
            TextareaState::new(window, cx).default_value(
                config
                    .get("search_excluded_dirs")
                    .cloned()
                    .unwrap_or_default(),
            )
        });
        let autosave = autosave::FieldSaves::new(
            fields
                .iter()
                .map(|field| (field.key, field.state.read(cx).value().to_string()))
                .chain(std::iter::once((
                    "search_excluded_dirs",
                    excluded.read(cx).value().to_string(),
                )))
                .chain(
                    [
                        "language",
                        "theme",
                        "if_ask_save_path",
                        "if_auto_change_save_path",
                        "zoom_delta",
                        "translator_engine",
                        "translator_target_lang",
                    ]
                    .into_iter()
                    .map(|key| (key, config.get(key).cloned().unwrap_or_default())),
                ),
        );
        let mut field_observers = Vec::new();
        for field in &fields {
            let key = field.key;
            field_observers.push(cx.observe_in(
                &field.state,
                window,
                move |this, _, window, cx| this.observe_field(key, false, window, cx),
            ));
            field_observers.push(cx.subscribe_in(
                &field.state,
                window,
                move |this, _, event, window, cx| match event {
                    InputEvent::PressEnter { shift: false, .. } => {
                        this.observe_field(key, true, window, cx)
                    }
                    InputEvent::Blur => this.observe_field(key, false, window, cx),
                    _ => {}
                },
            ));
        }
        field_observers.push(cx.observe_in(&excluded, window, |this, _, window, cx| {
            this.observe_field("search_excluded_dirs", false, window, cx)
        }));
        let action_result = services.quick_actions();
        let overview_request = services.request_overview().ok();
        let index_request = services.request_index_status().ok();
        let message = action_result.as_ref().err().cloned().unwrap_or_default();
        let actions = action_result
            .unwrap_or_default()
            .into_iter()
            .map(|action| actions::ActionFields::new(action, window, cx))
            .collect();
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() && this.recording.take().is_some() {
                this.services.set_shortcut_recording(false);
                if this.manual_failed || this.autosave.has_failures() {
                    this.saved_feedback();
                } else {
                    this.message.clear();
                }
                cx.notify();
            }
            if !window.is_window_active() {
                this.observe_all_fields(false, window, cx);
            }
        });
        Self {
            update: services.update_snapshot(),
            config,
            services,
            section: Section::Overview,
            fields,
            excluded,
            index_state: IndexState::Unavailable,
            index_status: None,
            index_request,
            pending: None,
            autosave,
            _field_observers: field_observers,
            composition_check: None,
            close_request: None,
            last_close_target: CloseTarget::Window,
            manual_failed: false,
            choosing_path: false,
            message,
            actions,
            editing_action: None,
            recording: None,
            pending_keys: Vec::new(),
            pending_run: None,
            focus: cx.focus_handle(),
            _activation: activation,
            overview: None,
            overview_request,
            startup_request: None,
        }
    }
    fn t(&self, zh: &'static str, en: &'static str) -> &'static str {
        text(&self.config, zh, en)
    }
    pub fn handle_event(
        &mut self,
        event: RuntimeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            RuntimeEvent::Update(snapshot) if snapshot.revision >= self.update.revision => {
                self.update = snapshot;
            }
            RuntimeEvent::Overview { id, result } if self.overview_request == Some(id) => {
                self.overview_request = None;
                match result {
                    Ok(overview) => self.overview = Some(overview),
                    Err(error) => self.message = error,
                }
            }
            RuntimeEvent::StartupChanged { id, result } if self.startup_request == Some(id) => {
                self.startup_request = None;
                self.message = match result {
                    Ok(_) => self.t("启动项已更新", "Startup entry updated").into(),
                    Err(error) => error,
                };
                self.refresh_overview(cx);
            }
            RuntimeEvent::SettingsSaved { id, result } => {
                let own = self.pending == Some(id);
                let receipt = self.autosave.finish(id, result.is_ok());
                if own {
                    self.pending = None;
                }
                match result {
                    Ok(config) => {
                        self.config = config;
                        self.apply_saved_fields(receipt.saved, window, cx);
                        if own {
                            if self.pending_keys.iter().any(|key| key == "quick_actions") {
                                self.manual_failed = false;
                            }
                            self.sync_saved_fields(window, cx);
                        }
                        if own || receipt.current {
                            self.saved_feedback();
                        }
                    }
                    Err(error) if own || receipt.current => {
                        if own && self.pending_keys.iter().any(|key| key == "quick_actions") {
                            self.manual_failed = true;
                        }
                        self.message = error;
                    }
                    Err(_) => {}
                }
                if own {
                    self.pending_keys.clear();
                }
                self.settle_close(window, cx);
            }
            RuntimeEvent::IndexState(state) => self.index_state = state,
            RuntimeEvent::QuickFinished { id, result, .. } if self.pending_run == Some(id) => {
                self.pending_run = None;
                self.message = match result {
                    Ok(()) => self.t("命令已启动", "Command launched").into(),
                    Err(error) => error,
                };
            }
            RuntimeEvent::IndexStatus { id, result } if self.index_request == Some(id) => {
                self.index_request = None;
                match result {
                    Ok(status) => {
                        self.index_state = status.state;
                        self.index_status = Some(status);
                    }
                    Err(error) => self.message = error,
                }
            }
            _ => return,
        }
        cx.notify();
    }
    fn save(&mut self, changes: Vec<(String, String)>, cx: &mut Context<Self>) {
        if self.pending.is_some() || self.close_request.is_some() {
            return;
        }
        let keys = changes.iter().map(|(key, _)| key.clone()).collect();
        match self.services.save_settings(changes.clone()) {
            Ok(id) => {
                for (key, value) in &changes {
                    self.autosave.accepted(key, value, id);
                }
                self.pending = Some(id);
                self.pending_keys = keys;
                self.message = self.t("正在保存…", "Saving…").into();
            }
            Err(error) => {
                for (key, value) in &changes {
                    self.autosave.rejected_value(key, value);
                }
                self.message = error;
            }
        }
        cx.notify();
    }
    fn save_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.has_marked_fields(window, cx) {
            self.message = self
                .t(
                    "请先确认中文组字，再保存",
                    "Confirm the composed text before saving",
                )
                .into();
            cx.notify();
            return;
        }
        if self.section == Section::Quick {
            self.save_actions(cx);
            return;
        }
        let changes = if self.section == Section::Search {
            vec![(
                "search_excluded_dirs".into(),
                self.excluded.read(cx).value().to_string(),
            )]
        } else {
            self.fields
                .iter()
                .filter(|field| self.field_visible(field))
                .map(|field| (field.key.into(), field.state.read(cx).value().to_string()))
                .collect()
        };
        self.save(changes, cx);
    }

    fn field_visible(&self, field: &Field) -> bool {
        if field.section != self.section {
            return false;
        }
        let engine = self
            .config
            .get("translator_engine")
            .map(String::as_str)
            .unwrap_or("google");
        match field.key {
            "translator_deepseek_api_key" | "translator_deepseek_model" => engine == "deepseek",
            "translator_custom_url" | "translator_custom_key" => engine == "custom",
            _ => true,
        }
    }

    fn choose_save_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.choosing_path || self.pending.is_some() || self.close_request.is_some() {
            return;
        }
        self.choosing_path = true;
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(self.t("选择保存目录", "Choose save directory").into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = receiver.await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.choosing_path = false;
                match result {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.first() {
                            if let Some(path) = path.to_str() {
                                if let Some(field) =
                                    this.fields.iter().find(|field| field.key == "save_path")
                                {
                                    field.state.update(cx, |input, cx| {
                                        input.set_value(path.to_owned(), window, cx)
                                    });
                                }
                            } else {
                                this.message = this
                                    .t(
                                        "目录名称不是有效的 Unicode",
                                        "Directory name is not valid Unicode",
                                    )
                                    .into();
                            }
                        }
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => this.message = error.to_string(),
                    Err(error) => this.message = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    fn choices(
        &self,
        key: &'static str,
        label: (&'static str, &'static str),
        options: &[(&'static str, &'static str, &'static str)],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        crate::visual::card(cx)
            .flex()
            .flex_col()
            .gap_2()
            .child(self.t(label.0, label.1))
            .when(self.autosave.failed(key), |row| {
                row.child(
                    crate::visual::caption(self.t("未保存", "Not saved"), cx)
                        .text_color(cx.theme().danger),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .children(options.iter().enumerate().map(|(index, &(value, zh, en))| {
                        crate::visual::choice(
                            Button::new((key, index)).label(self.t(zh, en)),
                            self.config.get(key).is_some_and(|current| current == value),
                            cx,
                        )
                        .disabled(self.controls_locked())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.save(vec![(key.into(), value.into())], cx)
                        }))
                    })),
            )
    }
}
impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let compact = window.viewport_size().width < px(760.);
        let can_save = matches!(self.section, Section::Quick | Section::Search)
            || self.fields.iter().any(|field| self.field_visible(field));
        let sections = [
            (
                "overview",
                Section::Overview,
                "概览",
                "Overview",
                "运行状态与常用入口",
                "Status and everyday tools",
            ),
            (
                "general",
                Section::General,
                "通用",
                "General",
                "让 Rotor 符合你的使用习惯",
                "Make Rotor feel at home",
            ),
            (
                "search",
                Section::Search,
                "搜索",
                "Search",
                "管理文件索引与排除目录",
                "Manage your file index and exclusions",
            ),
            (
                "pin",
                Section::Pin,
                "贴图",
                "Pinned images",
                "保存位置与缩放行为",
                "Save locations and zoom behavior",
            ),
            (
                "translation",
                Section::Translation,
                "翻译",
                "Translation",
                "选择翻译引擎与目标语言",
                "Choose an engine and target language",
            ),
            (
                "shortcuts",
                Section::Shortcuts,
                "快捷键",
                "Shortcuts",
                "录制后自动保存；手动输入按 Enter 确认",
                "Recording saves automatically; Enter confirms typed shortcuts",
            ),
            (
                "quick",
                Section::Quick,
                "快捷操作",
                "Quick actions",
                "通过快捷键运行常用命令",
                "Run your everyday commands with a shortcut",
            ),
            (
                "updates",
                Section::Updates,
                "更新",
                "Updates",
                "版本信息与下载进度",
                "Version details and download progress",
            ),
        ];
        let current = sections.iter().find(|item| item.1 == self.section).unwrap();
        let mut content = div().flex().flex_col().min_w_0().gap_4();
        match self.section {
            Section::Updates => {
                content = content.child(self.update_panel(cx));
            }
            Section::Overview => {
                content = content.child(self.overview_panel(cx));
            }
            Section::General => {
                content = content
                    .child(self.choices(
                        "language",
                        ("语言", "Language"),
                        &[
                            ("0", "跟随系统", "System"),
                            ("1", "简体中文", "简体中文"),
                            ("2", "English", "English"),
                        ],
                        cx,
                    ))
                    .child(self.choices(
                        "theme",
                        ("主题", "Theme"),
                        &[
                            ("0", "跟随系统", "System"),
                            ("1", "浅色", "Light"),
                            ("2", "深色", "Dark"),
                        ],
                        cx,
                    ));
            }
            Section::Search => {
                content = content
                    .child(self.index_panel(cx))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                Button::new("refresh-index")
                                    .label(self.t("刷新状态", "Refresh status"))
                                    .disabled(
                                        self.index_request.is_some() || self.controls_locked(),
                                    )
                                    .on_click(cx.listener(|this, _, _, cx| this.refresh_index(cx))),
                            )
                            .child(
                                Button::new("rebuild-index")
                                    .label(self.t("重建索引", "Rebuild index"))
                                    .disabled(self.controls_locked())
                                    .on_click(
                                        cx.listener(|this, _, _, _| this.services.rebuild_search()),
                                    ),
                            )
                            .child(
                                Button::new("release-index")
                                    .label(self.t("释放索引", "Release index"))
                                    .disabled(self.controls_locked())
                                    .on_click(
                                        cx.listener(|this, _, _, _| this.services.release_search()),
                                    ),
                            ),
                    )
                    .child(self.t(
                        "排除目录（每行一个名称或路径）",
                        "Excluded directories (one name or path per line)",
                    ))
                    .child(
                        Textarea::new(&self.excluded)
                            .h(px(150.))
                            .disabled(self.controls_locked()),
                    );
            }
            Section::Pin => {
                content = content
                    .child(self.choices(
                        "if_ask_save_path",
                        ("保存时选择路径", "Choose a path when saving"),
                        &[("true", "开启", "On"), ("false", "关闭", "Off")],
                        cx,
                    ))
                    .child(self.choices(
                        "if_auto_change_save_path",
                        ("记住上次保存目录", "Remember last save directory"),
                        &[("true", "开启", "On"), ("false", "关闭", "Off")],
                        cx,
                    ))
                    .child(self.choices(
                        "zoom_delta",
                        ("滚轮缩放步长", "Scroll zoom step"),
                        &[
                            ("1", "1", "1"),
                            ("2", "2", "2"),
                            ("3", "3", "3"),
                            ("4", "4", "4"),
                            ("5", "5", "5"),
                            ("6", "6", "6"),
                            ("7", "7", "7"),
                            ("8", "8", "8"),
                            ("9", "9", "9"),
                            ("10", "10", "10"),
                        ],
                        cx,
                    ));
            }
            Section::Translation => {
                content = content
                    .child(self.choices(
                        "translator_engine",
                        ("翻译引擎", "Translation engine"),
                        &[
                            ("google", "Google", "Google"),
                            ("deepseek", "DeepSeek", "DeepSeek"),
                            ("custom", "自定义", "Custom"),
                        ],
                        cx,
                    ))
                    .child(self.choices(
                        "translator_target_lang",
                        ("目标语言", "Target language"),
                        &[
                            ("auto", "自动", "Automatic"),
                            ("zh-CN", "简体中文", "Chinese"),
                            ("en", "英语", "English"),
                            ("ja", "日语", "Japanese"),
                            ("ko", "韩语", "Korean"),
                        ],
                        cx,
                    ));
            }
            Section::Shortcuts => {
                content = content.child(if self.services.uses_development_shortcuts() {
                    self.t(
                        "开发模式会为全局快捷键加入 Alt；贴图按键不变。",
                        "Development mode adds Alt to global shortcuts; pin keys are unchanged.",
                    )
                } else {
                    self.t(
                        "全局快捷键使用下面保存的组合。",
                        "Global shortcuts use the combinations saved below.",
                    )
                });
            }
            Section::Quick => {
                content = content.child(self.action_editor(cx));
            }
        }
        if self.fields.iter().any(|field| self.field_visible(field)) {
            content = content.child(
                crate::visual::card(cx).gap_3().children(
                    self.fields
                        .iter()
                        .filter(|field| self.field_visible(field))
                        .map(|field| {
                            let key = field.key;
                            div()
                                .flex()
                                .flex_col()
                                .min_w_0()
                                .when(self.section == Section::Shortcuts, |row| {
                                    row.flex_row().items_center()
                                })
                                .gap_2()
                                .child(
                                    div()
                                        .when(self.section == Section::Shortcuts, |label| {
                                            label
                                                .w(px(if compact { 110. } else { 165. }))
                                                .flex_shrink_0()
                                        })
                                        .child(self.t(field.label.0, field.label.1))
                                        .when(self.autosave.failed(key), |label| {
                                            label.child(
                                                crate::visual::caption(
                                                    self.t("未保存", "Not saved"),
                                                    cx,
                                                )
                                                .text_color(cx.theme().danger),
                                            )
                                        }),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_1()
                                        .min_w_0()
                                        .gap_2()
                                        .child(
                                            Input::new(&field.state)
                                                .aria_label(self.t(field.label.0, field.label.1))
                                                .disabled(
                                                    self.controls_locked() || self.choosing_path,
                                                ),
                                        )
                                        .when(self.section == Section::Shortcuts, |row| {
                                            row.child(
                                                Button::new((key, 0usize))
                                                    .label(self.t("录制", "Record"))
                                                    .tooltip(
                                                        self.t("录制快捷键", "Record shortcut"),
                                                    )
                                                    .disabled(self.controls_locked())
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            this.start_recording(
                                                                actions::Recording::Setting(key),
                                                                window,
                                                                cx,
                                                            )
                                                        },
                                                    )),
                                            )
                                        }),
                                )
                        }),
                ),
            );
        }
        if self.section == Section::Pin {
            content = content.child(
                Button::new("choose-save-directory")
                    .label(self.t("浏览目录…", "Browse…"))
                    .disabled(self.controls_locked() || self.choosing_path)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.choose_save_directory(window, cx)),
                    ),
            );
        }
        div()
            .id("settings")
            .track_focus(&self.focus)
            .flex()
            .size_full()
            .text_sm()
            .text_color(cx.theme().foreground)
            .bg(cx.theme().muted)
            .capture_any_mouse_down(cx.listener(|this, _, _, cx| {
                if this.recording.take().is_some() {
                    this.services.set_shortcut_recording(false);
                    cx.notify();
                }
            }))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.record_key(event, window, cx)
            }))
            .child(
                div()
                    .id("settings-navigation")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .w(px(if compact { 150. } else { 184. }))
                    .h_full()
                    .flex_shrink_0()
                    .p_3()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .overflow_y_scroll()
                    .child(
                        div()
                            .px_3()
                            .py_4()
                            .text_2xl()
                            .font_weight(FontWeight::BOLD)
                            .child("Rotor"),
                    )
                    .children(sections.into_iter().map(|(id, section, zh, en, _, _)| {
                        crate::visual::choice(
                            Button::new(id)
                                .ghost()
                                .w_full()
                                .justify_start()
                            .label(self.t(zh, en)),
                            self.section == section,
                            cx,
                        )
                        .disabled(self.close_request.is_some())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.section = section;
                            cx.notify();
                        }))
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .gap_4()
                    .p(if compact { px(16.) } else { px(24.) })
                    .child(crate::visual::heading(
                        self.t(current.2, current.3),
                        self.t(current.4, current.5),
                        cx,
                    ))
                    .child(
                        div()
                            .id("settings-content")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(content),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .flex_shrink_0()
                            .when(!self.message.is_empty(), |footer| {
                                footer.child(
                                    div()
                                        .id("settings-feedback")
                                        .max_h(px(100.))
                                        .overflow_y_scroll()
                                        .child(self.message.clone()),
                                )
                            })
                            .when(self.recording.is_some(), |root| {
                                root.child(
                                    Button::new("cancel-recording")
                                        .label(self.t("取消录制", "Cancel recording"))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.recording = None;
                                            this.services.set_shortcut_recording(false);
                                            this.message.clear();
                                            cx.notify();
                                        })),
                                )
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .child(crate::visual::caption(
                                        self.t("Rotor · 随时待命", "Rotor · Ready when you are"),
                                        cx,
                                    ))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .max_w_full()
                                            .gap_2()
                                            .when(can_save, |row| {
                                                row.child(
                                                    Button::new("save-fields")
                                                        .primary()
                                                        .label(self.t("保存更改", "Save changes"))
                                                        .disabled(self.controls_locked())
                                                        .on_click(cx.listener(|this, _, window, cx| {
                                                            this.save_fields(window, cx)
                                                        })),
                                                )
                                            })
                                            .when(self.manual_failed || self.autosave.has_failures(), |row| row.child(
                                                Button::new("discard-settings-close")
                                                    .label(if self.last_close_target == CloseTarget::Application {
                                                        self.t("放弃未保存并退出", "Discard unsaved changes and quit")
                                                    } else { self.t("放弃未保存并关闭", "Discard unsaved changes and close") })
                                                    .disabled(self.pending.is_some() || self.autosave.has_pending())
                                                    .on_click(cx.listener(|this, _, window, cx| this.discard_and_close(window, cx)))))
                                            .child(
                                                Button::new("close")
                                                    .label(self.t("关闭", "Close"))
                                                    .disabled(self.close_request.is_some())
                                                    .on_click(cx.listener(|this, _, window, cx| this.request_close(window, cx))),
                                            ),
                                    ),
                            ),
                    ),
            )
    }
}

impl Drop for SettingsView {
    fn drop(&mut self) {
        self.services.set_shortcut_recording(false);
    }
}
