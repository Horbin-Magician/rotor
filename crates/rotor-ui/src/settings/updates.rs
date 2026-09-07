use super::*;
use rotor_runtime::UpdatePhase;

impl SettingsView {
    pub(super) fn update_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let status = match self.update.phase {
            UpdatePhase::Installing => self.t("正在启动安装程序…", "Starting installer…"),
            UpdatePhase::HandedOff => self.t("安装程序已启动", "Installer started"),
            UpdatePhase::Idle => self.t("尚未检查", "Not checked"),
            UpdatePhase::Checking => self.t("正在检查…", "Checking…"),
            UpdatePhase::Current => self.t("已是最新版本", "Up to date"),
            UpdatePhase::Available => self.t("有可用更新", "Update available"),
            UpdatePhase::Downloading => self.t("正在下载并校验…", "Downloading and verifying…"),
            UpdatePhase::Ready => self.t(
                "下载完成，签名校验通过",
                "Downloaded and signature verified",
            ),
            UpdatePhase::Failed => self.t("更新未完成", "Update did not complete"),
        };
        let mut panel = div().flex().flex_col().gap_3()
            .child(format!("Rotor {} · GPUI Preview", env!("CARGO_PKG_VERSION")))
            .child(self.t("预览更新通道独立于正式版；通道尚未发布时检查会报错。", "The preview feed is separate from stable. Checks fail until the feed is published."))
            .child(status)
            .child(Button::new("check-updates").label(self.t("检查更新", "Check for updates"))
                .disabled(self.update.busy())
                .on_click(cx.listener(|this, _, _, cx| {
                    if let Err(error) = this.services.check_updates() { this.message = error; }
                    this.update = this.services.update_snapshot(); cx.notify();
                })));
        if let Some(release) = &self.update.release {
            panel = panel
                .child(format!("v{}", release.version))
                .child(release.notes.clone());
            if !self.update.busy() && self.update.phase != UpdatePhase::Ready {
                panel = panel.child(
                    Button::new("download-update")
                        .label(self.t("下载更新", "Download update"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Err(error) = this.services.download_update() {
                                this.message = error;
                            }
                            this.update = this.services.update_snapshot();
                            cx.notify();
                        })),
                );
            }
        }
        if matches!(
            self.update.phase,
            UpdatePhase::Checking | UpdatePhase::Downloading
        ) {
            if self.update.phase == UpdatePhase::Downloading {
                let done = self.update.downloaded as f64 / 1048576.;
                panel = panel.child(match self.update.total {
                    Some(total) => format!("{done:.1} / {:.1} MiB", total as f64 / 1048576.),
                    None => format!("{done:.1} MiB"),
                });
            }
            panel = panel.child(
                Button::new("cancel-update")
                    .label(self.t("取消", "Cancel"))
                    .on_click(cx.listener(|this, _, _, _| this.services.cancel_update())),
            );
        }
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        if self.update.phase == UpdatePhase::Ready {
            panel = panel.child(
                Button::new("install-update")
                    .label(self.t("退出并安装更新", "Quit and install update"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Err(error) = this.services.install_update() {
                            this.message = error;
                        }
                        this.update = this.services.update_snapshot();
                        cx.notify();
                    })),
            );
        }
        if let Some(error) = &self.update.error {
            panel = panel.child(error.clone());
        }
        if let Some(path) = &self.update.path {
            panel = panel.child(path.display().to_string()).child(
                Button::new("show-update-folder")
                    .label(self.t("打开下载目录", "Open download directory"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(parent) =
                            this.update.path.as_ref().and_then(|path| path.parent())
                            && let Err(error) = this
                                .services
                                .open_file(parent.to_string_lossy().into_owned(), false)
                        {
                            this.message = error;
                            cx.notify();
                        }
                    })),
            );
        }
        panel
    }
}
