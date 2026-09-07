use super::*;

impl SettingsView {
    pub(super) fn refresh_overview(&mut self, cx: &mut Context<Self>) {
        match self.services.request_overview() {
            Ok(id) => self.overview_request = Some(id),
            Err(error) => self.message = error,
        }
        cx.notify();
    }
    pub(super) fn overview_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut panel = div().flex().flex_col().gap_3().child(
            Button::new("refresh-overview")
                .label(self.t("刷新概览", "Refresh overview"))
                .on_click(cx.listener(|this, _, _, cx| this.refresh_overview(cx))),
        );
        if let Some(overview) = &self.overview {
            panel = panel
                .child(format!(
                    "Rotor {} · {} / {}",
                    overview.version, overview.platform, overview.architecture
                ))
                .child(format!(
                    "{}: {}",
                    self.t("数据目录", "Data directory"),
                    overview.data_directory
                ))
                .child(match &overview.resident_bytes {
                    Ok(bytes) => format!(
                        "{}: {:.1} MiB",
                        self.t("当前进程内存", "Process resident memory"),
                        *bytes as f64 / 1048576.
                    ),
                    Err(error) => error.clone(),
                })
                .child(format!(
                    "OCR: {}",
                    match overview.ocr_loaded {
                        Some(true) => self.t("模型已加载", "Model loaded"),
                        Some(false) => self.t("模型未加载", "Model unloaded"),
                        None => self.t("忙碌或状态不可用", "Busy or unavailable"),
                    }
                ))
                .child(
                    Button::new("open-data-directory")
                        .label(self.t("打开数据目录", "Open data directory"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Err(error) = this.services.open_data_directory() {
                                this.message = error;
                                cx.notify();
                            }
                        })),
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
                panel = panel.child(format!(
                    "{label}: {}",
                    match permission.granted {
                        Some(true) => self.t("可用", "Available"),
                        Some(false) => self.t("未授予", "Not granted"),
                        None => self.t("未知", "Unknown"),
                    }
                ));
                if permission.key == "administrator" && permission.granted == Some(false) {
                    panel = panel.child(self.t(
                        "NTFS 日志索引需要管理员权限。",
                        "NTFS journal indexing requires administrator privileges.",
                    ));
                }
            }
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
            panel = panel.child(self.t(
                "开发版启动项使用独立名称和当前资料目录。",
                "Development startup uses an independent entry and the current profile.",
            ));
        } else {
            panel = panel.child(self.t("正在读取系统状态…", "Loading system status…"));
        }
        panel
    }
}
