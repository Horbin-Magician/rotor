use gpui_kit::{
    component::{
        ActiveTheme, Disableable, Icon, IconName, WindowExt,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
        notification::Notification,
        scroll::ScrollableElement,
        slider::{Slider, SliderEvent, SliderState},
    },
    prelude::*,
    *,
};
use rotor_common::Config;
use rotor_runtime::{IndexState, OperationId, RuntimeEvent, SearchIndexStatus, Services};
use std::sync::Arc;
mod action_change;
mod actions;
mod ai_provider;
mod appearance;
mod automatic;
mod autosave;
mod controls;
mod general;
mod logo;
mod motion;
mod overview;
#[cfg(test)]
mod tests;
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
    AiProvider,
    Search,
    Pin,
    Translation,
    Quick,
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
    ai_test: ai_provider::ConfigurationTest,
    update: Arc<rotor_runtime::UpdateSnapshot>,
    config: Config,
    services: Arc<Services>,
    section: Section,
    fields: Vec<Field>,
    excluded: Entity<TextareaState>,
    zoom_step: Entity<SliderState>,
    index_state: IndexState,
    index_status: Option<SearchIndexStatus>,
    index_request: Option<OperationId>,
    pending: Option<OperationId>,
    autosave: autosave::FieldSaves,
    _field_observers: Vec<Subscription>,
    composition_check: Option<Task<()>>,
    action_save: Option<Task<()>>,
    close_request: Option<CloseTarget>,
    last_close_target: CloseTarget,
    manual_failed: bool,
    choosing_path: bool,
    message: String,
    displayed_message: String,
    actions: Vec<actions::ActionFields>,
    editing_action: Option<String>,
    recording: Option<actions::Recording>,
    pending_keys: Vec<String>,
    pending_run: Option<OperationId>,
    focus: FocusHandle,
    _activation: Subscription,
    overview: Option<rotor_runtime::Overview>,
    overview_request: Option<OperationId>,
    overview_refresh_started: Option<std::time::Instant>,
    startup_request: Option<OperationId>,
    logo: logo::Logo,
    navigation_hover: Option<Section>,
    navigation_highlights: [motion::Transition; 7],
    navigation_indicator: motion::Transition,
    logo_hover: bool,
    logo_glow: motion::Transition,
}
impl SettingsView {
    pub fn show_message(&mut self, message: String, cx: &mut Context<Self>) {
        self.message = message;
        cx.notify();
    }
    pub fn new(
        mut config: Config,
        services: Arc<Services>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        rotor_common::ai_provider::apply_defaults(&mut config);
        let definitions = [
            (
                "shortcut_search",
                Section::General,
                ("搜索", "Search"),
                false,
            ),
            (
                "shortcut_screenshot",
                Section::General,
                ("截图", "Screenshot"),
                false,
            ),
            (
                "shortcut_translate_select",
                Section::General,
                ("划词翻译", "Selection translation"),
                false,
            ),
            (
                "shortcut_translate_input",
                Section::General,
                ("输入翻译", "Input translation"),
                false,
            ),
            (
                "shortcut_pinwin_save",
                Section::Pin,
                ("贴图保存", "Pin save"),
                false,
            ),
            (
                "shortcut_pinwin_copy",
                Section::Pin,
                ("贴图复制", "Pin copy"),
                false,
            ),
            (
                "shortcut_pinwin_close",
                Section::Pin,
                ("贴图关闭", "Pin close"),
                false,
            ),
            (
                "shortcut_pinwin_hide",
                Section::Pin,
                ("贴图最小化", "Pin minimize"),
                false,
            ),
            (
                "save_path",
                Section::Pin,
                ("默认保存目录", "Default save directory"),
                false,
            ),
            (
                "ai_deepseek_base_url",
                Section::AiProvider,
                ("API 基础地址", "API base URL"),
                false,
            ),
            (
                "ai_deepseek_api_key",
                Section::AiProvider,
                ("API 密钥", "API key"),
                true,
            ),
            (
                "ai_deepseek_model",
                Section::AiProvider,
                ("模型 ID", "Model ID"),
                false,
            ),
            (
                "ai_deepseek_max_tokens",
                Section::AiProvider,
                ("最大输出 Token 数", "Maximum output tokens"),
                false,
            ),
            (
                "ai_openai_base_url",
                Section::AiProvider,
                ("API 基础地址", "API base URL"),
                false,
            ),
            (
                "ai_openai_api_key",
                Section::AiProvider,
                ("API 密钥", "API key"),
                true,
            ),
            (
                "ai_openai_model",
                Section::AiProvider,
                ("模型 ID", "Model ID"),
                false,
            ),
            (
                "ai_openai_max_tokens",
                Section::AiProvider,
                ("最大输出 Token 数", "Maximum output tokens"),
                false,
            ),
            (
                "ai_anthropic_base_url",
                Section::AiProvider,
                ("API 基础地址", "API base URL"),
                false,
            ),
            (
                "ai_anthropic_api_key",
                Section::AiProvider,
                ("API 密钥", "API key"),
                true,
            ),
            (
                "ai_anthropic_model",
                Section::AiProvider,
                ("模型 ID", "Model ID"),
                false,
            ),
            (
                "ai_anthropic_max_tokens",
                Section::AiProvider,
                ("最大输出 Token 数", "Maximum output tokens"),
                false,
            ),
            (
                "ai_custom_base_url",
                Section::AiProvider,
                ("API 基础地址", "API base URL"),
                false,
            ),
            (
                "ai_custom_api_key",
                Section::AiProvider,
                ("API 密钥", "API key"),
                true,
            ),
            (
                "ai_custom_model",
                Section::AiProvider,
                ("模型 ID", "Model ID"),
                false,
            ),
            (
                "ai_custom_max_tokens",
                Section::AiProvider,
                ("最大输出 Token 数", "Maximum output tokens"),
                false,
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
                        "ai_provider",
                        "ai_custom_protocol",
                        "translator_engine",
                        "translator_target_lang",
                    ]
                    .into_iter()
                    .map(|key| (key, config.get(key).cloned().unwrap_or_default())),
                ),
        );
        let zoom_step = cx.new(|_| {
            SliderState::new().min(1.).max(10.).step(1.).default_value(
                config
                    .get("zoom_delta")
                    .and_then(|value| value.parse::<u8>().ok())
                    .unwrap_or(2)
                    .clamp(1, 10) as f32,
            )
        });
        let mut field_observers =
            vec![cx.subscribe(&zoom_step, |this, _, event, cx| match event {
                SliderEvent::Change(_) => cx.notify(),
                SliderEvent::Release(value) if !this.controls_locked() => {
                    this.save(vec![("zoom_delta".into(), value.to_string())], cx);
                }
                _ => {}
            })];
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
            ai_test: ai_provider::ConfigurationTest::default(),
            update: services.update_snapshot(),
            config,
            services,
            section: Section::Overview,
            fields,
            excluded,
            zoom_step,
            index_state: IndexState::Unavailable,
            index_status: None,
            index_request,
            pending: None,
            autosave,
            _field_observers: field_observers,
            composition_check: None,
            action_save: None,
            close_request: None,
            last_close_target: CloseTarget::Window,
            manual_failed: false,
            choosing_path: false,
            message,
            displayed_message: String::new(),
            actions,
            editing_action: None,
            recording: None,
            pending_keys: Vec::new(),
            pending_run: None,
            focus: cx.focus_handle(),
            _activation: activation,
            overview: None,
            overview_request,
            overview_refresh_started: None,
            startup_request: None,
            logo: logo::Logo::new(),
            navigation_hover: None,
            navigation_highlights: std::array::from_fn(|index| {
                motion::Transition::new(if index == 0 { 1. } else { 0. }, 160)
            }),
            navigation_indicator: motion::Transition::new(0., 240),
            logo_hover: false,
            logo_glow: motion::Transition::new(0., 220),
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
            RuntimeEvent::AiProviderTested { id, result } => {
                self.finish_ai_test(id, result, cx);
            }
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
                    Ok(enabled) => {
                        if let Some(overview) = &mut self.overview {
                            overview.autostart = Ok(enabled);
                        }
                        self.t("启动项已更新", "Startup entry updated").into()
                    }
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
                self.message.clear();
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
    fn field_visible(&self, field: &Field) -> bool {
        if field.section != self.section {
            return false;
        }
        if field.section == Section::AiProvider {
            let provider = self
                .config
                .get("ai_provider")
                .map(String::as_str)
                .unwrap_or("deepseek");
            return field.key.starts_with(&format!("ai_{provider}_"));
        }
        true
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
}
impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.message != self.displayed_message {
            self.displayed_message = self.message.clone();
            if !self.message.is_empty() && !self.manual_failed && !self.autosave.has_failures() {
                let message = self.message.clone();
                cx.defer_in(window, move |_, window, cx| {
                    window.push_notification(Notification::info(message).id::<SettingsView>(), cx);
                });
            }
        }
        self.invalidate_ai_test(cx);
        let compact = window.viewport_size().width < px(760.);
        let logo_size = 50.;
        let logo = self.logo.image(logo_size, window.scale_factor());
        let logo_glow = self.logo.glow();
        let logo_pixels = (logo_size * window.scale_factor()).round().max(1.);
        let glow_padding = logo::glow_padding(logo_pixels as u32) as f32 / logo_pixels * logo_size;
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
                "基础",
                "General",
                "让 Rotor 符合你的使用习惯",
                "Make Rotor feel at home",
            ),
            (
                "ai-provider",
                Section::AiProvider,
                "AI 服务商",
                "AI providers",
                "统一管理 AI 服务、密钥与模型",
                "Manage AI services, credentials and models",
            ),
            (
                "pin",
                Section::Pin,
                "截图",
                "Screenshots",
                "保存位置与缩放行为",
                "Save locations and zoom behavior",
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
                "translation",
                Section::Translation,
                "翻译",
                "Translation",
                "选择翻译引擎与目标语言",
                "Choose an engine and target language",
            ),
            (
                "quick",
                Section::Quick,
                "快捷",
                "Quick actions",
                "通过快捷键运行常用命令",
                "Run your everyday commands with a shortcut",
            ),
        ];
        let now = std::time::Instant::now();
        let reduce_motion = cx.reduce_motion();
        let row_height = if compact { 36. } else { 38. };
        let separator_height = f32::from(window.rem_size()) + 1.;
        let indicator_inset = f32::from(window.rem_size()) * 0.25;
        let mut row_top = 0.;
        let mut indicator_target = 0.;
        let mut highlights = [0.; 7];
        let (glow_opacity, mut animating) =
            self.logo_glow
                .sample(if self.logo_hover { 1. } else { 0. }, now, reduce_motion);
        for (index, (_, section, ..)) in sections.iter().enumerate() {
            if matches!(section, Section::Pin) {
                row_top += separator_height;
            }
            let selected = self.section == *section;
            if selected {
                indicator_target = row_top;
            }
            let highlighted = selected
                || (self.close_request.is_none() && self.navigation_hover == Some(*section));
            let (value, running) = self.navigation_highlights[index].sample(
                if highlighted { 1. } else { 0. },
                now,
                reduce_motion,
            );
            highlights[index] = value;
            animating |= running;
            row_top += row_height;
        }
        let (indicator_top, moving) =
            self.navigation_indicator
                .sample(indicator_target, now, reduce_motion);
        if animating || moving {
            window.request_animation_frame();
        }
        let mut content = div().flex().flex_col().min_w_0().gap(px(6.));
        match self.section {
            Section::Overview => {
                let (rotation, spinning) =
                    motion::refresh_rotation(self.overview_refresh_started, now, reduce_motion);
                if spinning {
                    window.request_animation_frame();
                } else {
                    self.overview_refresh_started = None;
                }
                content = content.child(self.overview_panel(rotation, cx));
            }
            Section::General => {
                content = content.child(self.general_panel(cx));
            }
            Section::AiProvider => {
                content = content
                    .child(appearance::heading(
                        self.t("全局 AI 服务", "Global AI service"),
                        cx,
                    ))
                    .child(self.dropdown(
                        "ai_provider",
                        ("当前服务商", "Active provider"),
                        &[
                            ("deepseek", "DeepSeek", "DeepSeek"),
                            ("openai", "OpenAI", "OpenAI"),
                            ("anthropic", "Claude (Anthropic)", "Claude (Anthropic)"),
                            ("custom", "自定义", "Custom"),
                        ],
                        cx,
                    ));
                if self
                    .config
                    .get("ai_provider")
                    .is_some_and(|value| value == "custom")
                {
                    content = content.child(self.dropdown(
                        "ai_custom_protocol",
                        ("接口协议", "API protocol"),
                        &[
                            ("openai", "OpenAI 兼容", "OpenAI compatible"),
                            ("anthropic", "Anthropic Messages", "Anthropic Messages"),
                        ],
                        cx,
                    ));
                }
            }
            Section::Search => {
                content = content
                    .child(
                        appearance::group(self.t("排除目录", "Excluded directories"), cx).child(
                            appearance::caption(
                                self.t("每行一个名称或路径", "One name or path per line"),
                                cx,
                            )
                            .pl(px(12.)),
                        ),
                    )
                    .child(
                        div().pl(px(12.)).child(
                            Textarea::new(&self.excluded)
                                .text_size(px(13.))
                                .h(px(180.))
                                .disabled(self.controls_locked()),
                        ),
                    );
            }
            Section::Pin => {
                content = content
                    .child(appearance::heading(
                        self.t("保存与缩放", "Saving and zoom"),
                        cx,
                    ))
                    .child(self.toggle(
                        "if_ask_save_path",
                        ("保存时选择路径", "Choose a path when saving"),
                        cx,
                    ))
                    .child(self.toggle(
                        "if_auto_change_save_path",
                        ("记住上次保存目录", "Remember last save directory"),
                        cx,
                    ))
                    .child(self.zoom_step_slider(cx));
            }
            Section::Translation => {
                content = content.child(appearance::heading(
                    self.t("翻译服务", "Translation service"),
                    cx,
                ));
                if self
                    .config
                    .get("translator_engine")
                    .is_some_and(|value| matches!(value.as_str(), "ai" | "deepseek"))
                {
                    content = content.child(appearance::caption(
                        self.t(
                            "AI 翻译的密钥、模型与服务地址在「AI 服务商」中统一设置。",
                            "Configure AI translation credentials, model and endpoint in AI providers.",
                        ),
                        cx,
                    ));
                }
                if self
                    .config
                    .get("translator_engine")
                    .is_some_and(|value| value == "custom")
                {
                    content = content.child(appearance::caption(self.t(
                        "当前仍使用已保存的旧版 URL 翻译引擎；选择 AI 翻译后将使用全局 AI 服务。",
                        "The saved legacy URL engine is still active. Select AI translation to use the global AI service."
                    ), cx));
                }
                content = content
                    .child(self.dropdown(
                        "translator_engine",
                        ("翻译引擎", "Translation engine"),
                        &[
                            ("google", "Google", "Google"),
                            ("ai", "AI 翻译", "AI translation"),
                        ],
                        cx,
                    ))
                    .child(self.dropdown(
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
            Section::Quick => {
                content = content.child(self.action_editor(cx));
            }
        }
        if matches!(self.section, Section::Pin | Section::AiProvider) {
            let mut fields = appearance::group(
                if self.section == Section::Pin {
                    self.t("贴图快捷键", "Pin shortcuts")
                } else {
                    self.t("服务商配置", "Provider configuration")
                },
                cx,
            )
            .mt(px(10.));
            for field in self.fields.iter().filter(|field| self.field_visible(field)) {
                if field.key.starts_with("shortcut_") {
                    fields = fields.child(self.shortcut_field(field, cx));
                } else {
                    let control = div()
                        .flex()
                        .items_center()
                        .min_w_0()
                        .gap_2()
                        .child(
                            div().flex_1().min_w_0().child(
                                Input::new(&field.state)
                                    .text_size(px(13.))
                                    .aria_label(self.t(field.label.0, field.label.1))
                                    .disabled(self.controls_locked() || self.choosing_path),
                            ),
                        )
                        .when(field.key == "save_path", |row| {
                            row.child(
                                Button::new("choose-save-directory")
                                    .label("…")
                                    .accessibility_label(self.t("浏览目录…", "Browse…"))
                                    .tooltip(self.t("浏览目录…", "Browse…"))
                                    .disabled(self.controls_locked() || self.choosing_path)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.choose_save_directory(window, cx)
                                    })),
                            )
                        });
                    let row =
                        appearance::control_row(self.t(field.label.0, field.label.1), control)
                            .when(self.autosave.failed(field.key), |row| {
                                row.child(
                                    appearance::caption(self.t("未保存", "Not saved"), cx)
                                        .text_color(cx.theme().danger),
                                )
                            });
                    if field.key == "save_path" {
                        content = content.child(row);
                    } else {
                        fields = fields.child(row);
                    }
                }
            }
            if self
                .fields
                .iter()
                .any(|field| self.field_visible(field) && field.key != "save_path")
            {
                content = content.child(fields);
            }
        }
        if self.section == Section::AiProvider {
            content = content.child(self.ai_test_controls(cx));
        }
        div()
            .id("settings")
            .track_focus(&self.focus)
            .flex()
            .size_full()
            .text_size(px(13.))
            .text_color(appearance::palette(cx).foreground)
            .bg(appearance::palette(cx).background)
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
                    .w(px(if compact { 100. } else { 120. }))
                    .h_full()
                    .flex_shrink_0()
                    .border_r_1()
                    .border_color(appearance::palette(cx).border)
                    .when(cfg!(target_os = "macos"), |navigation| {
                        navigation.child(
                            div()
                                .h(px(40.))
                                .w_full()
                                .flex_shrink_0()
                                .window_control_area(WindowControlArea::Drag),
                        )
                    })
                    .child(
                        div()
                            .id("settings-navigation-scroll")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .h(px(100.))
                                    .pt(px(20.))
                                    .when(cfg!(target_os = "macos"), |logo| logo.h(px(60.)).pt_0())
                                    .flex_shrink_0()
                                    .child(
                                        div()
                                            .id("settings-project-link")
                                            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                                this.logo_hover = *hovered;
                                                cx.notify();
                                            }))
                                            .relative()
                                            .size(px(logo_size))
                                            .cursor_pointer()
                                            .on_click(|_, _, cx| {
                                                cx.open_url(
                                                    "https://github.com/Horbin-Magician/rotor",
                                                );
                                            })
                                            .child(
                                                img(logo_glow)
                                                    .absolute()
                                                    .top(px(-glow_padding))
                                                    .left(px(-glow_padding))
                                                    .size(px(logo_size + glow_padding * 2.))
                                                    .opacity(glow_opacity),
                                            )
                                            .child(img(logo).size(px(logo_size))),
                                    ),
                            )
                            .child(
                                div()
                                    .relative()
                                    .flex()
                                    .flex_col()
                                    .flex_shrink_0()
                                    .children(sections.into_iter().enumerate().map(
                                        |(index, (id, section, zh, en, _, _))| {
                                            let selected = self.section == section;
                                            div()
                                                .flex()
                                                .flex_col()
                                                .flex_shrink_0()
                                                .when(matches!(section, Section::Pin), |row| {
                                                    row.child(
                                                        div()
                                                            .mx_4()
                                                            .my_2()
                                                            .h(px(1.))
                                                            .bg(appearance::palette(cx).border),
                                                    )
                                                })
                                                .child(
                                                    div().relative().child(
                                                        appearance::navigation(
                                                            Button::new(id)
                                                                .w_full()
                                                                .h(px(row_height))
                                                                .rounded_none()
                                                                .text_size(px(14.)),
                                                            self.t(zh, en),
                                                            selected,
                                                            self.close_request.is_some(),
                                                            highlights[index],
                                                            cx,
                                                        )
                                                        .on_hover(cx.listener(
                                                            move |this, hovered: &bool, _, cx| {
                                                                if *hovered {
                                                                    this.navigation_hover =
                                                                        Some(section);
                                                                } else if this.navigation_hover
                                                                    == Some(section)
                                                                {
                                                                    this.navigation_hover = None;
                                                                }
                                                                cx.notify();
                                                            },
                                                        ))
                                                        .on_click(cx.listener(
                                                            move |this, _, _, cx| {
                                                                this.section = section;
                                                                cx.notify();
                                                            },
                                                        )),
                                                    ),
                                                )
                                        },
                                    ))
                                    .child(
                                        div()
                                            .absolute()
                                            .top(px(indicator_top + indicator_inset))
                                            .h(px(row_height - indicator_inset * 2.))
                                            .w(px(2.))
                                            .right_0()
                                            .bg(appearance::palette(cx).accent),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .when(cfg!(target_os = "macos"), |body| {
                        body.child(
                            div()
                                .h(px(40.))
                                .w_full()
                                .flex_shrink_0()
                                .window_control_area(WindowControlArea::Drag),
                        )
                    })
                    .when(cfg!(target_os = "windows"), |body| {
                        body.child(
                            div()
                                .flex()
                                .h(px(28.))
                                .flex_shrink_0()
                                .child(
                                    div()
                                        .flex_1()
                                        .h_full()
                                        .window_control_area(WindowControlArea::Drag),
                                )
                                .child(
                                    appearance::quiet_button(
                                        Button::new("settings-minimize")
                                            .icon(IconName::WindowMinimize)
                                            .tooltip(self.t("最小化", "Minimize"))
                                            .accessibility_label(self.t("最小化", "Minimize"))
                                            .w(px(44.))
                                            .h_full()
                                            .rounded_none(),
                                        cx,
                                    )
                                    .on_click(|_, window, _| window.minimize_window()),
                                )
                                .child(
                                    appearance::close_button(
                                        Button::new("settings-window-close")
                                            .icon(IconName::WindowClose)
                                            .tooltip(self.t("关闭", "Close"))
                                            .accessibility_label(self.t("关闭", "Close"))
                                            .w(px(44.))
                                            .h_full()
                                            .rounded_none(),
                                        cx,
                                    )
                                    .disabled(self.close_request.is_some())
                                    .on_click(cx.listener(
                                        |this, _, window, cx| this.request_close(window, cx),
                                    )),
                                ),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .p(px(16.))
                            .pr(px(20.))
                            .pt(px(4.))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_3()
                                    .when(
                                        self.manual_failed || self.autosave.has_failures(),
                                        |body| {
                                            body.child(
                                                appearance::card(cx)
                                                    .p_3()
                                                    .child(self.message.clone())
                                                    .child(
                                                        Button::new("retry-settings")
                                                            .label(self.t("重试", "Retry"))
                                                            .disabled(self.controls_locked())
                                                            .on_click(cx.listener(
                                                                |this, _, window, cx| {
                                                                    this.observe_all_fields(
                                                                        true, window, cx,
                                                                    );
                                                                    this.flush_actions(window, cx);
                                                                },
                                                            )),
                                                    )
                                                    .child(
                                                        Button::new("discard-settings-close")
                                                            .label(self.t(
                                                                "放弃更改并关闭",
                                                                "Discard changes and close",
                                                            ))
                                                            .disabled(
                                                                self.pending.is_some()
                                                                    || self.autosave.has_pending(),
                                                            )
                                                            .on_click(cx.listener(
                                                                |this, _, window, cx| {
                                                                    this.discard_and_close(
                                                                        window, cx,
                                                                    )
                                                                },
                                                            )),
                                                    ),
                                            )
                                        },
                                    )
                                    .child(content),
                            )
                            .overflow_y_scrollbar()
                            .id(("settings-content", self.section as usize)),
                    ),
            )
    }
}

impl Drop for SettingsView {
    fn drop(&mut self) {
        if let Some(id) = self.ai_test.pending {
            self.services.cancel_ai_provider_test(Some(id));
        }
        self.services.set_shortcut_recording(false);
    }
}
