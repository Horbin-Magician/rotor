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
            IndexState::Partial => self.t(
                "部分可用（部分卷失败）",
                "Partially available (some volumes failed)",
            ),
            IndexState::Ready => self.t("就绪", "Ready"),
            IndexState::Error => self.t("失败", "Error"),
        };
        let unavailable = self.t("不可用", "Not available");
        let status = self.index_status.as_ref();
        let mut panel = div()
            .flex()
            .flex_col()
            .gap(px(6.))
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
                    appearance::card(cx)
                        .ml(px(12.))
                        .gap(px(4.))
                        .px(px(10.))
                        .py(px(8.))
                        .child(
                            div()
                                .min_w_0()
                                .text_size(px(14.))
                                .font_weight(FontWeight::BOLD)
                                .child(volume.name.clone())
                                .when(!volume.indexed, |name| {
                                    name.child(appearance::caption(
                                        self.t("未索引", "Not indexed"),
                                        cx,
                                    ))
                                }),
                        )
                        .child(appearance::caption(
                            format!(
                                "{} · {} · {}",
                                volume
                                    .index_item_count
                                    .map(|count| count.to_string())
                                    .unwrap_or_else(|| unavailable.into()),
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

    fn overview_refresh_button(&self, rotation: f32, cx: &mut Context<Self>) -> Button {
        let colors = appearance::palette(cx);
        let disabled = self.overview_request.is_some()
            || self.index_request.is_some()
            || self.controls_locked();
        let icon = div()
            .id("overview-refresh-icon")
            .debug_selector(|| "overview-refresh-content".into())
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .when(!disabled, |icon| {
                icon.group_hover("overview-refresh-button", |style| {
                    style.text_color(colors.accent)
                })
            })
            .child(
                Icon::new(IconName::RotateCw)
                    .size(px(16.))
                    .rotate(radians(rotation)),
            );
        Button::new("refresh-overview")
            .group("overview-refresh-button")
            .size(px(24.))
            // Custom children use the text-button padding by default. Reserve
            // the whole square for the icon, including its rotated bounds.
            .p_0()
            .custom(
                gpui_kit::component::button::ButtonCustomVariant::new(cx)
                    .foreground(colors.secondary),
            )
            .child(icon)
            .tooltip(self.t("刷新概览", "Refresh overview"))
            .accessibility_label(self.t("刷新概览", "Refresh overview"))
            .disabled(disabled)
            .on_click(cx.listener(|this, _, _, cx| {
                this.overview_refresh_started = Some(std::time::Instant::now());
                this.refresh_overview(cx);
            }))
    }

    pub(super) fn overview_panel(&self, rotation: f32, cx: &mut Context<Self>) -> impl IntoElement {
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
                            .child(self.overview_refresh_button(rotation, cx)),
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
                let (label, detail) = match permission.key.as_str() {
                    "administrator" => (
                        self.t("管理员权限", "Administrator"),
                        self.t(
                            "用于 NTFS 日志索引及管理员启动",
                            "For NTFS journal indexing and admin launches",
                        ),
                    ),
                    "screen_capture" => (
                        self.t("屏幕录制", "Screen recording"),
                        self.t("用于截图捕获", "For screenshot capture"),
                    ),
                    "accessibility" => (
                        self.t("辅助功能", "Accessibility"),
                        self.t("用于划词翻译", "For selection translation"),
                    ),
                    "file_search" => (
                        self.t("文件搜索", "File search"),
                        self.t(
                            "使用当前用户可读取的目录或卷",
                            "Uses folders or volumes readable by the current user",
                        ),
                    ),
                    _ => (permission.name.as_str(), permission.detail.as_str()),
                };
                let colors = appearance::palette(cx);
                let (status_color, status_label, status_symbol) = match permission.granted {
                    Some(true) => (cx.theme().success, self.t("可用", "Available"), ""),
                    Some(false) => (cx.theme().danger, self.t("未授予", "Not granted"), "×"),
                    None => (colors.secondary, self.t("未知", "Unknown"), "?"),
                };
                permissions = permissions.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .min_h(px(32.))
                        .pl(px(12.))
                        .py(px(4.))
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_baseline()
                                .min_w_0()
                                .gap_x(px(8.))
                                .gap_y(px(4.))
                                .text_color(colors.secondary)
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::BOLD)
                                        .child(label.to_owned()),
                                )
                                .child(appearance::caption(detail.to_owned(), cx)),
                        )
                        .child(
                            div()
                                .id(SharedString::from(format!("permission-{}", permission.key)))
                                .flex()
                                .items_center()
                                .justify_center()
                                .flex_shrink_0()
                                .size(px(16.))
                                .rounded_full()
                                .bg(status_color)
                                .text_color(colors.background)
                                .text_size(px(13.))
                                .font_weight(FontWeight::BOLD)
                                .tooltip(move |window, cx| {
                                    gpui_kit::component::tooltip::Tooltip::new(status_label)
                                        .build(window, cx)
                                })
                                .when(permission.granted == Some(true), |indicator| {
                                    indicator.child(Icon::new(IconName::Check).size(px(13.)))
                                })
                                .when(permission.granted != Some(true), |indicator| {
                                    indicator.child(status_symbol)
                                }),
                        ),
                );
            }
            panel = panel.child(permissions);
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
        format!("{:.0} KB", bytes as f64 / 1024.)
    } else if bytes < 1073741824 {
        format!("{:.1} MB", bytes as f64 / 1048576.)
    } else {
        format!("{:.1} GB", bytes as f64 / 1073741824.)
    }
}

fn modified_at(milliseconds: Option<u64>) -> Option<String> {
    let timestamp = i64::try_from(milliseconds?).ok()?;
    chrono::DateTime::from_timestamp_millis(timestamp).map(|time| {
        time.with_timezone(&chrono::Local)
            .format("%Y/%-m/%-d %H:%M:%S")
            .to_string()
    })
}
