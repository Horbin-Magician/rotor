use gpui_kit::{
    component::{
        Disableable,
        button::Button,
        input::{Input, InputState, Textarea, TextareaState},
    },
    prelude::*,
    *,
};
use rotor_common::Config;
use rotor_runtime::{IndexState, OperationId, RuntimeEvent, SearchIndexStatus, Services};
use std::sync::Arc;

pub fn settings_title(config: &Config) -> &'static str {
    text(
        config,
        "Rotor 设置（开发版）",
        "Rotor Settings (Development)",
    )
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
    General,
    Search,
    Pin,
    Translation,
}
struct Field {
    key: &'static str,
    section: Section,
    label: (&'static str, &'static str),
    state: Entity<InputState>,
}
pub struct SettingsView {
    config: Config,
    services: Arc<Services>,
    section: Section,
    fields: Vec<Field>,
    excluded: Entity<TextareaState>,
    index_state: IndexState,
    index_status: Option<SearchIndexStatus>,
    pending: Option<OperationId>,
    pending_exclusions: bool,
    choosing_path: bool,
    message: String,
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
        let fields = definitions
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
        Self {
            config,
            services,
            section: Section::General,
            fields,
            excluded,
            index_state: IndexState::Unavailable,
            index_status: None,
            pending: None,
            pending_exclusions: false,
            choosing_path: false,
            message: String::new(),
        }
    }
    fn t(&self, zh: &'static str, en: &'static str) -> &'static str {
        text(&self.config, zh, en)
    }
    pub fn handle_event(&mut self, event: RuntimeEvent, cx: &mut Context<Self>) {
        match event {
            RuntimeEvent::SettingsSaved { id, result } => {
                let own = self.pending == Some(id);
                if own {
                    self.pending = None;
                }
                match result {
                    Ok(config) => {
                        self.config = config;
                        if own {
                            self.message = self.t("设置已保存", "Settings saved").into();
                            if self.pending_exclusions {
                                self.services.rebuild_search();
                            }
                        }
                    }
                    Err(error) if own => self.message = error,
                    Err(_) => {}
                }
                if own {
                    self.pending_exclusions = false;
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
    fn save(&mut self, changes: Vec<(String, String)>, cx: &mut Context<Self>) {
        if self.pending.is_some() {
            return;
        }
        let exclusions = changes.iter().any(|(key, _)| key == "search_excluded_dirs");
        match self.services.save_settings(changes) {
            Ok(id) => {
                self.pending = Some(id);
                self.pending_exclusions = exclusions;
                self.message = self.t("正在保存…", "Saving…").into();
            }
            Err(error) => self.message = error,
        }
        cx.notify();
    }
    fn save_fields(&mut self, cx: &mut Context<Self>) {
        let changes = if self.section == Section::Search {
            vec![(
                "search_excluded_dirs".into(),
                self.excluded.read(cx).value().to_string(),
            )]
        } else {
            self.fields
                .iter()
                .filter(|field| field.section == self.section)
                .map(|field| (field.key.into(), field.state.read(cx).value().to_string()))
                .collect()
        };
        self.save(changes, cx);
    }

    fn choose_save_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.choosing_path || self.pending.is_some() {
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
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.t(label.0, label.1))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_2()
                    .children(options.iter().enumerate().map(|(index, &(value, zh, en))| {
                        Button::new((key, index))
                            .label(self.t(zh, en))
                            .disabled(
                                self.pending.is_some()
                                    || self.config.get(key).is_some_and(|current| current == value),
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.save(vec![(key.into(), value.into())], cx)
                            }))
                    })),
            )
    }
}
impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = div().flex().flex_col().gap_4();
        match self.section {
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
                let state = match self.index_state {
                    IndexState::Unavailable => self.t("未启用", "Disabled"),
                    IndexState::Unbuild => self.t("待构建", "Not built"),
                    IndexState::Building => self.t("构建中", "Building"),
                    IndexState::Released => self.t("已释放", "Released"),
                    IndexState::Loading => self.t("加载中", "Loading"),
                    IndexState::Ready => self.t("就绪", "Ready"),
                    IndexState::Error => self.t("失败", "Error"),
                };
                content = content
                    .child(format!("{}: {state}", self.t("索引状态", "Index status")))
                    .children(self.index_status.as_ref().map(|status| {
                        format!(
                            "{} {} · {} {}",
                            status.index_item_count,
                            self.t("项", "items"),
                            status.volume_count,
                            self.t("个磁盘", "volumes")
                        )
                    }))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("refresh-index")
                                    .label(self.t("刷新状态", "Refresh status"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Err(error) = this.services.request_index_status() {
                                            this.show_message(error, cx);
                                        }
                                    })),
                            )
                            .child(
                                Button::new("rebuild-index")
                                    .label(self.t("重建索引", "Rebuild index"))
                                    .on_click(
                                        cx.listener(|this, _, _, _| this.services.rebuild_search()),
                                    ),
                            )
                            .child(
                                Button::new("release-index")
                                    .label(self.t("释放索引", "Release index"))
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
                            .disabled(self.pending.is_some()),
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
                            ("5", "5", "5"),
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
        }
        content = content.children(
            self.fields
                .iter()
                .filter(|field| field.section == self.section)
                .map(|field| {
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(self.t(field.label.0, field.label.1))
                        .child(
                            Input::new(&field.state)
                                .disabled(self.pending.is_some() || self.choosing_path),
                        )
                }),
        );
        if self.section == Section::Pin {
            content = content.child(
                Button::new("choose-save-directory")
                    .label(self.t("浏览目录…", "Browse…"))
                    .disabled(self.pending.is_some() || self.choosing_path)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.choose_save_directory(window, cx)),
                    ),
            );
        }
        if self.section != Section::General {
            content = content.child(
                Button::new("save-fields")
                    .label(self.t("保存", "Save"))
                    .disabled(self.pending.is_some())
                    .on_click(cx.listener(|this, _, _, cx| this.save_fields(cx))),
            );
        }
        div()
            .flex()
            .flex_col()
            .p_6()
            .gap_4()
            .size_full()
            .child(div().text_2xl().child("Rotor"))
            .child(
                div().flex().gap_2().children(
                    [
                        ("general", Section::General, "通用", "General"),
                        ("search", Section::Search, "搜索", "Search"),
                        ("pin", Section::Pin, "贴图", "Pinned screenshots"),
                        ("translation", Section::Translation, "翻译", "Translation"),
                    ]
                    .into_iter()
                    .map(|(id, section, zh, en)| {
                        Button::new(id)
                            .label(self.t(zh, en))
                            .disabled(self.section == section)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.section = section;
                                cx.notify();
                            }))
                    }),
                ),
            )
            .child(
                div()
                    .id("settings-content")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(content),
            )
            .child(self.message.clone())
            .child(
                Button::new("close")
                    .label(self.t("关闭", "Close"))
                    .on_click(|_, window, _| window.remove_window()),
            )
    }
}
