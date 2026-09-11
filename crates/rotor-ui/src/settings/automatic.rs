use super::*;
use std::time::Duration;

impl SettingsView {
    pub(super) fn controls_locked(&self) -> bool {
        self.pending.is_some()
            || self.close_request.is_some()
            || matches!(
                self.update.phase,
                rotor_runtime::UpdatePhase::Installing | rotor_runtime::UpdatePhase::HandedOff
            )
    }

    pub(super) fn observe_field(
        &mut self,
        key: &'static str,
        force: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.close_request.is_some() {
            return;
        }
        let (value, composing, focused) = if key == "search_excluded_dirs" {
            let composing = self.excluded.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            });
            let input = self.excluded.read(cx);
            (
                input.value().to_string(),
                composing,
                input.focus_handle(cx).is_focused(window) && window.is_window_active(),
            )
        } else if let Some(field) = self.fields.iter().find(|field| field.key == key) {
            let composing = field.state.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            });
            let input = field.state.read(cx);
            (
                input.value().to_string(),
                composing,
                input.focus_handle(cx).is_focused(window) && window.is_window_active(),
            )
        } else {
            return;
        };
        if self
            .autosave
            .observe(key, &value, composing, focused, force)
        {
            match self
                .services
                .save_settings_coalesced(vec![(key.into(), value.clone())])
            {
                Ok(id) => {
                    self.autosave.accepted(key, &value, id);
                    self.message.clear();
                }
                Err(error) => {
                    self.autosave.rejected(key);
                    self.message = error;
                }
            }
            cx.notify();
        }
        self.check_composition_later(window, cx);
    }

    pub(super) fn observe_all_fields(
        &mut self,
        force: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keys = self
            .fields
            .iter()
            .map(|field| field.key)
            .collect::<Vec<_>>();
        for key in keys {
            self.observe_field(key, force, window, cx);
        }
        self.observe_field("search_excluded_dirs", force, window, cx);
    }

    fn check_composition_later(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.composition_check.is_some()
            || !self.autosave.has_composition()
            || self.close_request.is_some()
        {
            return;
        }
        // The current input engine can unmark a composition without emitting
        // Change. Poll only while a composition exists, never while idle.
        self.composition_check = Some(cx.spawn_in(window, async move |view, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.composition_check = None;
                this.observe_all_fields(false, window, cx);
            });
        }));
    }

    pub(super) fn apply_saved_fields(
        &mut self,
        fields: Vec<autosave::SavedField>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        for field in fields {
            let Some(value) = self.config.get(field.key).cloned() else {
                continue;
            };
            if field.key == "search_excluded_dirs" {
                let composing = self.excluded.update(cx, |input, cx| {
                    input.marked_text_range(window, cx).is_some()
                });
                let current = self.excluded.read(cx).value();
                if !composing && current.as_ref() == field.submitted {
                    self.autosave.normalized(field.key, value.clone());
                    if current.as_ref() != value {
                        self.excluded
                            .update(cx, |input, cx| input.set_value(value, window, cx));
                    }
                }
            } else if let Some(input) = self
                .fields
                .iter()
                .find(|input| input.key == field.key)
                .map(|input| input.state.clone())
            {
                let composing = input.update(cx, |input, cx| {
                    input.marked_text_range(window, cx).is_some()
                });
                let current = input.read(cx).value();
                if !composing && current.as_ref() == field.submitted {
                    self.autosave.normalized(field.key, value.clone());
                    if current.as_ref() != value {
                        input.update(cx, |input, cx| input.set_value(value, window, cx));
                    }
                }
            }
        }
    }

    pub(super) fn saved_feedback(&mut self) {
        self.message = if self.pending.is_some() || self.autosave.has_pending() {
            ""
        } else if self.autosave.has_failures() || self.manual_failed {
            self.t(
                "部分设置未保存，请检查并重试",
                "Some settings were not saved; review and retry",
            )
        } else {
            ""
        }
        .into();
    }

    pub fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_close(CloseTarget::Window, window, cx);
    }
    pub fn request_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_close(CloseTarget::Application, window, cx);
    }
    pub fn waiting_to_quit(&self) -> bool {
        self.close_request == Some(CloseTarget::Application)
    }
    fn begin_close(&mut self, target: CloseTarget, window: &mut Window, cx: &mut Context<Self>) {
        // A later window-close event must not downgrade an application quit
        // already waiting for its save receipts (including updater handoff).
        let target = if self.close_request == Some(CloseTarget::Application) {
            CloseTarget::Application
        } else {
            target
        };
        self.recording = None;
        self.services.set_shortcut_recording(false);
        self.observe_all_fields(true, window, cx);
        if self.autosave.has_composition() || self.action_has_marked_text(window, cx) {
            self.message = self
                .t("请先完成输入法组字", "Finish composing text before closing")
                .into();
            cx.notify();
            return;
        }
        self.flush_actions(window, cx);
        self.close_request = Some(target);
        self.last_close_target = target;
        self.settle_close(window, cx);
        cx.notify();
    }
    pub(super) fn settle_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_request.is_some() && self.action_save.is_some() {
            if self.pending.is_some() {
                return;
            }
            self.close_request = None;
            self.flush_actions(window, cx);
            self.close_request = Some(self.last_close_target);
            if self.action_save.is_some() {
                self.close_request = None;
                self.manual_failed = true;
                self.message = self
                    .t("请先完成输入法组字", "Finish composing text before closing")
                    .into();
                return;
            }
        }
        let state = self
            .autosave
            .close_state(self.pending.is_some(), self.manual_failed);
        if state == autosave::CloseState::Waiting {
            return;
        }
        let Some(target) = self.close_request else {
            return;
        };
        if state == autosave::CloseState::Failed {
            self.close_request = None;
            if self.message.is_empty() {
                self.message = self
                    .t(
                        "仍有设置未保存，请重试或放弃后关闭",
                        "Unsaved changes remain; retry or discard before closing",
                    )
                    .into();
            }
            return; // Keep the failed draft visible and allow explicit retry/discard.
        }
        // Keep observers blocked through blur/destruction notifications.
        match target {
            CloseTarget::Window => window.remove_window(),
            CloseTarget::Application => cx.quit(),
        }
    }
    pub(super) fn discard_and_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending.is_some() || self.autosave.has_pending() {
            return;
        }
        // In particular, blur must not retry a draft the user just discarded.
        self.action_save = None;
        self.close_request = Some(self.last_close_target);
        match self.last_close_target {
            CloseTarget::Window => window.remove_window(),
            CloseTarget::Application => cx.quit(),
        }
    }
}
