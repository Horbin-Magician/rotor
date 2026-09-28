use super::*;
use gpui_kit::component::{Sizable, text::TextView};
use rotor_runtime::UpdatePhase;

// Render the modal in its own entity so it can observe settings without borrowing
// SettingsView again while its dialog layer is being built.
struct UpdateDialog {
    settings: WeakEntity<SettingsView>,
    _subscription: Subscription,
}

impl Render for UpdateDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.settings
            .update(cx, |settings, cx| {
                settings.update_dialog_content(window, cx)
            })
            .unwrap_or_else(|_| div())
    }
}

impl SettingsView {
    fn update_action_error(&mut self, code: &str, error: &str) {
        log::error!("[{code}] {error}");
        self.message = format!("{code} · {}", self.t("操作失败", "Action failed"));
    }

    fn update_download_cancelled(&self) -> bool {
        self.update.phase == UpdatePhase::Failed
            && self.update.error.as_deref() == Some("Update download cancelled")
    }

    fn update_label(&self) -> String {
        if let Some(code) = self.update.error_code() {
            let label = match code {
                "E-UPD-003" | "E-UPD-004" => self.t("安装失败", "Install failed"),
                "E-UPD-002" => self.t("下载失败", "Download failed"),
                _ => self.t("检查失败", "Check failed"),
            };
            return format!("{code} · {label}");
        }
        match self.update.phase {
            UpdatePhase::Idle => self.t("检查更新", "Check for updates"),
            UpdatePhase::Checking => self.t("正在检查…", "Checking…"),
            UpdatePhase::Current => self.t("已是最新版本", "Up to date"),
            UpdatePhase::Available => self.t("有可用更新", "Update available"),
            UpdatePhase::Downloading => return self.download_label(),
            UpdatePhase::Ready if self.update.error.is_some() => {
                self.t("安装失败，点击重试", "Installation failed · Retry")
            }
            UpdatePhase::Ready => self.t("更新已就绪", "Ready to install"),
            UpdatePhase::Installing => self.t("正在启动安装程序…", "Starting installer…"),
            UpdatePhase::HandedOff => self.t("安装程序已启动", "Installer started"),
            UpdatePhase::Failed if self.update_download_cancelled() => {
                self.t("下载已取消", "Download cancelled")
            }
            UpdatePhase::Failed if self.update.release.is_some() => {
                self.t("下载失败，点击重试", "Download failed · Retry")
            }
            UpdatePhase::Failed => self.t("检查失败，点击重试", "Check failed · Retry"),
        }
        .into()
    }

    fn download_label(&self) -> String {
        if let Some(total) = self.update.total.filter(|total| *total > 0) {
            if self.update.downloaded >= total {
                return self.t("正在校验更新…", "Verifying update…").into();
            }
            return format!(
                "{} {:.0}%",
                self.t("正在下载", "Downloading"),
                self.update.downloaded as f64 / total as f64 * 100.
            );
        }
        format!(
            "{} {:.1} MiB",
            self.t("正在下载", "Downloading"),
            self.update.downloaded as f64 / 1048576.
        )
    }

