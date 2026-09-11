use super::action_change::{ActionChange, plan_action_change};
use super::*;
use crate::shortcut::recorded_key;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::ButtonCustomVariant;
use rotor_runtime::QuickAction;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static NEXT_ACTION: AtomicU64 = AtomicU64::new(1);

fn action_icon_button(button: Button, danger: bool, cx: &App) -> Button {
    let colors = appearance::palette(cx);
    button
        .compact()
        .w(px(32.))
        .h(px(32.))
        .rounded(px(16.))
        .custom(
            ButtonCustomVariant::new(cx)
                .foreground(if danger {
                    cx.theme().danger
                } else {
                    colors.secondary
                })
                .hover(colors.foreground.opacity(0.08))
                .active(colors.foreground.opacity(0.12)),
        )
}

fn shortcut_label(value: &str) -> String {
    let parts = value.split('+').collect::<Vec<_>>();
    let mut labels = Vec::new();
    for (aliases, label) in [
        (&["ctrl", "control"][..], "Ctrl"),
        (&["alt", "option"][..], "Alt"),
        (&["shift"][..], "Shift"),
        (&["super", "meta", "cmd", "command", "win"][..], "Super"),
    ] {
        if parts
            .iter()
            .any(|part| aliases.iter().any(|alias| part.eq_ignore_ascii_case(alias)))
        {
            labels.push(label.to_owned());
        }
    }
    for part in parts {
        if [
            "ctrl", "control", "alt", "option", "shift", "super", "meta", "cmd", "command", "win",
        ]
        .iter()
        .any(|modifier| part.eq_ignore_ascii_case(modifier))
        {
            continue;
        }
        let key = part
            .strip_prefix("Key")
            .or_else(|| part.strip_prefix("Digit"))
            .unwrap_or(part);
        labels.push(if key.len() == 1 {
            key.to_uppercase()
        } else {
            key.to_owned()
        });
    }
    labels.join("+")
}
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
            cx.subscribe_in(&name, window, |this, _, event, window, cx| {
                if matches!(event, gpui_kit::component::input::InputEvent::Change) {
                    this.schedule_action_save(window, cx);
                }
            }),
            cx.subscribe_in(&shortcut, window, |this, _, event, window, cx| {
                if matches!(event, gpui_kit::component::input::InputEvent::Change) {
                    this.schedule_action_save(window, cx);
                }
            }),
            cx.subscribe_in(&command, window, |this, _, event, window, cx| {
                if matches!(event, gpui_kit::component::input::InputEvent::Change) {
                    this.schedule_action_save(window, cx);
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
            self.flush_actions(window, cx);
        }
        cx.notify();
    }
    pub(super) fn sync_saved_fields(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending_keys.iter().any(|key| key == "quick_actions")
            && let Ok(actions) = self.services.quick_actions()
        {
            if self
                .actions
                .iter()
                .map(|action| &action.id)
                .eq(actions.iter().map(|action| &action.id))
            {
                for (field, saved) in self.actions.iter_mut().zip(actions) {
                    field.enabled = saved.enabled;
                }
                return;
            }
            self.actions = actions
                .into_iter()
                .map(|action| ActionFields::new(action, window, cx))
                .collect();
            self.editing_action = None;
        }
    }
    fn schedule_action_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.close_request.is_some() {
            return;
        }
        self.action_save = Some(cx.spawn_in(window, async move |view, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(350))
                .await;
            let _ = view.update_in(cx, |this, window, cx| {
                // Closing drains this draft after the in-flight receipt arrives.
                if this.close_request.is_some() {
                    return;
                }
                this.action_save = None;
                if this.pending.is_some() || this.action_has_marked_text(window, cx) {
                    this.schedule_action_save(window, cx);
                } else {
                    this.flush_actions(window, cx);
                }
            });
        }));
        cx.notify();
    }
    pub(super) fn flush_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending.is_some() || self.action_has_marked_text(window, cx) {
            self.schedule_action_save(window, cx);
            return;
        }
        self.action_save = None;
        let actions = self
            .actions
            .iter()
            .map(|action| action.value(cx))
            .collect::<Vec<_>>();
        let normalized = rotor_runtime::quick::normalize_actions(actions.clone());
        if normalized
            .as_ref()
            .is_ok_and(|actions| self.services.quick_actions().as_ref() == Ok(actions))
        {
            self.manual_failed = false;
            return;
        }
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
                self.message.clear();
            }
            Err(error) => {
                self.manual_failed = true;
                self.message = error;
            }
        }
        cx.notify();
    }
    fn change_action(&mut self, change: ActionChange, window: &mut Window, cx: &mut Context<Self>) {
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
            self.flush_actions(window, cx);
            cx.notify();
        }
    }
    fn collapse_action_edit(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.controls_locked() || self.editing_action.as_deref() != Some(id) {
            return;
        }
        self.editing_action = None;
        self.recording = None;
        self.services.set_shortcut_recording(false);
        self.flush_actions(window, cx);
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
        self.flush_actions(window, cx);
        cx.notify();
    }
    pub(super) fn action_editor(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = self.controls_locked();
        let saved = self.services.quick_actions().unwrap_or_default();
        let header = self.action_header(disabled, cx);
        let mut page = div().flex().flex_col().min_w_0().gap(px(6.)).child(header);
        for (index, action) in self.actions.iter().enumerate() {
            let card = self.action_card(index, action, &saved, disabled, cx);
            page = page.child(card);
        }
        page
    }

    fn action_header(&self, disabled: bool, cx: &mut Context<Self>) -> Div {
        let enabled = self.actions.iter().filter(|action| action.enabled).count();
        let title = div()
            .text_size(px(14.))
            .font_weight(FontWeight::BOLD)
            .child(self.t("快捷操作", "Quick actions"));
        let count = appearance::caption(format!("{enabled}/{}", self.actions.len()), cx);
        let colors = appearance::palette(cx);
        let add_icon = div()
            .id("add-action-icon")
            .flex()
            .items_center()
            .when(!disabled, |icon| {
                icon.group_hover("add-action-button", |style| style.text_color(colors.accent))
            })
            .child(Icon::new(IconName::CirclePlus).size(px(18.)));
        let add = Button::new("add-action")
            .group("add-action-button")
            .custom(ButtonCustomVariant::new(cx).foreground(colors.secondary))
            .compact()
            .child(add_icon)
            .accessibility_label(self.t("添加操作", "Add action"))
            .tooltip(self.t("添加操作", "Add action"))
            .disabled(disabled)
            .on_click(cx.listener(|this, _, window, cx| this.add_action(window, cx)));
        div()
            .flex()
            .items_center()
            .gap_2()
            .pb(px(4.))
            .border_b_1()
            .border_color(appearance::palette(cx).border)
            .child(title)
            .child(count)
            .child(div().flex_1())
            .child(add)
    }
    // Separate builder frames keep expanded editors within the Windows main stack.
    fn action_card(
        &self,
        index: usize,
        action: &ActionFields,
        saved: &[QuickAction],
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let editing = self.editing_action.as_ref() == Some(&action.id);
        let controls = self.action_controls(index, action, saved, disabled, cx);
        let toggle_id = action.id.clone();
        let description = self.action_description(action, cx);
        let toggle = gpui_kit::component::switch::Switch::new(("toggle-action", index))
            .checked(action.enabled)
            .accessibility_label(self.t("启用操作", "Enable action"))
            .disabled(disabled)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.change_action(ActionChange::Toggle(toggle_id.clone()), window, cx);
            }));
        let row = div()
            .flex()
            .items_center()
            .gap_2()
            .min_h(px(38.))
            .child(toggle)
            .child(description)
            .child(controls);
        let mut card = appearance::card(cx)
            .border_0()
            .px(px(10.))
            .py(px(8.))
            .child(row);
        if editing {
            let fields = self.action_fields(index, action, cx);
            card = card.child(fields);
        }
        card
    }

    fn action_description(&self, action: &ActionFields, cx: &App) -> Div {
        let colors = appearance::palette(cx);
        let shortcut = shortcut_label(&action.shortcut.read(cx).value());
        let name = div()
            .min_w_0()
            .truncate()
            .font_weight(FontWeight::BOLD)
            .child(action.name.read(cx).value());
        let mut title = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .min_w_0()
            .child(name);
        if !shortcut.is_empty() {
            let badge = div()
                .max_w_full()
                .truncate()
                .px(px(6.))
                .py(px(2.))
                .rounded_sm()
                .text_size(px(12.))
                .text_color(colors.accent)
                .bg(colors.accent.opacity(0.12))
                .child(shortcut);
            title = title.child(badge);
        }
        let command = appearance::caption(action.command.read(cx).value(), cx).truncate();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .gap_1()
            .child(title)
            .child(command)
    }

    fn action_controls(
        &self,
        index: usize,
        action: &ActionFields,
        saved: &[QuickAction],
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let id = action.id.clone();
        let editing = self.editing_action.as_ref() == Some(&id);
        let edit_id = id.clone();
        let delete_id = id.clone();
        let normalized = rotor_runtime::quick::normalize_actions(vec![action.value(cx)])
            .ok()
            .and_then(|mut actions| actions.pop());
        let runnable = normalized
            .as_ref()
            .is_some_and(|draft| draft.enabled && saved.iter().any(|saved| saved == draft));
        let run = action_icon_button(Button::new(("run-action", index)), false, cx)
            .icon(IconName::Play)
            .accessibility_label(self.t("运行已保存操作", "Run saved action"))
            .tooltip(self.t("运行已保存操作", "Run saved action"))
            .disabled(disabled || !runnable || self.pending_run.is_some())
            .on_click(cx.listener(move |this, _, _, cx| {
                match this.services.run_quick_action(id.clone()) {
                    Ok(id) => this.pending_run = Some(id),
                    Err(error) => this.message = error,
                }
                cx.notify();
            }));
        let edit = action_icon_button(Button::new(("edit-action", index)), false, cx)
            .when(editing, |button| button.icon(IconName::Close))
            .when(!editing, |button| {
                button.child(Icon::new(IconName::Pencil).size(px(16.)))
            })
            .accessibility_label(if editing {
                self.t("收起编辑", "Collapse editor")
            } else {
                self.t("编辑操作", "Edit action")
            })
            .tooltip(if editing {
                self.t("收起编辑", "Collapse editor")
            } else {
                self.t("编辑操作", "Edit action")
            })
            .disabled(disabled)
            .on_click(cx.listener(move |this, _, window, cx| {
                if editing {
                    this.collapse_action_edit(&edit_id, window, cx);
                } else {
                    this.editing_action = Some(edit_id.clone());
                    cx.notify();
                }
            }));
        let delete = action_icon_button(Button::new(("remove-action", index)), true, cx)
            .child(Icon::new(IconName::Trash).size(px(16.)))
            .accessibility_label(self.t("删除操作", "Delete action"))
            .tooltip(self.t("删除操作", "Delete action"))
            .disabled(disabled)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.change_action(ActionChange::Delete(delete_id.clone()), window, cx);
            }));
        div()
            .flex()
            .flex_shrink_0()
            .gap_2()
            .items_center()
            .child(run)
            .child(edit)
            .child(delete)
    }

    fn action_fields(&self, index: usize, action: &ActionFields, cx: &mut Context<Self>) -> Div {
        let name = appearance::control_row(
            self.t("名称", "Name"),
            Input::new(&action.name)
                .text_size(px(13.))
                .aria_label(self.t("名称", "Name"))
                .disabled(self.controls_locked()),
        );
        let shortcut = self.action_shortcut(index, action, cx);
        let command = appearance::control_row(
            self.t("命令", "Command"),
            Textarea::new(&action.command)
                .text_size(px(13.))
                .h(px(72.))
                .disabled(self.controls_locked()),
        );
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(name)
            .child(shortcut)
            .child(command)
    }

    fn action_shortcut(&self, index: usize, action: &ActionFields, cx: &mut Context<Self>) -> Div {
        let disabled = self.controls_locked();
        let record_id = action.id.clone();
        let clear_shortcut = action.shortcut.clone();
        let record = appearance::control_button(Button::new(("record-action", index)), cx)
            .label(if matches!(&self.recording, Some(Recording::Action(target)) if target == &record_id) {
                self.t("请按快捷键…", "Press a shortcut…").into()
            } else { action.shortcut.read(cx).value() })
            .accessibility_label(self.t("快捷键", "Shortcut"))
            .tooltip(self.t("点击录制快捷键，Esc 取消", "Click to record a shortcut; Esc cancels"))
            .disabled(disabled)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.start_recording(Recording::Action(record_id.clone()), window, cx)
            }));
        let clear = appearance::quiet_button(Button::new(("clear-action-shortcut", index)), cx)
            .icon(IconName::Close)
            .accessibility_label(self.t("清除快捷键", "Clear shortcut"))
            .tooltip(self.t("清除快捷键", "Clear shortcut"))
            .disabled(disabled || action.shortcut.read(cx).value().is_empty())
            .on_click(cx.listener(move |this, _, window, cx| {
                clear_shortcut.update(cx, |input, cx| input.set_value("", window, cx));
                this.schedule_action_save(window, cx);
            }));
        appearance::control_row(
            self.t("快捷键", "Shortcut"),
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(div().flex_1().min_w_0().child(record))
                .child(clear),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::recorded_key;
    use gpui_kit::Keystroke;
    #[test]
    fn action_edits_autosave_without_replacing_the_editor() {
        use super::*;
        use gpui::AppContext;
        use gpui_kit::component::Root;
        use rotor_common::ConfigService;
        use rotor_runtime::{ServiceOptions, Services};
        use std::{
            sync::Mutex,
            time::{Duration, Instant},
        };

        let profile = tempfile::tempdir().unwrap();
        let config = ConfigService::load_from(profile.path()).unwrap();
        let (services, events) = Services::new(
            Arc::new(Mutex::new(config)),
            None,
            ServiceOptions { index_files: false },
        )
        .unwrap();
        let services = Arc::new(services);
        let mut app = gpui::TestAppContext::single();
        app.update(gpui_kit::component::init);
        let mut view = None;
        let (_, cx) = app.add_window_view(|window, cx| {
            let settings =
                cx.new(|cx| SettingsView::new(services.settings(), services.clone(), window, cx));
            view = Some(settings.clone());
            Root::new(settings, window, cx)
        });
        let view = view.unwrap();
        let receive_save = |cx: &mut gpui::VisualTestContext| {
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                assert!(Instant::now() < deadline, "settings receipt timed out");
                if let Ok(event) = events.try_recv() {
                    let saved = matches!(&event, RuntimeEvent::SettingsSaved { .. });
                    cx.update(|window, cx| {
                        view.update(cx, |view, cx| view.handle_event(event, window, cx))
                    });
                    if saved {
                        break;
                    }
                } else {
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        };
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.add_action(window, cx);
                assert!(view.pending.is_some());
            })
        });
        receive_save(cx);
        let (id, name) = view.read_with(cx, |view, _| {
            let action = view.actions.last().unwrap();
            assert_eq!(view.editing_action.as_ref(), Some(&action.id));
            (action.id.clone(), action.name.clone())
        });
        cx.update(|window, cx| {
            name.update(cx, |name, cx| {
                name.set_value("Synthetic action", window, cx);
                cx.emit(InputEvent::Change);
            })
        });
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(400));
        cx.run_until_parked();
        receive_save(cx);
        assert!(
            services
                .quick_actions()
                .unwrap()
                .iter()
                .any(|action| action.id == id && action.name == "Synthetic action")
        );
        view.read_with(cx, |view, _| {
            assert_eq!(view.editing_action.as_ref(), Some(&id));
            assert_eq!(view.actions.last().unwrap().name, name);
        });
        services.shutdown();
    }

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
