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
        let state = match self.index_state {
            IndexState::Unavailable => self.t("未启用", "Disabled"),
            IndexState::Unbuild => self.t("待构建", "Not built"),
            IndexState::Building => self.t("构建中", "Building"),
            IndexState::Released => self.t("已释放", "Released"),
            IndexState::Loading => self.t("加载中", "Loading"),
            IndexState::Ready => self.t("就绪", "Ready"),
            IndexState::Error => self.t("失败", "Error"),
        };
        let mut panel = crate::visual::card(cx).child(
            div()
                .flex()
                .flex_wrap()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(self.t("文件索引", "File index")),
                )
                .child(crate::visual::caption(state, cx)),
        );
        if let Some(status) = &self.index_status {
            let unavailable = self.t("暂无记录", "Not available");
            panel = panel
                .child(format!(
                    "{} {} · {}/{} {} · {:.1} MiB",
                    status.index_item_count,
                    self.t("项", "items"),
                    status.indexed_volume_count,
                    status.volume_count,
                    self.t("个磁盘已索引", "volumes indexed"),
                    status.index_file_size_bytes as f64 / 1048576.
                ))
                .child(crate::visual::caption(
                    format!(
                        "{}: {}",
                        self.t("最近索引时间", "Last indexed"),
                        modified_at(status.latest_index_modified_at)
                            .unwrap_or_else(|| unavailable.into())
                    ),
                    cx,
                ))
                .children(status.volumes.iter().map(|volume| {
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .gap_1()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .pt_2()
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .justify_between()
                                .gap_2()
                                .child(volume.name.clone())
                                .child(crate::visual::caption(
                                    if volume.indexed {
                                        self.t("已索引", "Indexed")
                                    } else {
                                        self.t("未索引", "Not indexed")
                                    },
                                    cx,
                                )),
                        )
                        .child(crate::visual::caption(
                            format!(
                                "{} {} · {:.1} MiB · {}",
                                volume
                                    .index_item_count
                                    .map(|count| count.to_string())
                                    .unwrap_or_else(|| unavailable.into()),
                                self.t("项", "items"),
                                volume.index_file_size_bytes as f64 / 1048576.,
                                modified_at(volume.index_file_modified_at)
                                    .unwrap_or_else(|| unavailable.into())
                            ),
                            cx,
                        ))
                }));
        } else {
            panel = panel.child(crate::visual::caption(
                if self.index_request.is_some() {
                    self.t("正在读取索引状态…", "Loading index status…")
                } else {
                    self.t(
                        "索引状态不可用，请刷新重试",
                        "Index status unavailable; refresh to retry",
                    )
                },
                cx,
            ));
        }
        panel
    }

    pub(super) fn refresh_overview(&mut self, cx: &mut Context<Self>) {
        match self.services.request_overview() {
            Ok(id) => self.overview_request = Some(id),
            Err(error) => self.message = error,
        }
        self.refresh_index(cx);
        cx.notify();
    }
    pub(super) fn overview_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut panel = div().flex().flex_col().gap_4().child(
            div()
                .flex()
                .flex_wrap()
                .gap_2()
                .child(
                    Button::new("refresh-overview")
                        .label(self.t("刷新概览", "Refresh overview"))
                        .disabled(self.overview_request.is_some())
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_overview(cx))),
                )
                .child(
                    Button::new("project-home")
                        .label(self.t("项目主页", "Project home"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Err(error) = this
                                .services
                                .open_url("https://github.com/Horbin-Magician/rotor".into())
                            {
                                this.message = error;
                                cx.notify();
                            }
                        })),
                ),
        );
        if let Some(overview) = &self.overview {
            panel = panel
                .child(
                    crate::visual::card(cx)
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!(
                                    "Rotor {} · {} / {}",
                                    overview.version, overview.platform, overview.architecture
                                )),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap_4()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(crate::visual::caption(
                                            self.t("当前进程内存", "Process memory"),
                                            cx,
                                        ))
                                        .child(div().text_2xl().child(
                                            match &overview.resident_bytes {
                                                Ok(bytes) => {
                                                    format!("{:.1} MiB", *bytes as f64 / 1048576.)
                                                }
                                                Err(error) => error.clone(),
                                            },
                                        )),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(crate::visual::caption("OCR", cx))
                                        .child(match overview.ocr_loaded {
                                            Some(true) => self.t("模型已加载", "Model loaded"),
                                            Some(false) => self.t("模型未加载", "Model unloaded"),
                                            None => {
                                                self.t("忙碌或状态不可用", "Busy or unavailable")
                                            }
                                        }),
                                ),
                        ),
                )
                .child(self.index_panel(cx))
                .child(
                    crate::visual::card(cx)
                        .child(crate::visual::caption(
                            self.t("数据目录", "Data directory"),
                            cx,
                        ))
                        .child(overview.data_directory.clone())
                        .child(
                            Button::new("open-data-directory")
                                .label(self.t("打开数据目录", "Open data directory"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    if let Err(error) = this.services.open_data_directory() {
                                        this.message = error;
                                        cx.notify();
                                    }
                                })),
                        ),
                );
            let mut permissions = crate::visual::card(cx).child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.t("权限与系统服务", "Permissions and system services")),
            );
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
                permissions = permissions.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .justify_between()
                        .gap_2()
                        .child(label.to_owned())
                        .child(
                            div()
                                .text_color(match permission.granted {
                                    Some(true) => cx.theme().success,
                                    Some(false) => cx.theme().danger,
                                    None => cx.theme().muted_foreground,
                                })
                                .child(match permission.granted {
                                    Some(true) => self.t("可用", "Available"),
                                    Some(false) => self.t("未授予", "Not granted"),
                                    None => self.t("未知", "Unknown"),
                                }),
                        ),
                );
                if permission.key == "administrator" && permission.granted == Some(false) {
                    permissions = permissions.child(crate::visual::caption(
                        self.t(
                            "NTFS 日志索引需要管理员权限。",
                            "NTFS journal indexing requires administrator privileges.",
                        ),
                        cx,
                    ));
                }
            }
            panel = panel.child(permissions);
            match overview.autostart {
                Ok(enabled) => {
                    panel = panel.child(
                        Button::new("toggle-startup")
                            .label(if enabled {
                                self.t("关闭登录启动", "Disable login startup")
                            } else {
                                self.t("启用登录启动", "Enable login startup")
                            })
                            .disabled(self.startup_request.is_some())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                match this.services.set_autostart(!enabled) {
                                    Ok(id) => this.startup_request = Some(id),
                                    Err(error) => this.message = error,
                                }
                                cx.notify();
                            })),
                    );
                }
                Err(ref error) => {
                    panel = panel.child(format!(
                        "{}: {error}",
                        self.t("启动项状态不可用", "Startup status unavailable")
                    ))
                }
            }
            if !rotor_common::native_app::PRODUCTION {
                panel = panel.child(self.t(
                    "开发版启动项使用独立名称和当前资料目录。",
                    "Development startup uses an independent entry and the current profile.",
                ));
            }
        } else {
            panel = panel.child(self.t("正在读取系统状态…", "Loading system status…"));
        }
        panel
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