    pub(super) fn update_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let checking = self.update.phase == UpdatePhase::Checking;
        let failed = self.update.error.is_some();
        let button = appearance::button("check-updates")
            .h(px(32.))
            .label(self.update_label())
            .when(checking, |button| {
                button
                    .info()
                    .child(gpui_kit::component::spinner::Spinner::new().small())
            })
            .when(failed, |button| button.danger())
            .when(
                !failed && self.update.phase == UpdatePhase::Current,
                |button| button.success(),
            )
            .when(!failed && self.update.release.is_some(), |button| {
                button.primary()
            })
            .disabled(self.controls_locked())
            .tooltip(if self.update.error.is_some() {
                self.update_label()
            } else {
                if checking {
                    self.t("点击取消检查", "Click to cancel check")
                } else if self.update.release.is_some() {
                    self.t("查看更新内容与进度", "View release notes and progress")
                } else {
                    self.t("检查是否有新版本", "Check for a new version")
                }
                .into()
            })
            .on_click(cx.listener(|this, _, window, cx| {
                if this.update.phase == UpdatePhase::Checking {
                    this.services.cancel_update();
                } else if this.update.release.is_some() {
                    this.show_update_dialog(window, cx);
                } else {
                    if let Err(error) = this.services.check_updates() {
                        this.update_action_error("E-UPD-001", &error);
                    }
                    this.update = this.services.update_snapshot();
                    if this.update.phase == UpdatePhase::Available {
                        this.show_update_dialog(window, cx);
                    }
                    cx.notify();
                }
            }));
        div()
            .id("update-row")
            .debug_selector(|| "update-row".into())
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .min_h(px(36.))
            .pl(px(12.))
            .child(format!(
                "{} {}",
                self.t("当前版本：", "Current version:"),
                env!("CARGO_PKG_VERSION")
            ))
            .child(button)
    }

    pub(super) fn show_update_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.update_dialog_open || self.update.release.is_none() {
            return;
        }
        self.update_dialog_open = true;
        let settings = cx.entity();
        let content = cx.new(|cx| UpdateDialog {
            settings: settings.downgrade(),
            _subscription: cx.observe(&settings, |_, _, cx| cx.notify()),
        });
        let title = self.t("软件更新", "Software update");
        let settings = settings.downgrade();
        window.open_dialog(cx, move |dialog, window, _| {
            let settings = settings.clone();
            dialog
                .title(title)
                .w((window.viewport_size().width - px(48.)).min(px(620.)))
                .margin_top(px(32.))
                .overlay_closable(false)
                .content({
                    let content = content.clone();
                    move |body, _, _| body.child(content.clone())
                })
                .on_close(move |_, _, cx| {
                    let _ = settings.update(cx, |settings, cx| {
                        settings.update_dialog_open = false;
                        cx.notify();
                    });
                })
        });
        cx.notify();
    }

    fn update_dialog_content(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let Some(release) = &self.update.release else {
            return div();
        };
        let notes = if release.notes.trim().is_empty() {
            self.t("此版本未提供更新说明。", "No release notes were provided.")
                .to_owned()
        } else {
            release.notes.clone()
        };
        let body = div()
            .flex()
            .flex_col()
            .gap_3()
            .min_w_0()
            .child(
                div()
                    .text_size(px(18.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(format!(
                        "Rotor {} → {}",
                        env!("CARGO_PKG_VERSION"),
                        release.version
                    )),
            )
            .child(
                div()
                    .id("update-release-notes")
                    .debug_selector(|| "update-release-notes".into())
                    .h(window.viewport_size().height - px(180.))
                    .min_w_0()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().border)
                    .overflow_hidden()
                    .child(
                        div().size_full().p_3().overflow_y_scrollbar().child(
                            TextView::markdown("update-notes-markdown", notes)
                                .selectable(true)
                                .scrollable(false),
                        ),
                    ),
            );
        body.child(self.update_dialog_actions(cx))
    }

    fn update_dialog_actions(&self, cx: &mut Context<Self>) -> Div {
        let mut status = div().flex_1().min_w_0();
        if self.update.phase != UpdatePhase::Available {
            status = status.child(
                appearance::caption(self.update_label(), cx)
                    .debug_selector(|| "update-status".into())
                    .truncate()
                    .when(self.update.error_code().is_some(), |status| {
                        status.text_color(cx.theme().danger)
                    }),
            );
        }
        let mut actions = div()
            .flex()
            .flex_shrink_0()
            .flex_wrap()
            .items_center()
            .justify_end()
            .gap_2();
        if self.update.path.is_some() {
            actions = actions.child(
                appearance::button("show-update-folder")
                    .label(self.t("打开下载目录", "Open download folder"))
                    .disabled(self.controls_locked())
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(parent) =
                            this.update.path.as_ref().and_then(|path| path.parent())
                            && let Err(error) = this
                                .services
                                .open_file(parent.to_string_lossy().into_owned(), false)
                        {
                            this.update_action_error("E-UPD-005", &error);
                            cx.notify();
                        }
                    })),
            );
        }
        if self.update.phase == UpdatePhase::Downloading {
            actions = actions.child(
                appearance::button("cancel-update")
                    .label(self.t("取消下载", "Cancel download"))
                    .on_click(cx.listener(|this, _, _, _| this.services.cancel_update())),
            );
        } else if self.update.phase == UpdatePhase::Ready {
            actions = actions.child(
                appearance::button("install-update")
                    .primary()
                    .label(self.t("退出并安装更新", "Quit and install update"))
                    .disabled(
                        self.controls_locked()
                            || self.autosave.has_pending()
                            || self.autosave.has_composition()
                            || self.autosave.has_failures()
                            || self.manual_failed,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Err(error) = this.services.install_update() {
                            this.update_action_error("E-UPD-003", &error);
                        }
                        this.update = this.services.update_snapshot();
                        cx.notify();
                    })),
            );
        } else if !self.update.busy() {
            actions = actions.child(
                appearance::button("download-update")
                    .primary()
                    .label(if self.update.phase == UpdatePhase::Failed {
                        self.t("重新下载", "Retry download")
                    } else {
                        self.t("下载更新", "Download update")
                    })
                    .disabled(self.controls_locked())
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Err(error) = this.services.download_update() {
                            this.update_action_error("E-UPD-002", &error);
                        }
                        this.update = this.services.update_snapshot();
                        cx.notify();
                    })),
            );
        }
        div()
            .debug_selector(|| "update-actions".into())
            .flex()
            .items_center()
            .gap_3()
            .child(status)
            .child(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{Section, SettingsView};
    use gpui::{AppContext, px};
    use gpui_kit::component::{Root, WindowExt};
    use rotor_common::ConfigService;
    use rotor_runtime::{RuntimeEvent, ServiceOptions, Services, UpdatePhase, UpdateSnapshot};
    use std::sync::{Arc, Mutex};

    #[test]
    fn update_dialog_tracks_events_without_growing_the_settings_row() {
        std::thread::Builder::new()
            .name("update-dialog-main-stack".into())
            .stack_size(1024 * 1024)
            .spawn(|| {
                let profile = tempfile::tempdir().unwrap();
                let config = ConfigService::load_from(profile.path()).unwrap();
                let (services, _events) = Services::new(
                    Arc::new(Mutex::new(config)),
                    None,
                    ServiceOptions { index_files: false },
                ).unwrap();
                let services = Arc::new(services);
                let mut cx = gpui::TestAppContext::single();
                cx.update(gpui_kit::component::init);
                let mut settings = None;
                let (_, cx) = cx.add_window_view(|window, cx| {
                    let view = cx.new(|cx| SettingsView::new(services.settings(), services.clone(), window, cx));
                    view.update(cx, |view, _| view.section = Section::General);
                    settings = Some(view.clone());
                    Root::new(view, window, cx)
                });
                let view = settings.unwrap();
                cx.update(|window, cx| window.draw(cx).clear(cx));
                let initial_row = cx.debug_bounds("update-row").unwrap();
                for phase in [UpdatePhase::Checking, UpdatePhase::Current, UpdatePhase::Failed] {
                    view.update(cx, |view, cx| {
                        view.update = Arc::new(UpdateSnapshot {
                            phase,
                            error: (phase == UpdatePhase::Failed).then(|| "Synthetic network error".into()),
                            ..Default::default()
                        });
                        cx.notify();
                    });
                    cx.update(|window, cx| window.draw(cx).clear(cx));
                    assert_eq!(cx.debug_bounds("update-row").unwrap(), initial_row);
                    assert!(cx.debug_bounds("dialog-layer").is_none());
                }
                let mut snapshot = UpdateSnapshot {
                    revision: 10,
                    phase: UpdatePhase::Available,
                    release: Some(Arc::new(rotor_updater::Release {
                        version: "99.0.0".into(),
                        notes: "# Release notes\n\n## Improvements\n\n- **Search** improvements\n- Screenshot fixes\n\n".repeat(30),
                        artifact: rotor_updater::Artifact { signature: String::new(), url: String::new() },
                        target: String::new(),
                    })),
                    ..Default::default()
                };
                cx.update(|window, cx| {
                    view.update(cx, |view, cx| view.handle_event(RuntimeEvent::Update(Arc::new(snapshot.clone())), window, cx));
                    window.draw(cx).clear(cx);
                    assert!(window.has_active_dialog(cx));
                });
                assert!(cx.debug_bounds("dialog-layer").is_some());
                let notes_bounds = cx.debug_bounds("update-release-notes").unwrap();
                let available_height = cx.update(|window, _| window.viewport_size().height - px(180.));
                assert!(notes_bounds.size.height <= available_height, "{notes_bounds:?}");
                assert_eq!(cx.debug_bounds("update-row").unwrap(), initial_row);
                for phase in [UpdatePhase::Downloading, UpdatePhase::Failed, UpdatePhase::Ready, UpdatePhase::Installing] {
                    snapshot.revision += 1;
                    snapshot.phase = phase;
                    snapshot.downloaded = 50;
                    snapshot.total = Some(100);
                    snapshot.error = match phase {
                        UpdatePhase::Failed => Some("Synthetic download error\n".repeat(100)),
                        UpdatePhase::Ready => Some("Installer identity/version differs from the selected update".into()),
                        _ => None,
                    };
                    cx.update(|window, cx| {
                        view.update(cx, |view, cx| view.handle_event(RuntimeEvent::Update(Arc::new(snapshot.clone())), window, cx));
                        window.draw(cx).clear(cx);
                        assert!(window.has_active_dialog(cx));
                    });
                    assert_eq!(cx.debug_bounds("update-row").unwrap(), initial_row);
                    assert!(cx.debug_bounds("update-release-notes").is_some());
                    let status = cx.debug_bounds("update-status").unwrap();
                    assert!(status.size.height <= px(24.), "{status:?}");
                    view.read_with(cx, |view, _| {
                        let label = view.update_label();
                        assert!(!label.contains('\n'));
                        assert!(!label.contains("Synthetic"));
                        if phase == UpdatePhase::Ready {
                            assert!(label.contains("E-UPD-004"));
                        }
                    });
                }
                // Closing must restore the entry point; old/duplicate events must
                // not reopen the dialog or replace a newer download state.
                cx.simulate_keystrokes("escape");
                cx.update(|window, cx| window.draw(cx).clear(cx));
                view.read_with(cx, |view, _| assert!(!view.update_dialog_open));
                snapshot.revision = 10;
                snapshot.phase = UpdatePhase::Available;
                cx.update(|window, cx| {
                    view.update(cx, |view, cx| view.handle_event(RuntimeEvent::Update(Arc::new(snapshot)), window, cx));
                    window.draw(cx).clear(cx);
                    assert!(!window.has_active_dialog(cx));
                    view.update(cx, |view, cx| view.show_update_dialog(window, cx));
                    window.draw(cx).clear(cx);
                    assert!(window.has_active_dialog(cx));
                });
                services.shutdown();
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
