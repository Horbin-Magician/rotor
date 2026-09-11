use super::*;
use gpui_kit::component::{
    menu::{DropdownMenu, PopupMenuItem},
    switch::Switch,
};
impl SettingsView {
    pub(super) fn zoom_step_slider(&self, cx: &mut Context<Self>) -> Div {
        appearance::control_row(
            self.t("滚轮缩放步长", "Scroll zoom step"),
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .px_2()
                        .child(Slider::new(&self.zoom_step).disabled(self.controls_locked())),
                )
                .child(
                    div()
                        .w(px(24.))
                        .text_right()
                        .child(self.zoom_step.read(cx).value().to_string()),
                ),
        )
        .when(self.autosave.failed("zoom_delta"), |row| {
            row.child(
                appearance::caption(self.t("未保存", "Not saved"), cx)
                    .text_color(cx.theme().danger),
            )
        })
    }

    pub(super) fn dropdown(
        &self,
        key: &'static str,
        label: (&'static str, &'static str),
        options: &'static [(&'static str, &'static str, &'static str)],
        cx: &mut Context<Self>,
    ) -> Div {
        let current = self.config.get(key).map(String::as_str).unwrap_or("0");
        let selected = options
            .iter()
            .find(|option| option.0 == current)
            .unwrap_or(&options[0]);
        let view = cx.entity().downgrade();
        let items: Vec<_> = options
            .iter()
            .map(|&(value, zh, en)| (value, self.t(zh, en), value == selected.0))
            .collect();
        appearance::control_row(
            self.t(label.0, label.1),
            appearance::control_button(Button::new(key), cx)
                .h(px(34.))
                .px_3()
                .accessibility_label(self.t(label.0, label.1))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .w_full()
                        .child(self.t(selected.1, selected.2))
                        .child(Icon::new(IconName::ChevronDown).size(px(14.))),
                )
                .disabled(self.controls_locked())
                .dropdown_menu(move |mut menu, _, _| {
                    for &(value, label, checked) in &items {
                        let view = view.clone();
                        menu = menu.item(PopupMenuItem::new(label).checked(checked).on_click(
                            move |_, _, cx| {
                                let _ = view.update(cx, |this, cx| {
                                    this.save(vec![(key.into(), value.into())], cx);
                                });
                            },
                        ));
                    }
                    menu
                }),
        )
        .when(self.autosave.failed(key), |row| {
            row.child(
                appearance::caption(self.t("未保存", "Not saved"), cx)
                    .text_color(cx.theme().danger),
            )
        })
    }

    pub(super) fn shortcut_field(&self, field: &Field, cx: &mut Context<Self>) -> Div {
        let key = field.key;
        let recording =
            matches!(self.recording, Some(actions::Recording::Setting(target)) if target == key);
        let value = if recording {
            self.t("请按快捷键…", "Press a shortcut…").to_owned()
        } else {
            field.state.read(cx).value().to_string()
        };
        appearance::control_row(
            self.t(field.label.0, field.label.1),
            appearance::control_button(Button::new(key), cx)
                .label(value)
                .accessibility_label(self.t(field.label.0, field.label.1))
                .tooltip(self.t(
                    "点击录制快捷键，Esc 取消",
                    "Click to record a shortcut; Esc cancels",
                ))
                .disabled(self.controls_locked())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.start_recording(actions::Recording::Setting(key), window, cx);
                })),
        )
        .when(self.autosave.failed(key), |row| {
            row.child(
                appearance::caption(self.t("未保存", "Not saved"), cx)
                    .text_color(cx.theme().danger),
            )
        })
    }
    pub(super) fn toggle(
        &self,
        key: &'static str,
        label: (&'static str, &'static str),
        cx: &mut Context<Self>,
    ) -> Div {
        appearance::control_row(
            self.t(label.0, label.1),
            div().flex().justify_end().child(
                Switch::new(key)
                    .accessibility_label(self.t(label.0, label.1))
                    .checked(self.config.get(key).is_some_and(|value| value == "true"))
                    .disabled(self.controls_locked())
                    .on_click(cx.listener(move |this, enabled: &bool, _, cx| {
                        this.save(vec![(key.into(), enabled.to_string())], cx);
                    })),
            ),
        )
        .when(self.autosave.failed(key), |row| {
            row.child(
                appearance::caption(self.t("未保存", "Not saved"), cx)
                    .text_color(cx.theme().danger),
            )
        })
    }
}
