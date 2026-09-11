use super::PreparedImage;
mod annotation;
mod crop;
mod menu;
mod ocr;
mod toolbar;
use gpui_kit::{
    component::{ActiveTheme, Disableable, Selectable},
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
pub type PinMinimizedReader = Rc<dyn Fn(&Window) -> Option<bool>>;
#[derive(Clone, Copy, Debug)]
pub struct PinBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}
pub type PinBoundsSetter = Rc<dyn Fn(&Window, PinBounds) -> Result<(), String>>;
pub type PinPointerCapture = Rc<dyn Fn(&Window, bool) -> Result<(), String>>;
pub struct PinInit {
    pub image: PreparedImage,
    pub config: ShotterConfig,
    pub id: Option<u32>,
    pub pending: Option<OperationId>,
    pub error: Option<String>,
    pub position: PinPositionReader,
    pub minimized: PinMinimizedReader,
    pub content_scale: f32,
    pub bounds: PinBoundsSetter,
    pub pointer: PinPointerCapture,
}
enum ExportIntent {
    Save,
    Target(PinExportTarget),
}
struct PendingFinish {
    id: OperationId,
    // Only an accepted file export may update the remembered directory.
    // Keeping it with the request prevents a later Delete from reusing a
    // failed export's path.
    remember_directory: Option<String>,
}
pub struct PinView {
    services: Arc<Services>,
    settings: Config,
    image: PreparedImage,
    record: ShotterConfig,
    id: Option<u32>,
    pending_create: Option<OperationId>,
    pending_update: Option<OperationId>,
    pending_finish: Option<PendingFinish>,
    queued_export: Option<ExportIntent>,
    dialog: bool,
    preparing_export: bool,
    dirty: bool,
    message: String,
    focus: FocusHandle,
    position: PinPositionReader,
    minimized: PinMinimizedReader,
    save_task: Option<Task<()>>,
    _bounds: Subscription,
    _activation: Subscription,
    canvas: annotation::CanvasState,
    content_scale: f32,
    bounds: PinBoundsSetter,
    pointer: PinPointerCapture,
    pointer_owned: bool,
    move_drag: Option<crop::MoveDrag>,
    crop_drag: Option<crop::CropDrag>,
    crop_hover: rotor_canvas::CropEdges,
    ocr: ocr::OcrState,
}
impl PinView {
    pub fn new(
        services: Arc<Services>,
        init: PinInit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let settings = services.settings();
        window.set_window_title(&pin_title(&settings, init.id));
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let bounds =
            cx.observe_window_bounds(window, |this, window, cx| this.record_position(window, cx));
        let canvas = annotation::CanvasState::new(&init.image, &init.config);
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            this.sync_minimized(window, cx);
            if !window.is_window_active() {
                this.cancel_pointer(window, cx);
            }
            cx.notify();
        });
        Self {
            settings,
            services,
            image: init.image,
            record: init.config,
            id: init.id,
            pending_create: init.pending,
            pending_update: None,
            pending_finish: None,
            queued_export: None,
            dialog: false,
            preparing_export: false,
            dirty: false,
            message: init.error.unwrap_or_default(),
            focus,
            position: init.position,
            minimized: init.minimized,
            save_task: None,
            _bounds: bounds,
            _activation: activation,
            canvas,
            content_scale: init.content_scale,
            bounds: init.bounds,
            pointer: init.pointer,
            pointer_owned: false,
            move_drag: None,
            crop_drag: None,
            crop_hover: Default::default(),
            ocr: Default::default(),
        }
    }
    fn t(&self, zh: &'static str, en: &'static str) -> &'static str {
        if rotor_common::i18n::language_for_config(&self.settings) == "zh-CN" {
            zh
        } else {
            en
        }
    }
    fn shortcut_hint(&self, label: &'static str, key: &str) -> String {
        match self.settings.get(key).filter(|key| !key.is_empty()) {
            Some(shortcut) => format!("{label} · {shortcut}"),
            None => label.into(),
        }
    }
    fn busy(&self) -> bool {
        self.pending_create.is_some()
            || self.pending_finish.is_some()
            || self.dialog
            || self.preparing_export
            || self.queued_export.is_some()
            || self.ocr.loading()
    }
    pub fn config(&self) -> &ShotterConfig {
        &self.record
    }
    pub fn persisted_id(&self) -> Option<u32> {
        self.id
    }
    pub fn shutdown_record(&self) -> Option<(u32, ShotterConfig)> {
        self.id.map(|id| (id, self.committed_crop_record()))
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
        if let Some(input) = self.canvas.editor.clone() {
            input.update(cx, |input, cx| input.focus(window, cx));
        } else {
            self.focus.focus(window, cx);
        }
        cx.notify();
    }
    fn sync_minimized(&mut self, window: &Window, cx: &mut Context<Self>) {
        if let Some(minimized) = (self.minimized)(window)
            && self.record.minimized != minimized
        {
            self.record.minimized = minimized;
            self.dirty = true;
            self.flush(cx);
        }
    }
    fn record_position(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_minimized(window, cx);
        if self.busy() || self.record.minimized || self.crop_drag.is_some() {
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
        if !self.dirty || self.crop_drag.is_some() {
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
        self.queued_export = None;
        if self.busy() {
            return;
        }
        if let Some(id) = self.id {
            match self.services.delete_pin(id) {
                Ok(request) => {
                    self.pending_finish = Some(PendingFinish {
                        id: request,
                        remember_directory: None,
                    });
                }
                Err(error) => self.message = error,
            }
        } else {
            window.remove_window();
        }
        cx.notify();
    }
    fn export(&mut self, target: PinExportTarget, window: &mut Window, cx: &mut Context<Self>) {
        self.request_export(ExportIntent::Target(target), window, cx);
    }
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request_export(ExportIntent::Save, window, cx);
    }
    fn request_export(
        &mut self,
        intent: ExportIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.pending_finish.is_some()
            || self.dialog
            || self.preparing_export
            || self.queued_export.is_some()
            || self.crop_drag.is_some()
            || !self.canvas.can_request_export()
        {
            return;
        }
        self.ensure_canvas(window, cx);
        if self.pending_create.is_some() {
            self.queued_export = Some(intent);
            self.message = self.t("正在准备导出…", "Preparing export…").into();
            cx.notify();
            return;
        }
        self.finish_export_request(intent, window, cx);
    }
    fn resume_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.queued_export.is_none() || self.pending_create.is_some() {
            return;
        }
        if self.canvas.ready() {
            let intent = self.queued_export.take().unwrap();
            self.finish_export_request(intent, window, cx);
        } else {
            self.queued_export = None;
            self.message = self
                .canvas
                .error
                .clone()
                .unwrap_or_else(|| self.t("图像尚未就绪", "Image is not ready").into());
        }
    }
    fn finish_export_request(
        &mut self,
        intent: ExportIntent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let scene = self.canvas.export_scene();
        let output = rotor_canvas::ImageSize {
            width: scene.crop.width,
            height: scene.crop.height,
        };
        let source = self.image.image.clone();
        let services = self.services.clone();
        self.preparing_export = true;
        self.message = self.t("正在准备导出…", "Preparing export…").into();
        cx.notify();
        cx.spawn_in(window, async move |view, cx| {
            // Render the document at source resolution, independent of display zoom.
            let result = services.render_canvas(source, scene, output).await;
            let _ = cx.update(|window, cx| {
                view.update(cx, |this, cx| {
                    this.preparing_export = false;
                    match result {
                        Ok(image) => {
                            this.message.clear();
                            match intent {
                                ExportIntent::Save => this.save_ready(image, window, cx),
                                ExportIntent::Target(target) => {
                                    this.export_frame(target, image, cx)
                                }
                            }
                        }
                        Err(error) => this.message = error,
                    }
                    cx.notify();
                })
            });
        })
        .detach();
    }
    fn export_frame(
        &mut self,
        target: PinExportTarget,
        image: Arc<image::RgbaImage>,
        cx: &mut Context<Self>,
    ) {
        if self.pending_create.is_some() || self.pending_finish.is_some() || self.dialog {
            return;
        }
        self.flush(cx);
        let remember_directory = match &target {
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
        match self.services.export_pin_frame(self.id, image, target) {
            Ok(request) => {
                self.pending_finish = Some(PendingFinish {
                    id: request,
                    remember_directory,
                });
                self.message = self.t("正在导出…", "Exporting…").into();
            }
            Err(error) => self.message = error,
        }
        cx.notify();
    }
    fn save_ready(
        &mut self,
        frame: Arc<image::RgbaImage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // The accepted document snapshot is already rendered; display resizes
        // during that work must not discard the requested export.
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
            self.export_frame(PinExportTarget::File(directory.join(name)), frame, cx);
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
                            this.export_frame(PinExportTarget::File(path), frame, cx);
                        } else {
                            this.message =
                                this.t("请使用 .png 文件名", "Use a .png filename").into();
                        }
                    }
                    Ok(Ok(None)) => this.message.clear(),
                    Ok(Err(error)) => this.message = error.to_string(),
                    Err(error) => this.message = error.to_string(),
                }
                cx.notify();
            });
        })
        .detach();
    }
    fn minimize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        self.record.minimized = true;
        self.dirty = true;
        self.flush(cx);
        window.minimize_window();
    }
    fn zoom(&mut self, delta: f32, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || delta == 0. || self.crop_drag.is_some() || self.move_drag.is_some() {
            return;
        }
        let step = self
            .settings
            .get("zoom_delta")
            .and_then(|value| value.parse::<i32>().ok())
            .unwrap_or(2)
            .clamp(1, 25);
        let (_, _, width, height) = self.crop();
        let maximum = (8192. / width.max(height).max(1) as f32 * self.content_scale
            / window.scale_factor()
            * 100.)
            .floor()
            .clamp(1., 500.) as i64;
        let factor = (self.record.zoom_factor as i64
            + step as i64 * if delta > 0. { 1 } else { -1 })
        .clamp(5.min(maximum), maximum) as u32;
        self.record.zoom_factor = factor;
        self.dirty = true;
        let scale = factor as f32 / 100. / self.content_scale;
        window.resize(size(
            px((width as f32 * scale).round().max(1.)),
            px((height as f32 * scale).round().max(1.)),
        ));
        self.record_position(window, cx);
        cx.notify();
    }
    pub fn handle_event(
        &mut self,
        event: &RuntimeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ocr_event(event, window, cx);
        match event {
            RuntimeEvent::Pin(PinEvent::Created { id, result })
                if self.pending_create == Some(*id) =>
            {
                self.pending_create = None;
                match result {
                    Ok(pin) => {
                        self.id = Some(pin.id);
                        window.set_window_title(&pin_title(&self.settings, self.id));
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
            ) if self.pending_finish.as_ref().map(|pending| pending.id) == Some(*id) => {
                let pending = self.pending_finish.take().unwrap();
                match result {
                    Ok(()) => {
                        if let Some(directory) = pending.remember_directory
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
            } => {
                self.settings = config.clone();
                window.set_window_title(&pin_title(&self.settings, self.id));
            }
            _ => return,
        }
        self.resume_export(window, cx);
        cx.notify();
    }
}
fn pin_title(config: &Config, id: Option<u32>) -> String {
    let label = if rotor_common::i18n::language_for_config(config) == "zh-CN" {
        "贴图"
    } else {
        "Pinned image"
    };
    match id {
        Some(id) => format!("Rotor · {label} {id}"),
        None => format!("Rotor · {label}"),
    }
}

fn shortcut_matches(event: &Keystroke, configured: Option<&String>) -> bool {
    let Some(configured) = configured else {
        return false;
    };
    use std::str::FromStr;
    let expected = global_hotkey::hotkey::HotKey::from_str(configured).ok();
    let actual = crate::shortcut::recorded_key(event, true)
        .ok()
        .flatten()
        .and_then(|value| global_hotkey::hotkey::HotKey::from_str(&value).ok());
    expected.is_some() && expected == actual
}

impl Render for PinView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_canvas(window, cx);
        let toolbar_active = window.is_window_active();
        // A single 27 x 25 button, 4 px panel padding and 16 px viewport margin.
        // Omit both panels immediately when even the overflow button cannot fit.
        let toolbar_fits =
            window.viewport_size().width >= px(51.) && window.viewport_size().height >= px(49.);
        let editing = self.canvas.editing();
        let annotation_width = px(192.).min((window.viewport_size().width - px(16.)).max(px(0.)));
        div()
            .id("pin")
            .track_focus(&self.focus)
            .size_full()
            .overflow_hidden()
            .child(self.canvas_element(window, cx))
            .when(self.ocr.active, |root| {
                root.child(self.ocr_layer(window, cx))
            })
            .when(toolbar_fits, |root| {
                root.child(
                    toolbar::Slide::new(self.pin_toolbar(window, cx)).with_spring(
                        "pin-toolbar-slide",
                        SpringAnimation::new(SpringConfig::new(625., 50., 1.))
                            .to(if toolbar_active && !editing { 1.0 } else { 0.0 })
                            .from(0.0),
                        |mut toolbar, progress| {
                            toolbar.progress = progress.clamp(0., 1.);
                            toolbar
                        },
                    ),
                )
            })
            .when(toolbar_fits, |root| {
                root.child(
                    toolbar::Slide::new(
                        toolbar::panel("pin-annotation-toolbar", annotation_width, window)
                            .child(self.canvas_tools(cx)),
                    )
                    .with_spring(
                        "pin-annotation-toolbar-slide",
                        SpringAnimation::new(SpringConfig::new(625., 50., 1.))
                            .to(if toolbar_active && editing { 1.0 } else { 0.0 })
                            .from(0.0),
                        |mut toolbar, progress| {
                            toolbar.progress = progress.clamp(0., 1.);
                            toolbar
                        },
                    ),
                )
            })
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                if this.canvas.editor.is_none() && !this.ocr.active {
                    this.zoom(event.delta.pixel_delta(px(16.)).y.as_f32(), window, cx);
                }
                cx.stop_propagation();
            }))
            .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !this.ocr_keys(event, window, cx) {
                    this.canvas_keys(event, window, cx);
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if this.canvas.editor.is_some() || this.ocr.active {
                    return;
                }
                if shortcut_matches(&event.keystroke, this.settings.get("shortcut_pinwin_save")) {
                    this.save(window, cx);
                } else if shortcut_matches(
                    &event.keystroke,
                    this.settings.get("shortcut_pinwin_copy"),
                ) {
                    this.export(PinExportTarget::Clipboard, window, cx);
                } else if shortcut_matches(
                    &event.keystroke,
                    this.settings.get("shortcut_pinwin_close"),
                ) {
                    this.close(window, cx);
                } else if shortcut_matches(
                    &event.keystroke,
                    this.settings.get("shortcut_pinwin_hide"),
                ) {
                    this.minimize(window, cx);
                } else {
                    return;
                }
                cx.stop_propagation();
            }))
            .capture_any_mouse_down(cx.listener(|this, event: &MouseDownEvent, window, cx| {
                if event.button == MouseButton::Right {
                    this.show_pin_menu(menu::CONTEXT_COMMANDS, event.position, window, cx);
                    cx.stop_propagation();
                }
            }))
            .on_action(cx.listener(|this, _: &menu::Annotate, window, cx| {
                this.run_command(menu::Command::Annotate, window, cx)
            }))
            .on_action(cx.listener(|this, _: &menu::Ocr, window, cx| {
                this.run_command(menu::Command::Ocr, window, cx)
            }))
            .on_action(cx.listener(|this, _: &menu::Minimize, window, cx| {
                this.run_command(menu::Command::Minimize, window, cx)
            }))
            .on_action(cx.listener(|this, _: &menu::Save, window, cx| {
                this.run_command(menu::Command::Save, window, cx)
            }))
            .on_action(cx.listener(|this, _: &menu::Close, window, cx| {
                this.run_command(menu::Command::Close, window, cx)
            }))
            .on_action(cx.listener(|this, _: &menu::Copy, window, cx| {
                this.run_command(menu::Command::Copy, window, cx)
            }))
    }
}

#[cfg(test)]
mod shortcut_tests {
    use super::shortcut_matches;
    use gpui_kit::Keystroke;
    #[test]
    fn canonical_recorded_key_names_work_for_local_shortcuts() {
        assert!(shortcut_matches(
            &Keystroke::parse("s").unwrap(),
            Some(&"KeyS".into())
        ));
        assert!(shortcut_matches(
            &Keystroke::parse("ctrl-s").unwrap(),
            Some(&"Ctrl+KeyS".into())
        ));
        assert!(!shortcut_matches(
            &Keystroke::parse("s").unwrap(),
            Some(&"Ctrl+KeyS".into())
        ));
    }
}
