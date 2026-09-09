use super::action_change::{ActionChange, plan_action_change};
use super::*;
use crate::shortcut::recorded_key;
use gpui_kit::component::IconName;
use rotor_runtime::QuickAction;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_ACTION: AtomicU64 = AtomicU64::new(1);
#[derive(Clone)]
pub(super) enum Recording {
    Setting(&'static str),
    Action(String),
}
pub(super) struct ActionFields {
    id: String,
    name: Entity<InputState>,
    shortcut: Entity<InputState>,
    command: Entity<TextareaState>,
    enabled: bool,
    _input_events: Vec<Subscription>,
}
impl ActionFields {
    pub(super) fn new(
        action: QuickAction,
        window: &mut Window,
        cx: &mut Context<SettingsView>,
    ) -> Self {
        let name = cx.new(|cx| InputState::new(window, cx).default_value(action.name));
        let shortcut = cx.new(|cx| InputState::new(window, cx).default_value(action.shortcut));
        let command = cx.new(|cx| TextareaState::new(window, cx).default_value(action.command));
        let input_events = vec![
            cx.subscribe(&name, |_, _, event, cx| {
                if matches!(event, gpui_kit::component::input::InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe(&shortcut, |_, _, event, cx| {
                if matches!(event, gpui_kit::component::input::InputEvent::Change) {
                    cx.notify();
                }
            }),
            cx.subscribe(&command, |_, _, event, cx| {
                if matches!(event, gpui_kit::component::input::InputEvent::Change) {
                    cx.notify();
                }
            }),
        ];
        Self {
            id: action.id,
            name,
            shortcut,
            command,
            enabled: action.enabled,
            _input_events: input_events,
        }
    }
    fn value(&self, cx: &App) -> QuickAction {
        QuickAction {
            id: self.id.clone(),
            name: self.name.read(cx).value().to_string(),
            shortcut: self.shortcut.read(cx).value().to_string(),
            command: self.command.read(cx).value().to_string(),
            enabled: self.enabled,
        }
    }
}
impl SettingsView {
    pub(super) fn action_has_marked_text(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.actions.iter().any(|action| {
            action.name.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            }) || action.shortcut.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            }) || action.command.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            })
        })
    }
    pub(super) fn start_recording(
        &mut self,
        target: Recording,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.controls_locked() {
            return;
        }
        self.recording = Some(target);
        self.services.set_shortcut_recording(true);
        self.focus.focus(window, cx);
        self.message = self
            .t(
                "请按快捷键；全局录制按 Esc 取消",
                "Press a shortcut; Esc cancels global recording",
            )
            .into();
        cx.notify();
    }
    pub(super) fn record_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.recording.clone() else {
            return;
        };
        let local =
            matches!(&target, Recording::Setting(key) if key.starts_with("shortcut_pinwin_"));
        if event.keystroke.key == "escape"
            && !local
            && !event.keystroke.modifiers.control
            && !event.keystroke.modifiers.alt
            && !event.keystroke.modifiers.platform
        {
            self.recording = None;
            self.services.set_shortcut_recording(false);
            self.message.clear();
            cx.notify();
            cx.stop_propagation();
            return;
        }
        match recorded_key(&event.keystroke, local) {
            Ok(Some(value)) => {
                self.receive_recorded_shortcut(value, window, cx);
            }
            Ok(None) => {}
            Err(error) => {
                self.message = error;
                cx.notify();
            }
        }
        cx.stop_propagation();
    }
    pub fn receive_recorded_shortcut(
        &mut self,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(target) = self.recording.take() else {
            return;
        };
        self.services.set_shortcut_recording(false);
        let setting_key = match &target {
            Recording::Setting(key) => Some(*key),
            Recording::Action(_) => None,
        };
        let field = match target {
            Recording::Setting(key) => self
                .fields
                .iter()
                .find(|field| field.key == key)
                .map(|field| field.state.clone()),
            Recording::Action(id) => self
                .actions
                .iter()
                .find(|action| action.id == id)
                .map(|action| action.shortcut.clone()),
        };
        if let Some(field) = field {
            field.update(cx, |field, cx| field.set_value(value, window, cx));
        }
        if let Some(key) = setting_key {
            self.observe_field(key, true, window, cx);
        } else {
            self.message = self
                .t("已录入，保存后生效", "Recorded; save to apply")
                .into();
        }
        cx.notify();
    }
    pub(super) fn sync_saved_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_keys.iter().any(|key| key == "quick_actions")
            && let Ok(actions) = self.services.quick_actions()
        {
            self.actions = actions
                .into_iter()
                .map(|action| ActionFields::new(action, window, cx))
                .collect();
            self.editing_action = None;
        }
    }
    pub(super) fn save_actions(&mut self, cx: &mut Context<Self>) {
        let actions = self.actions.iter().map(|action| action.value(cx)).collect();
        self.submit_actions(actions, cx);
    }
    fn submit_actions(&mut self, actions: Vec<QuickAction>, cx: &mut Context<Self>) {
        if self.controls_locked() {
            return;
        }
        match self.services.save_quick_actions(actions) {
            Ok(id) => {
                self.manual_failed = false;
                self.pending = Some(id);
                self.pending_keys = vec!["quick_actions".into()];
                self.message = self.t("正在保存…", "Saving…").into();
            }
            Err(error) => {
                self.manual_failed = true;
                self.message = error;
            }
        }
        cx.notify();
    }
    fn change_action(&mut self, change: ActionChange, cx: &mut Context<Self>) {
        if self.controls_locked() {
            return;
        }
        let saved = match self.services.quick_actions() {
            Ok(saved) => saved,
            Err(error) => {
                self.show_message(error, cx);
                return;
            }
        };
        let drafts = self
            .actions
            .iter()
            .map(|action| action.value(cx))
            .collect::<Vec<_>>();
        let Some(plan) = plan_action_change(&drafts, &saved, self.editing_action.is_some(), change)
        else {
            return;
        };
        if plan.save_immediately {
            // Keep the current rows until the existing registration/disk
            // transaction succeeds; a failed delete therefore leaves its row.
            self.submit_actions(plan.actions, cx);
        } else {
            let enabled = plan
                .actions
                .iter()
                .map(|action| (action.id.as_str(), action.enabled))
                .collect::<std::collections::HashMap<_, _>>();
            self.actions.retain_mut(|action| {
                if let Some(value) = enabled.get(action.id.as_str()) {
                    action.enabled = *value;
                    true
                } else {
                    false
                }
            });
            if self
                .editing_action
                .as_ref()
                .is_some_and(|id| !self.actions.iter().any(|action| &action.id == id))
            {
                self.editing_action = None;
            }
            self.message = self
                .t(
                    "更改已加入未保存的草稿，保存后生效",
                    "Change added to the existing draft; save to apply",
                )
                .into();
            cx.notify();
        }
    }
    fn cancel_action_edit(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.controls_locked() || self.editing_action.as_deref() != Some(id) {
            return;
        }
        let saved = match self.services.quick_actions() {
            Ok(saved) => saved,
            Err(error) => {
                self.show_message(error, cx);
                return;
            }
        };
        if let Some(index) = self.actions.iter().position(|action| action.id == id) {
            if let Some(action) = saved.iter().find(|action| action.id == id).cloned() {
                self.actions[index] = ActionFields::new(action, window, cx);
            } else {
                self.actions.remove(index);
            }
        }
        self.editing_action = None;
        self.recording = None;
        self.services.set_shortcut_recording(false);
        if self
            .actions
            .iter()
            .map(|action| action.value(cx))
            .collect::<Vec<_>>()
            == saved
        {
            self.manual_failed = false;
        }
        if self.manual_failed || self.autosave.has_failures() {
            self.saved_feedback();
        } else {
            self.message = self.t("已取消编辑", "Edit cancelled").into();
        }
        self.focus.focus(window, cx);
        cx.notify();
    }

    fn add_action(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        self.actions.push(ActionFields::new(
            QuickAction {
                id: format!(
                    "action-{stamp}-{}",
                    NEXT_ACTION.fetch_add(1, Ordering::Relaxed)
                ),
                name: self.t("新快捷操作", "New quick action").into(),
                shortcut: String::new(),
                command: String::new(),
                enabled: false,
            },
            window,
            cx,
        ));
        self.editing_action = self.actions.last().map(|action| action.id.clone());
        cx.notify();
    }
    pub(super) fn action_editor(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = self.controls_locked();
        let saved = self.services.quick_actions().unwrap_or_default();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("add-action")
                            .label(self.t("添加操作", "Add action"))
                            .disabled(disabled)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.add_action(window, cx)),
                            ),
                    )
                    .child(
                        Button::new("default-actions")
                            .label(self.t("载入默认", "Load defaults"))
                            .disabled(disabled)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.actions = rotor_runtime::quick::default_actions()
                                    .into_iter()
                                    .map(|action| ActionFields::new(action, window, cx))
                                    .collect();
                                this.editing_action = None;
                                this.message = this
                                    .t("默认值已载入，保存后生效", "Defaults loaded; save to apply")
                                    .into();
                                cx.notify();
                            })),
                    ),
            )
            .children(self.actions.iter().enumerate().map(|(index, action)| {
                let id = action.id.clone();
                let record_id = id.clone();
                let editing = self.editing_action.as_ref() == Some(&id);
                let edit_id = id.clone();
                let toggle_id = id.clone();
                let delete_id = id.clone();
                let normalized = rotor_runtime::quick::normalize_actions(vec![action.value(cx)])
                    .ok()
                    .and_then(|mut actions| actions.pop());
                let runnable = normalized
                    .as_ref()
                    .is_some_and(|draft| draft.enabled && saved.iter().any(|saved| saved == draft));
                let controls = div()
                    .flex()
                    .gap_1()
                    .items_center()
                    .child(
                        Button::new(("toggle-action", index))
                            .selected(action.enabled)
                            .toggled(action.enabled)
                            .compact()
                            .label(if action.enabled {
                                self.t("启用", "On")
                            } else {
                                self.t("停用", "Off")
                            })
                            .disabled(disabled)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change_action(ActionChange::Toggle(toggle_id.clone()), cx);
                            })),
                    )
                    .child(
                        Button::new(("run-action", index))
                            .icon(IconName::Play)
                            .compact()
                            .accessibility_label(self.t("运行已保存操作", "Run saved action"))
                            .tooltip(self.t("运行已保存操作", "Run saved action"))
                            .disabled(disabled || !runnable || self.pending_run.is_some())
                            .on_click(cx.listener(move |this, _, _, cx| {
                                match this.services.run_quick_action(id.clone()) {
                                    Ok(id) => this.pending_run = Some(id),
                                    Err(error) => this.message = error,
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(("edit-action", index))
                            .icon(if editing {
                                IconName::Close
                            } else {
                                IconName::Settings2
                            })
                            .compact()
                            .accessibility_label(if editing {
                                self.t("取消编辑", "Cancel edit")
                            } else {
                                self.t("编辑操作", "Edit action")
                            })
                            .tooltip(if editing {
                                self.t("取消编辑", "Cancel edit")
                            } else {
                                self.t("编辑操作", "Edit action")
                            })
                            .selected(editing)
                            .disabled(disabled)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                if editing {
                                    this.cancel_action_edit(&edit_id, window, cx);
                                } else {
                                    this.editing_action = Some(edit_id.clone());
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        Button::new(("remove-action", index))
                            .icon(IconName::Delete)
                            .compact()
                            .accessibility_label(self.t("删除操作", "Delete action"))
                            .tooltip(self.t("删除操作", "Delete action"))
                            .disabled(disabled)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.change_action(ActionChange::Delete(delete_id.clone()), cx);
                            })),
                    );
                appearance::card(cx)
                    .p_3()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_1()
                                    .child(
                                        div()
                                            .truncate()
                                            .font_weight(FontWeight::BOLD)
                                            .child(action.name.read(cx).value()),
                                    )
                                    .child(
                                        appearance::caption(action.shortcut.read(cx).value(), cx)
                                            .truncate(),
                                    ),
                            )
                            .child(controls),
                    )
                    .when(!editing, |card| {
                        card.child(
                            appearance::caption(action.command.read(cx).value(), cx).truncate(),
                        )
                    })
                    .when(editing, |card| {
                        card.child(appearance::caption(self.t("名称", "Name"), cx))
                            .child(
                                Input::new(&action.name)
                                    .text_size(px(13.))
                                    .disabled(disabled),
                            )
                            .child(appearance::caption(self.t("快捷键", "Shortcut"), cx))
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        Input::new(&action.shortcut)
                                            .text_size(px(13.))
                                            .disabled(disabled),
                                    )
                                    .child(
                                        Button::new(("record-action", index))
                                            .label(self.t("录制", "Record"))
                                            .disabled(disabled)
                                            .on_click(cx.listener(move |this, _, window, cx| {
                                                this.start_recording(
                                                    Recording::Action(record_id.clone()),
                                                    window,
                                                    cx,
                                                )
                                            })),
                                    ),
                            )
                            .child(appearance::caption(self.t("命令", "Command"), cx))
                            .child(
                                Textarea::new(&action.command)
                                    .text_size(px(13.))
                                    .h(px(72.))
                                    .disabled(disabled),
                            )
                    })
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::recorded_key;
    use gpui_kit::Keystroke;
    #[test]
    fn recording_distinguishes_local_keys_from_global_chords() {
        assert!(recorded_key(&Keystroke::parse("s").unwrap(), false).is_err());
        assert!(
            recorded_key(&Keystroke::parse("s").unwrap(), true)
                .unwrap()
                .is_some()
        );
        assert!(
            recorded_key(&Keystroke::parse("ctrl-shift-s").unwrap(), false)
                .unwrap()
                .is_some()
        );
        assert!(
            recorded_key(&Keystroke::parse("escape").unwrap(), true)
                .unwrap()
                .is_some()
        );
    }
}
