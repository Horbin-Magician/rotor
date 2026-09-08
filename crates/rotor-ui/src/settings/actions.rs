use super::*;
use crate::shortcut::recorded_key;
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
}
impl ActionFields {
    pub(super) fn new(
        action: QuickAction,
        window: &mut Window,
        cx: &mut Context<SettingsView>,
    ) -> Self {
        Self {
            id: action.id,
            name: cx.new(|cx| InputState::new(window, cx).default_value(action.name)),
            shortcut: cx.new(|cx| InputState::new(window, cx).default_value(action.shortcut)),
            command: cx.new(|cx| TextareaState::new(window, cx).default_value(action.command)),
            enabled: action.enabled,
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
    pub(super) fn start_recording(
        &mut self,
        target: Recording,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        self.message = self
            .t("已录入，保存后生效", "Recorded; save to apply")
            .into();
        cx.notify();
    }
    pub(super) fn sync_saved_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for field in &self.fields {
            if self.pending_keys.iter().any(|key| key == field.key) {
                let value = self.config.get(field.key).cloned().unwrap_or_default();
                field
                    .state
                    .update(cx, |field, cx| field.set_value(value, window, cx));
            }
        }
        if self.pending_keys.iter().any(|key| key == "quick_actions")
            && let Ok(actions) = self.services.quick_actions()
        {
            self.actions = actions
                .into_iter()
                .map(|action| ActionFields::new(action, window, cx))
                .collect();
        }
    }
    pub(super) fn save_actions(&mut self, cx: &mut Context<Self>) {
        if self.pending.is_some() {
            return;
        }
        let actions = self.actions.iter().map(|action| action.value(cx)).collect();
        match self.services.save_quick_actions(actions) {
            Ok(id) => {
                self.pending = Some(id);
                self.pending_keys = vec!["quick_actions".into()];
                self.message = self.t("正在保存…", "Saving…").into();
            }
            Err(error) => self.message = error,
        }
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
        cx.notify();
    }
    pub(super) fn action_editor(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = self.pending.is_some();
        let saved = self.services.quick_actions().unwrap_or_default();
        div()
            .flex()
            .flex_col()
            .gap_4()
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
                let normalized = rotor_runtime::quick::normalize_actions(vec![action.value(cx)])
                    .ok()
                    .and_then(|mut actions| actions.pop());
                let runnable = normalized
                    .as_ref()
                    .is_some_and(|draft| draft.enabled && saved.iter().any(|saved| saved == draft));
                crate::visual::card(cx)
                    .child(crate::visual::caption(self.t("名称", "Name"), cx))
                    .child(Input::new(&action.name).disabled(disabled))
                    .child(crate::visual::caption(self.t("快捷键", "Shortcut"), cx))
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(Input::new(&action.shortcut).disabled(disabled))
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
                    .child(crate::visual::caption(self.t("命令", "Command"), cx))
                    .child(Textarea::new(&action.command).h(px(72.)).disabled(disabled))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                Button::new(("toggle-action", index))
                                    .selected(action.enabled)
                                    .label(if action.enabled {
                                        self.t("已启用", "Enabled")
                                    } else {
                                        self.t("已停用", "Disabled")
                                    })
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(action) = this.actions.get_mut(index) {
                                            action.enabled = !action.enabled;
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new(("run-action", index))
                                    .label(self.t("运行已保存操作", "Run saved action"))
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
                                Button::new(("remove-action", index))
                                    .label(self.t("删除", "Delete"))
                                    .disabled(disabled)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if index < this.actions.len() {
                                            this.actions.remove(index);
                                        }
                                        cx.notify();
                                    })),
                            ),
                    )
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
