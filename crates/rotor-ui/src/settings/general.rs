use super::*;
use gpui_kit::component::switch::Switch;

impl SettingsView {
    pub(super) fn general_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Keep the builders in separate frames: their by-value GPUI temporaries
        // otherwise accumulate with update_panel on Windows' 1 MiB main stack
        // in unoptimized builds.
        let general = self.general_controls(cx);
        let general = general.child(self.update_panel(cx));
        let shortcuts = self.global_shortcuts(cx);
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(general)
            .child(shortcuts)
    }

    fn general_controls(&self, cx: &mut Context<Self>) -> Div {
        let startup = self.overview.as_ref().map(|overview| &overview.autostart);
        let mut general = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(appearance::heading(self.t("通用", "General"), cx))
            .child(self.dropdown(
                "language",
                ("语言", "Language"),
                &[
                    ("0", "系统默认", "System default"),
                    ("1", "简体中文", "简体中文"),
                    ("2", "English", "English"),
                ],
                cx,
            ))
            .child(self.dropdown(
                "theme",
                ("主题", "Theme"),
                &[
                    ("0", "跟随系统", "Follow system"),
                    ("1", "浅色", "Light"),
                    ("2", "深色", "Dark"),
                ],
                cx,
            ))
            .child(appearance::control_row(
                self.t("开机自启", "Launch at startup"),
                div().flex().justify_end().child(
                    Switch::new("autostart")
                        .accessibility_label(self.t("开机自启", "Launch at startup"))
                        .checked(matches!(startup, Some(Ok(true))))
                        .disabled(
                            self.controls_locked()
                                || self.startup_request.is_some()
                                || self.overview_request.is_some()
                                || !matches!(startup, Some(Ok(_))),
                        )
                        .on_click(cx.listener(|this, enabled: &bool, _, cx| {
                            if this.controls_locked() || this.startup_request.is_some() {
                                return;
                            }
                            match this.services.set_autostart(*enabled) {
                                Ok(id) => this.startup_request = Some(id),
                                Err(error) => this.message = error,
                            }
                            cx.notify();
                        })),
                ),
            ));
        if startup.is_none() || matches!(startup, Some(Err(_))) {
            general = general.child(
                div().pl(px(12.)).child(
                    Button::new("retry-startup")
                        .label(self.t("刷新启动状态", "Refresh startup status"))
                        .disabled(self.controls_locked() || self.overview_request.is_some())
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_overview(cx))),
                ),
            );
        }
        general
    }

    fn global_shortcuts(&self, cx: &mut Context<Self>) -> Div {
        let mut shortcuts = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(appearance::heading(
                self.t("全局快捷键", "Global shortcuts"),
                cx,
            ));
        for key in [
            "shortcut_screenshot",
            "shortcut_search",
            "shortcut_translate_select",
            "shortcut_translate_input",
        ] {
            let field = self
                .fields
                .iter()
                .find(|field| field.key == key)
                .expect("global shortcut field");
            shortcuts = shortcuts.child(self.shortcut_field(field, cx));
        }
        shortcuts
    }
}
