use super::*;

impl SettingsView {
    pub(super) fn refresh_index(&mut self, cx: &mut Context<Self>) {
        if self.index_request.is_some() {
            return;
        }
        match self.services.request_index_status() {
            Ok(id) => self.index_request = Some(id),
            Err(error) => self.message = error,
        }
        cx.notify();
    }

    pub(super) fn index_panel(&self, cx: &App) -> impl IntoElement {
        let colors = appearance::palette(cx);
        let state = match self.index_state {
            IndexState::Unavailable => self.t("索引不可用", "Unavailable"),
            IndexState::Unbuild => self.t("待构建", "Not built"),
            IndexState::Building => self.t("构建中", "Building"),
            IndexState::Released => self.t("已释放", "Released"),
            IndexState::Loading => self.t("加载中", "Loading"),
            IndexState::Ready => self.t("就绪", "Ready"),
            IndexState::Error => self.t("失败", "Error"),
        };
        let unavailable = self.t("不可用", "Not available");
        let status = self.index_status.as_ref();
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_1()
            .child(appearance::heading(self.t("索引情况", "Index details"), cx))
            .child(appearance::detail_row(
                self.t("索引状态", "Index status"),
                div()
                    .px(px(6.))
                    .py(px(2.))
                    .rounded_sm()
                    .border_1()
                    .border_color(colors.border)
                    .when(self.index_state == IndexState::Ready, |badge| {
                        badge.text_color(cx.theme().success)
                    })
                    .child(state),
                cx,
            ))
            .child(appearance::detail_row(
                self.t("索引文件大小", "Index file size"),
                status
                    .map(|status| byte_size(status.index_file_size_bytes))
                    .unwrap_or_else(|| unavailable.into()),
                cx,
            ))
            .child(appearance::detail_row(
                self.t("索引条目", "Index entries"),
                status
                    .map(|status| status.index_item_count.to_string())
                    .unwrap_or_else(|| unavailable.into()),
                cx,
            ))
            .child(appearance::detail_row(
                self.t("最近索引时间", "Last indexed"),
                status
                    .and_then(|status| modified_at(status.latest_index_modified_at))
                    .unwrap_or_else(|| unavailable.into()),
                cx,
            ));
        if let Some(status) = status {
            for volume in &status.volumes {
                panel = panel.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .mx_0()
                        .py_2()
                        .border_t_1()
                        .border_color(colors.border)
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .justify_between()
                                .gap_2()
                                .child(volume.name.clone())
                                .child(appearance::caption(
                                    if volume.indexed {
                                        self.t("已索引", "Indexed")
                                    } else {
                                        self.t("未索引", "Not indexed")
                                    },
                                    cx,
                                )),
                        )
                        .child(appearance::caption(
                            format!(
                                "{} {} · {} · {}",
                                volume
                                    .index_item_count
                                    .map(|count| count.to_string())
                                    .unwrap_or_else(|| unavailable.into()),
                                self.t("项", "items"),
                                byte_size(volume.index_file_size_bytes),
                                modified_at(volume.index_file_modified_at)
                                    .unwrap_or_else(|| unavailable.into())
                            ),
                            cx,
                        )),
                );
            }
        } else {
            panel = panel.child(
                appearance::caption(
                    if self.index_request.is_some() {
                        self.t("正在读取索引状态…", "Loading index status…")
                    } else {
                        self.t(
                            "索引状态不可用，请刷新重试",
                            "Index status unavailable; refresh to retry",
                        )
                    },
                    cx,
                )
                .px_2(),
            );
        }
        panel
    }

    pub(super) fn refresh_overview(&mut self, cx: &mut Context<Self>) {
        if self.overview_request.is_some() {
            return;
        }
        match self.services.request_overview() {
            Ok(id) => self.overview_request = Some(id),
            Err(error) => self.message = error,
        }
        self.refresh_index(cx);
        cx.notify();
    }

    fn summary_card(&self, icon: IconName, label: &'static str, value: String, cx: &App) -> Div {
        let colors = appearance::palette(cx);
        appearance::card(cx)
            .ml(px(12.))
            .flex_row()
            .items_center()
            .justify_between()
            .gap_3()
            .min_h(px(36.))
            .px_3()
            .py_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .flex_shrink_0()
                    .text_color(colors.secondary)
                    .child(Icon::new(icon).size(px(14.)))
                    .child(div().text_size(px(13.)).child(label)),
            )
            .child(
                div()
                    .min_w_0()
                    .text_right()
                    .text_size(px(12.))
                    .font_weight(FontWeight::BOLD)
                    .child(value),
            )
    }

    pub(super) fn overview_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut panel = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .pb(px(4.))
                            .border_b_1()
                            .border_color(appearance::palette(cx).border)
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::BOLD)
                                    .child(self.t("系统概览", "System overview")),
                            )
                            .child(
                                appearance::quiet_button(
                                    Button::new("refresh-overview")
                                        .w(px(24.))
                                        .h(px(20.))
                                        .icon(IconName::RotateCw)
                                        .tooltip(self.t("刷新概览", "Refresh overview"))
                                        .accessibility_label(
                                            self.t("刷新概览", "Refresh overview"),
                                        ),
                                    cx,
                                )
                                .disabled(
                                    self.overview_request.is_some()
                                        || self.index_request.is_some()
                                        || self.controls_locked(),
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.refresh_overview(cx))),
                            ),
                    )
                    .child(
                        self.summary_card(
                            IconName::Cpu,
                            self.t("内存占用", "Memory usage"),
                            self.overview
                                .as_ref()
                                .map(|overview| match &overview.resident_bytes {
                                    Ok(bytes) => byte_size(*bytes),
                                    Err(_) => self.t("不可用", "Unavailable").into(),
                                })
                                .unwrap_or_else(|| "—".into()),
                            cx,
                        ),
                    )
                    .child(
                        self.summary_card(
                            IconName::Menu,
                            self.t("索引概览", "Index overview"),
                            self.index_status
                                .as_ref()
                                .map(|status| {
                                    format!(
                                        "{}/{}",
                                        status.indexed_volume_count, status.volume_count
                                    )
                                })
                                .unwrap_or_else(|| "—".into()),
                            cx,
                        ),
                    )
                    .child(
                        self.summary_card(
                            IconName::CircleCheck,
                            self.t("权限概览", "Permissions"),
                            self.overview
                                .as_ref()
                                .map(|overview| {
                                    format!(
                                        "{}/{}",
                                        overview
                                            .permissions
                                            .iter()
                                            .filter(|permission| permission.granted == Some(true))
                                            .count(),
                                        overview.permissions.len()
                                    )
                                })
                                .unwrap_or_else(|| "—".into()),
                            cx,
                        ),
                    ),
            )
            .child(self.index_panel(cx));

        if let Some(overview) = &self.overview {
            let mut permissions = div().flex().flex_col().gap_1().child(appearance::heading(
                self.t("权限使用情况", "Permission details"),
                cx,
            ));
            for permission in &overview.permissions {
                let label = match permission.key.as_str() {
                    "administrator" => self.t("管理员权限", "Administrator"),
                    "screen_capture" => self.t("屏幕捕获权限", "Screen capture"),
                    "accessibility" => self.t(
                        "辅助功能权限（划词翻译）",
                        "Accessibility (selection translation)",
                    ),
                    "file_search" => self.t("文件搜索访问", "File search access"),
                    _ => permission.name.as_str(),
                };
                permissions = permissions.child(appearance::detail_row(
                    label.to_owned(),
                    div()
                        .text_color(match permission.granted {
                            Some(true) => cx.theme().success,
                            Some(false) => cx.theme().danger,
                            None => appearance::palette(cx).secondary,
                        })
                        .child(match permission.granted {
                            Some(true) => self.t("可用", "Available"),
                            Some(false) => self.t("未授予", "Not granted"),
                            None => self.t("未知", "Unknown"),
                        }),
                    cx,
                ));
                if permission.key == "administrator" && permission.granted == Some(false) {
                    permissions = permissions.child(
                        appearance::caption(
                            self.t(
                                "NTFS 日志索引需要管理员权限。",
                                "NTFS journal indexing requires administrator privileges.",
                            ),
                            cx,
                        )
                        .px_2(),
                    );
                }
            }
            panel = panel.child(permissions);

            let mut application = div().flex().flex_col().gap_3()
                .child(appearance::heading(self.t("应用信息", "Application"), cx))
                .child(appearance::detail_row(self.t("当前版本", "Version"),
                    format!("Rotor {} · {} / {}", overview.version, overview.platform, overview.architecture), cx))
                .child(appearance::detail_row("OCR", match overview.ocr_loaded {
                    Some(true) => self.t("模型已加载", "Model loaded"),
                    Some(false) => self.t("模型未加载", "Model unloaded"),
                    None => self.t("忙碌或状态不可用", "Busy or unavailable"),
                }, cx))
                .child(appearance::caption(if cfg!(target_os = "windows") {
                    self.t("内存占用统计当前进程的私有工作集，点击刷新更新。", "Memory usage is the process private working set. Refresh to update.")
                } else {
                    self.t("内存占用统计当前进程内存，点击刷新更新。", "Memory usage is for the current process. Refresh to update.")
                }, cx).px_2());
            if let Err(error) = &overview.resident_bytes {
                application = application.child(
                    appearance::caption(error.clone(), cx)
                        .px_2()
                        .text_color(cx.theme().danger),
                );
            }
            application = application.child(
                appearance::card(cx)
                    .child(appearance::caption(
                        self.t("数据目录", "Data directory"),
                        cx,
                    ))
                    .child(div().min_w_0().child(overview.data_directory.clone()))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                Button::new("open-data-directory")
                                    .label(self.t("打开数据目录", "Open data directory"))
                                    .disabled(self.controls_locked())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Err(error) = this.services.open_data_directory() {
                                            this.message = error;
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                Button::new("project-home")
                                    .label(self.t("项目主页", "Project home"))
                                    .disabled(self.controls_locked())
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if let Err(error) = this.services.open_url(
                                            "https://github.com/Horbin-Magician/rotor".into(),
                                        ) {
                                            this.message = error;
                                            cx.notify();
                                        }
                                    })),
                            ),
                    ),
            );
            match overview.autostart {
                Ok(enabled) => {
                    application = application.child(appearance::detail_row(
                        self.t("登录启动", "Launch at login"),
                        Button::new("toggle-startup")
                            .label(if enabled {
                                self.t("关闭登录启动", "Disable login startup")
                            } else {
                                self.t("启用登录启动", "Enable login startup")
                            })
                            .disabled(self.startup_request.is_some() || self.controls_locked())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                match this.services.set_autostart(!enabled) {
                                    Ok(id) => this.startup_request = Some(id),
                                    Err(error) => this.message = error,
                                }
                                cx.notify();
                            })),
                        cx,
                    ));
                }
                Err(ref error) => {
                    application = application.child(appearance::caption(
                        format!(
                            "{}: {error}",
                            self.t("启动项状态不可用", "Startup status unavailable")
                        ),
                        cx,
                    ));
                }
            }
            if !rotor_common::native_app::PRODUCTION {
                application = application.child(appearance::caption(
                    self.t(
                        "开发版启动项使用独立名称和当前资料目录。",
                        "Development startup uses an independent entry and the current profile.",
                    ),
                    cx,
                ));
            }
            panel = panel.child(application);
        } else {
            panel = panel.child(appearance::caption(
                if self.overview_request.is_some() {
                    self.t("正在读取系统状态…", "Loading system status…")
                } else {
                    self.t(
                        "系统状态不可用，请刷新重试",
                        "System status unavailable; refresh to retry",
                    )
                },
                cx,
            ));
        }
        panel
    }
}

fn byte_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1048576 {
        format!("{:.1} KiB", bytes as f64 / 1024.)
    } else if bytes < 1073741824 {
        format!("{:.1} MiB", bytes as f64 / 1048576.)
    } else {
        format!("{:.1} GiB", bytes as f64 / 1073741824.)
    }
}

fn modified_at(milliseconds: Option<u64>) -> Option<String> {
    let timestamp = i64::try_from(milliseconds?).ok()?;
    chrono::DateTime::from_timestamp_millis(timestamp).map(|time| {
        time.with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M")
            .to_string()
    })
}
