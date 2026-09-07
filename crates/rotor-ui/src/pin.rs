use super::PreparedImage;
use gpui_kit::{
    component::{Disableable, button::Button},
    prelude::*,
    *,
};
use rotor_common::Config;
use rotor_runtime::{
    OperationId, PinEvent, PinExportTarget, RuntimeEvent, Services, ShotterConfig,
};
use std::{
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub type PinPositionReader = Rc<dyn Fn(&Window) -> Option<(i32, i32)>>;
pub struct PinInit {
    pub image: PreparedImage,
    pub config: ShotterConfig,
    pub id: Option<u32>,
    pub pending: Option<OperationId>,
    pub error: Option<String>,
    pub position: PinPositionReader,
}
pub struct PinView {
    services: Arc<Services>,
    settings: Config,
    image: PreparedImage,
    record: ShotterConfig,
    id: Option<u32>,
    pending_create: Option<OperationId>,
    pending_update: Option<OperationId>,
    pending_finish: Option<OperationId>,
    remember_directory: Option<String>,
    dialog: bool,
    hovered: bool,
    dirty: bool,
    message: String,
    focus: FocusHandle,
    position: PinPositionReader,
    save_task: Option<Task<()>>,
    _bounds: Subscription,
}
impl PinView {
    pub fn new(
        services: Arc<Services>,
        init: PinInit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let bounds =
            cx.observe_window_bounds(window, |this, window, cx| this.record_position(window, cx));
        Self {
            settings: services.settings(),
            services,
            image: init.image,
            record: init.config,
            id: init.id,
            pending_create: init.pending,
            pending_update: None,
            pending_finish: None,
            remember_directory: None,
            dialog: false,
            hovered: false,
            dirty: false,
            message: init.error.unwrap_or_default(),
            focus,
            position: init.position,
            save_task: None,
            _bounds: bounds,
        }
    }
    fn t(&self, zh: &'static str, en: &'static str) -> &'static str {
        if rotor_common::i18n::language_for_config(&self.settings) == "zh-CN" {
            zh
        } else {
            en
        }
    }
    fn busy(&self) -> bool {
        self.pending_create.is_some() || self.pending_finish.is_some() || self.dialog
    }
    pub fn config(&self) -> &ShotterConfig {
        &self.record
    }
    pub fn persisted_id(&self) -> Option<u32> {
        self.id
    }
    pub fn start_persistence(&mut self, cx: &mut Context<Self>) {
        if self.id.is_some() || self.pending_create.is_some() {
            return;
        }
        match self
            .services
            .create_pin(self.image.image.clone(), self.record.clone())
        {
            Ok(id) => self.pending_create = Some(id),
            Err(error) => self.message = error,
        }
        cx.notify();
    }
    pub fn source(&self) -> &PreparedImage {
        &self.image
    }
    fn crop(&self) -> (u32, u32, u32, u32) {
        rotor_runtime::pin_source_crop(
            &self.record,
            self.image.image.width(),
            self.image.image.height(),
        )
        .unwrap_or((0, 0, 0, 0))
    }
    pub fn reveal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.record.minimized = false;
        self.dirty = true;
        self.flush(cx);
        window.activate_window();
        self.focus.focus(window, cx);
        cx.notify();
    }
    fn record_position(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || self.record.minimized {
            return;
        }
        if let Some((x, y)) = (self.position)(window) {
            let offset_x = x as i64 - self.record.monitor_pos.0 as i64 - self.record.rect.0 as i64;
            let offset_y = y as i64 - self.record.monitor_pos.1 as i64 - self.record.rect.1 as i64;
            if let Some(display) = window.display(cx) {
                let label = format!("ssmask-{}", u64::from(display.id()) as u32);
                if self.record.mask_label != label {
                    self.record.mask_label = label;
                    self.dirty = true;
                }
            }
            if let (Ok(x), Ok(y)) = (i32::try_from(offset_x), i32::try_from(offset_y))
                && self.record.offset != (x, y)
            {
                self.record.offset = (x, y);
                self.dirty = true;
            }
        }
        if self.dirty {
            self.save_task = Some(cx.spawn_in(window, async move |view, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                let _ = view.update(cx, |view, cx| view.flush(cx));
            }));
        }
    }
    pub fn flush(&mut self, cx: &mut Context<Self>) {
        if !self.dirty {
            return;
        }
        if let Some(id) = self.id {
            match self.services.update_pin(id, self.record.clone()) {
                Ok(request) => {
                    self.pending_update = Some(request);
                    self.dirty = false;
                }
                Err(error) => self.message = error,
            }
        }
        cx.notify();
    }
    pub fn persist_geometry(&mut self, cx: &mut Context<Self>) {
        self.dirty = true;
        self.flush(cx);
    }
    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        if let Some(id) = self.id {
            match self.services.delete_pin(id) {
                Ok(request) => self.pending_finish = Some(request),
                Err(error) => self.message = error,
            }
        } else {
            window.remove_window();
        }
        cx.notify();
    }
    fn export(&mut self, target: PinExportTarget, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.flush(cx);
        self.remember_directory = match &target {
            PinExportTarget::File(path)
                if self
                    .settings
                    .get("if_auto_change_save_path")
                    .is_none_or(|value| value != "false") =>
            {
                path.parent()
                    .and_then(|path| path.to_str())
                    .map(str::to_owned)
            }
            _ => None,
        };
        match self.services.export_pin(
            self.id,
            self.image.image.clone(),
            self.record.clone(),
            target,
        ) {
            Ok(request) => {
                self.pending_finish = Some(request);
                self.message = self.t("正在导出…", "Exporting…").into();
            }
            Err(error) => self.message = error,
        }
        cx.notify();
    }
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let name = format!("Rotor_{stamp}.png");
        let configured = self
            .settings
            .get("save_path")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        if self
            .settings
            .get("if_ask_save_path")
            .is_some_and(|value| value == "false")
            && let Some(directory) = configured.as_ref()
        {
            self.export(PinExportTarget::File(directory.join(name)), cx);
            return;
        }
        let directory = configured.or_else(std::env::home_dir).unwrap_or_default();
        let receiver = cx.prompt_for_new_path(&directory, Some(&name));
        self.dialog = true;
        cx.notify();
        cx.spawn_in(window, async move |view, cx| {
            let result = receiver.await;
            let _ = view.update(cx, |this, cx| {
                this.dialog = false;
                match result {
                    Ok(Ok(Some(path))) => {
                        if path
                            .extension()
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
                        {
                            this.export(PinExportTarget::File(path), cx);
                        } else {
                            this.message =
                                this.t("请使用 .png 文件名", "Use a .png filename").into();
                        }
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => this.message = error.to_string(),
                    Err(error) => this.message = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn hide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.record.minimized = true;
        self.dirty = true;
        self.flush(cx);
        window.minimize_window();
    }
    fn zoom(&mut self, delta: f32, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || delta == 0. {
            return;
        }
        let step = self
            .settings
            .get("zoom_delta")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(2)
            .clamp(1, 25);
        let (_, _, width, height) = self.crop();
        let maximum = (8192. / width.max(height).max(1) as f32 * 100.)
            .floor()
            .clamp(1., 500.) as i64;
        let factor = (self.record.zoom_factor as i64
            + step as i64 * if delta > 0. { 1 } else { -1 })
        .clamp(5.min(maximum), maximum) as u32;
        self.record.zoom_factor = factor;
        self.dirty = true;
        let scale = factor as f32 / 100. / window.scale_factor();
        window.resize(size(px(width as f32 * scale), px(height as f32 * scale)));
        self.record_position(window, cx);
        cx.notify();
    }
    pub fn handle_event(
        &mut self,
        event: &RuntimeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            RuntimeEvent::Pin(PinEvent::Created { id, result })
                if self.pending_create == Some(*id) =>
            {
                self.pending_create = None;
                match result {
                    Ok(pin) => {
                        self.id = Some(pin.id);
                        self.message.clear();
                    }
                    Err(error) => {
                        self.message = format!(
                            "{}: {error}",
                            self.t(
                                "贴图未持久化，仍可保存或复制",
                                "Pin is not persisted; Save or Copy is still available"
                            )
                        )
                    }
                }
            }
            RuntimeEvent::Pin(PinEvent::Updated { id, result, .. })
                if self.pending_update == Some(*id) =>
            {
                self.pending_update = None;
                if let Err(error) = result {
                    self.dirty = true;
                    self.message = error.clone();
                }
            }
            RuntimeEvent::Pin(
                PinEvent::Deleted { id, result, .. } | PinEvent::Exported { id, result },
            ) if self.pending_finish == Some(*id) => {
                self.pending_finish = None;
                match result {
                    Ok(()) => {
                        if let Some(directory) = self.remember_directory.take()
                            && let Err(error) = self
                                .services
                                .save_settings(vec![("save_path".into(), directory)])
                        {
                            eprintln!("Save directory: {error}");
                        }
                        window.remove_window();
                    }
                    Err(error) => self.message = error.clone(),
                }
            }
            RuntimeEvent::SettingsSaved {
                result: Ok(config), ..
            } => self.settings = config.clone(),
            _ => return,
        }
        cx.notify();
    }
}
fn shortcut_matches(event: &Keystroke, configured: Option<&String>) -> bool {
    let Some(configured) = configured else {
        return false;
    };
    let normalized = configured
        .to_ascii_lowercase()
        .replace("command", "cmd")
        .replace("control", "ctrl")
        .replace('+', "-");
    Keystroke::parse(&normalized).is_ok_and(|key| {
        key.modifiers == event.modifiers && key.key.eq_ignore_ascii_case(&event.key)
    })
}
impl Render for PinView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (x, y, _, _) = self.crop();
        let scale = self.record.zoom_factor as f32 / 100. / window.scale_factor();
        let busy = self.busy();
        div()
            .id("pin")
            .track_focus(&self.focus)
            .size_full()
            .overflow_hidden()
            .child(
                div()
                    .size_full()
                    .overflow_hidden()
                    .child(
                        img(self.image.render.clone())
                            .absolute()
                            .left(px(-(x as f32) * scale))
                            .top(px(-(y as f32) * scale))
                            .w(px(self.image.image.width() as f32 * scale))
                            .h(px(self.image.image.height() as f32 * scale)),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, _| {
                            if !this.busy() {
                                window.start_window_move();
                            }
                        }),
                    ),
            )
            .when(
                self.hovered || !self.message.is_empty() || self.pending_create.is_some(),
                |root| {
                    root.child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .flex()
                            .flex_col()
                            .max_w_full()
                            .bg(rgba(0x222222dd))
                            .text_color(rgba(0xffffffff))
                            .child(
                                div()
                                    .flex()
                                    .child(
                                        Button::new("pin-save")
                                            .label("S")
                                            .tooltip(self.t("保存", "Save"))
                                            .compact()
                                            .disabled(busy)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.save(window, cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("pin-copy")
                                            .label("C")
                                            .tooltip(self.t("复制", "Copy"))
                                            .compact()
                                            .disabled(busy)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.export(PinExportTarget::Clipboard, cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("pin-hide")
                                            .label("H")
                                            .tooltip(self.t("隐藏", "Hide"))
                                            .compact()
                                            .disabled(busy)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.hide(window, cx)
                                            })),
                                    )
                                    .child(
                                        Button::new("pin-close")
                                            .label("×")
                                            .tooltip(self.t("关闭", "Close"))
                                            .compact()
                                            .disabled(busy)
                                            .on_click(cx.listener(|this, _, window, cx| {
                                                this.close(window, cx)
                                            })),
                                    ),
                            )
                            .child(div().text_xs().child(self.message.clone())),
                    )
                },
            )
            .on_mouse_move(cx.listener(|this, _, _, cx| {
                if !this.hovered {
                    this.hovered = true;
                    cx.notify();
                }
            }))
            .on_mouse_exit(cx.listener(|this, _, _, cx| {
                this.hovered = false;
                cx.notify();
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                this.zoom(event.delta.pixel_delta(px(16.)).y.as_f32(), window, cx);
                cx.stop_propagation();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if shortcut_matches(&event.keystroke, this.settings.get("shortcut_pinwin_save")) {
                    this.save(window, cx);
                } else if shortcut_matches(
                    &event.keystroke,
                    this.settings.get("shortcut_pinwin_copy"),
                ) {
                    this.export(PinExportTarget::Clipboard, cx);
                } else if shortcut_matches(
                    &event.keystroke,
                    this.settings.get("shortcut_pinwin_close"),
                ) {
                    this.close(window, cx);
                } else if shortcut_matches(
                    &event.keystroke,
                    this.settings.get("shortcut_pinwin_hide"),
                ) {
                    this.hide(window, cx);
                } else {
                    return;
                }
                cx.stop_propagation();
            }))
    }
}
