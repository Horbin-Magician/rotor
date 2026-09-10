use super::*;
use crate::capture::PreparedFrame;
use gpui_kit::component::input::{Textarea, TextareaState};
use rotor_canvas::{
    Annotation, Color, Document, ImagePoint, ImageRect, ImageSize, StrokeStyle, ViewTransform,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Tool {
    Move,
    Pen,
    Rectangle,
    Arrow,
    Text,
}
#[derive(Clone, Copy, PartialEq, Eq)]
struct FrameKey {
    revision: u64,
    crop: ImageRect,
    output: ImageSize,
}
#[derive(Clone, Copy)]
enum Rollback {
    Undo,
    Redo,
}

struct AcceptedRender {
    retired: Option<Arc<RenderImage>>,
}

pub(super) struct CanvasState {
    document: Document,
    tool: Tool,
    draft: Option<Annotation>,
    preview: Option<Annotation>,
    pub(super) editor: Option<Entity<TextareaState>>,
    editor_origin: ImagePoint,
    frame: Option<PreparedFrame>,
    frame_key: Option<FrameKey>,
    requested: Option<FrameKey>,
    pub(super) rendering: bool,
    pub(super) error: Option<String>,
    epoch: u64,
    task: Option<Task<()>>,
    rollback: Option<(u64, Rollback)>,
    text_pending: bool,
}
impl CanvasState {
    pub(super) fn content_revision(&self) -> u64 {
        self.document.revision()
    }
    pub(super) fn frame_revision(&self) -> u64 {
        self.epoch
    }
    pub(super) fn new(image: &PreparedImage, record: &ShotterConfig) -> Self {
        let (x, y, width, height) =
            rotor_runtime::pin_source_crop(record, image.image.width(), image.image.height())
                .expect("validated pin source");
        Self {
            document: Document::new(
                ImageSize {
                    width: image.image.width(),
                    height: image.image.height(),
                },
                ImageRect {
                    x,
                    y,
                    width,
                    height,
                },
            )
            .expect("validated crop"),
            tool: Tool::Move,
            draft: None,
            preview: None,
            editor: None,
            editor_origin: ImagePoint { x: 0., y: 0. },
            frame: None,
            frame_key: None,
            requested: None,
            rendering: false,
            error: None,
            epoch: 0,
            task: None,
            rollback: None,
            text_pending: false,
        }
    }
    pub(super) fn frame(&self) -> Option<&PreparedFrame> {
        self.frame.as_ref()
    }
    pub(super) fn export_scene(&self) -> rotor_canvas::Scene {
        self.document.scene().clone()
    }
    pub(super) fn can_request_export(&self) -> bool {
        self.draft.is_none() && self.editor.is_none()
    }
    pub(super) fn ready(&self) -> bool {
        !self.rendering
            && self.draft.is_none()
            && self.editor.is_none()
            && self.frame_key.is_some()
            && self.frame_key == self.requested
    }
    pub(super) fn editing(&self) -> bool {
        self.tool != Tool::Move || self.editor.is_some() || self.draft.is_some()
    }
}
impl CanvasState {
    fn accept_render(
        &mut self,
        epoch: u64,
        key: FrameKey,
        prepared: Result<PreparedFrame, String>,
    ) -> Option<AcceptedRender> {
        let prepared = prepared.and_then(|frame| {
            if frame.dimensions == (key.output.width, key.output.height) {
                Ok(frame)
            } else {
                Err("Rendered frame dimensions differ from its viewport".into())
            }
        });
        if self.epoch != epoch {
            return None;
        }
        self.rendering = false;
        let mut retired = None;
        match prepared {
            Ok(frame) => {
                retired = self.frame.replace(frame).map(|old| old.render);
                self.frame_key = Some(key);
                self.preview = None;
                self.rollback = None;
                if self.editor.is_none() || self.text_pending {
                    self.error = None;
                }
                if self.text_pending {
                    self.editor = None;
                    self.text_pending = false;
                }
            }
            Err(error) => {
                self.error = Some(error);
                self.text_pending = false;
                if let Some((revision, rollback)) = self.rollback.take()
                    && revision == self.document.revision()
                {
                    match rollback {
                        Rollback::Undo => {
                            self.document.undo();
                        }
                        Rollback::Redo => {
                            self.document.redo();
                        }
                    }
                    self.preview = None;
                    self.requested = None;
                    if let Some(mut old) = self.frame_key
                        && old.crop == key.crop
                        && old.output == key.output
                    {
                        old.revision = self.document.revision();
                        self.frame_key = Some(old);
                        self.requested = Some(old);
                    }
                }
            }
        }
        Some(AcceptedRender { retired })
    }
}
impl PinView {
    pub(super) fn cancel_pointer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_move(window, cx);
        if self.crop_drag.is_some() {
            self.finish_crop(window, cx);
        }
        if self.canvas.draft.take().is_some() {
            cx.notify();
        }
        window.release_pointer();
        self.release_native_pointer(window);
    }
    pub(super) fn commit_canvas_crop(
        &mut self,
        crop: ImageRect,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.canvas.document.set_crop(crop) {
            Ok(_) => {
                self.canvas.rollback = None;
                self.ensure_canvas_after_bounds(window, cx);
            }
            Err(error) => self.canvas.error = Some(error),
        }
    }
    fn transform(&self, window: &Window) -> ViewTransform {
        let (x, y, width, height) = self.crop();
        ViewTransform {
            crop: ImageRect {
                x,
                y,
                width,
                height,
            },
            width: window.viewport_size().width.as_f32() as f64,
            height: window.viewport_size().height.as_f32() as f64,
        }
    }
    pub(super) fn ensure_canvas(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // The retained frame is positioned and clipped by canvas_element.
        // Resampling/uploading a new image here would compete with every drag frame.
        if self.crop_drag.is_some() {
            return;
        }
        let transform = self.transform(window);
        let size = window.viewport_size();
        let output = ImageSize {
            width: (size.width.as_f32() * window.scale_factor())
                .round()
                .max(1.) as u32,
            height: (size.height.as_f32() * window.scale_factor())
                .round()
                .max(1.) as u32,
        };
        let key = FrameKey {
            revision: self.canvas.document.revision(),
            crop: transform.crop,
            output,
        };
        if self
            .ocr
            .signature
            .is_some_and(|signature| signature != (key.revision, key.crop))
        {
            self.clear_ocr(window);
        }
        if self.canvas.requested == Some(key) {
            return;
        }
        let debounce = self
            .canvas
            .requested
            .is_some_and(|previous| previous.revision == key.revision);
        self.canvas.epoch = self.canvas.epoch.wrapping_add(1);
        let epoch = self.canvas.epoch;
        self.canvas.requested = Some(key);
        self.canvas.rendering = true;
        let mut scene = self.canvas.document.scene().clone();
        scene.crop = key.crop;
        let source = self.image.image.clone();
        let services = self.services.clone();
        self.canvas.task = Some(cx.spawn_in(window, async move |view, cx| {
            if debounce {
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
            }
            let result = services.render_canvas(source, scene, output).await;
            let prepared = match result {
                Ok(image) => {
                    cx.background_executor()
                        .spawn(async move { PreparedFrame::new(image) })
                        .await
                }
                Err(error) => Err(error),
            };
            let _ = cx.update(|window, cx| {
                view.update(cx, |this, cx| {
                    if let Some(accepted) = this.canvas.accept_render(epoch, key, prepared) {
                        if let Some(retired) = accepted.retired {
                            retire_canvas_frame(retired, window, cx);
                        }
                        if this.canvas.error.is_some() && this.queued_export.take().is_some() {
                            this.message = this.canvas.error.clone().unwrap();
                        }
                        this.resume_export(window, cx);
                        cx.notify();
                    }
                })
            });
        }));
    }
    pub(super) fn ensure_canvas_after_bounds(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let view = cx.weak_entity();
        window.defer(cx, move |window, cx| {
            let _ = view.update(cx, |this, cx| this.ensure_canvas(window, cx));
        });
    }
    pub(super) fn set_tool(&mut self, tool: Tool, window: &mut Window, cx: &mut Context<Self>) {
        self.canvas.tool = tool;
        self.canvas.draft = None;
        self.canvas.editor = None;
        self.canvas.text_pending = false;
        self.focus.focus(window, cx);
        window.release_pointer();
        self.release_native_pointer(window);
        cx.notify();
    }
    pub(super) fn cancel_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.canvas.editing() {
            return false;
        }
        self.set_tool(Tool::Move, window, cx);
        true
    }
    fn add_annotation(
        &mut self,
        annotation: Annotation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.canvas.document.add(annotation.clone()) {
            Ok(()) => {
                self.canvas.preview = Some(annotation);
                self.canvas.error = None;
                self.canvas.rollback = Some((self.canvas.document.revision(), Rollback::Undo));
                self.ensure_canvas(window, cx);
            }
            Err(error) => {
                self.canvas.error = Some(error);
                self.canvas.text_pending = false;
            }
        }
        cx.notify();
    }
    fn undo_canvas(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy()
            || self.canvas.rendering
            || self.canvas.editor.is_some()
            || self.canvas.draft.is_some()
            || self.crop_drag.is_some()
        {
            return;
        }
        let before_crop = self.canvas.document.scene().crop;
        let changed = self.canvas.document.undo();
        if changed {
            let next_crop = self.canvas.document.scene().crop;
            if next_crop != before_crop
                && let Err(error) = self.apply_crop(next_crop, window, cx)
            {
                self.canvas.document.redo();
                self.message = error;
                cx.notify();
                return;
            }
            self.canvas.error = None;
            self.canvas.preview = None;
            self.canvas.rollback = (next_crop == before_crop)
                .then_some((self.canvas.document.revision(), Rollback::Redo));
            self.ensure_canvas(window, cx);
            cx.notify();
        }
    }
    fn begin_mark(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.busy() || self.canvas.editor.is_some() {
            return false;
        }
        if self.canvas.tool == Tool::Move {
            if self.crop_edges(point, window).any() {
                return !self.canvas.rendering && self.begin_crop(point, window, cx);
            }
            return self.begin_move(point, window, cx);
        }
        if self.canvas.rendering {
            return false;
        }
        let transform = self.transform(window);
        let Some(origin) = transform.to_image(ImagePoint {
            x: point.x.as_f32() as f64,
            y: point.y.as_f32() as f64,
        }) else {
            return false;
        };
        let width = 3. * transform.crop.width as f64 / transform.width;
        let style = StrokeStyle {
            color: Color::RED,
            width,
        };
        self.canvas.error = None;
        if self.canvas.tool == Tool::Text {
            let input = cx.new(|cx| TextareaState::new(window, cx));
            input.update(cx, |input, cx| input.focus(window, cx));
            self.canvas.editor = Some(input);
            self.canvas.text_pending = false;
            self.canvas.editor_origin = origin;
            cx.notify();
            return false;
        }
        self.canvas.draft = Some(match self.canvas.tool {
            Tool::Pen => Annotation::Pen {
                points: vec![origin],
                style,
            },
            Tool::Rectangle => Annotation::Rectangle {
                start: origin,
                end: origin,
                style,
            },
            Tool::Arrow => Annotation::Arrow {
                start: origin,
                end: origin,
                style,
            },
            _ => return false,
        });
        if !self.capture_native_pointer(window) {
            self.canvas.draft = None;
            cx.notify();
            return false;
        }
        cx.notify();
        true
    }
    fn move_mark(&mut self, point: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if self.move_drag.is_some() {
            self.move_pin(point, window, cx);
            return;
        }
        if self.crop_drag.is_some() {
            self.move_crop(point, window, cx);
            return;
        }
        if self.canvas.tool == Tool::Move {
            let edges = self.crop_edges(point, window);
            if edges != self.crop_hover {
                self.crop_hover = edges;
                cx.notify();
            }
            return;
        }
        let transform = self.transform(window);
        let Some(mut point) = transform.to_image(ImagePoint {
            x: point.x.as_f32() as f64,
            y: point.y.as_f32() as f64,
        }) else {
            return;
        };
        point.x = point.x.clamp(
            transform.crop.x as f64,
            (transform.crop.x + transform.crop.width) as f64,
        );
        point.y = point.y.clamp(
            transform.crop.y as f64,
            (transform.crop.y + transform.crop.height) as f64,
        );
        match self.canvas.draft.as_mut() {
            Some(Annotation::Pen { points, .. }) => {
                if points.len() < 65536 && points.last() != Some(&point) {
                    points.push(point);
                }
            }
            Some(Annotation::Rectangle { end, .. } | Annotation::Arrow { end, .. }) => *end = point,
            _ => return,
        }
        cx.notify();
    }
    fn end_mark(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.move_drag.is_some() {
            self.finish_move(window, cx);
            return;
        }
        if self.crop_drag.is_some() {
            self.finish_crop(window, cx);
            return;
        }
        let Some(annotation) = self.canvas.draft.take() else {
            return;
        };
        window.release_pointer();
        self.release_native_pointer(window);
        if matches!(&annotation, Annotation::Rectangle { start, end, .. } if start.x == end.x || start.y == end.y)
            || matches!(&annotation, Annotation::Arrow { start, end, .. } if start == end)
        {
            cx.notify();
            return;
        }
        self.add_annotation(annotation, window, cx);
    }
    fn finish_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.canvas.rendering {
            return;
        }
        let Some(input) = self.canvas.editor.clone() else {
            return;
        };
        if input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return;
        }
        let text = input.read(cx).value().to_string();
        if text.trim().is_empty() {
            self.canvas.editor = None;
            cx.notify();
            return;
        }
        let transform = self.transform(window);
        self.canvas.text_pending = true;
        self.add_annotation(
            Annotation::Text {
                origin: self.canvas.editor_origin,
                text,
                font_size: 16. * transform.crop.height as f64 / transform.height,
                color: Color::RED,
            },
            window,
            cx,
        );
    }
    pub(super) fn canvas_keys(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.crop_drag.is_some() && event.keystroke.key == "escape" {
            self.cancel_crop(window, cx);
            cx.stop_propagation();
            return true;
        }
        if let Some(input) = self.canvas.editor.clone() {
            let composing = input.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            });
            if event.keystroke.key == "escape" && !composing {
                self.set_tool(Tool::Move, window, cx);
                cx.stop_propagation();
            } else if event.keystroke.key == "enter"
                && (event.keystroke.modifiers.control || event.keystroke.modifiers.platform)
                && !composing
            {
                self.finish_text(window, cx);
                cx.stop_propagation();
            }
            return true;
        }
        if event.keystroke.key == "escape" && self.canvas.editing() {
            self.cancel_editing(window, cx);
            cx.stop_propagation();
            return true;
        }
        if is_canvas_undo(&event.keystroke, self.canvas.editing()) {
            self.undo_canvas(window, cx);
            cx.stop_propagation();
            return true;
        }
        false
    }
    pub(super) fn canvas_tools(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = self.busy() || self.canvas.rendering || self.crop_drag.is_some();
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .children(
                [
                    (Tool::Move, "移动", "Move"),
                    (Tool::Pen, "画笔", "Pen"),
                    (Tool::Rectangle, "矩形", "Rectangle"),
                    (Tool::Arrow, "箭头", "Arrow"),
                    (Tool::Text, "文字", "Text"),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, (tool, zh, en))| {
                    Button::new(("canvas-tool", index))
                        .label(self.t(zh, en))
                        .tooltip(self.t(zh, en))
                        .compact()
                        .selected(self.canvas.tool == tool)
                        .toggled(self.canvas.tool == tool)
                        .disabled(disabled)
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.set_tool(tool, window, cx)),
                        )
                }),
            )
            .child(
                Button::new("canvas-undo")
                    .icon(IconName::Undo2)
                    .accessibility_label(self.t("撤销", "Undo"))
                    .tooltip(self.t("撤销", "Undo"))
                    .compact()
                    .disabled(
                        disabled
                            || !self.canvas.document.can_undo()
                            || self.canvas.editor.is_some(),
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.undo_canvas(window, cx))),
            )
            .when(self.canvas.editor.is_some(), |row| {
                row.child(
                    Button::new("canvas-text-done")
                        .icon(IconName::Check)
                        .accessibility_label(self.t("完成文字标注", "Finish text annotation"))
                        .tooltip(self.t("完成文字标注", "Finish text annotation"))
                        .compact()
                        .disabled(disabled)
                        .on_click(cx.listener(|this, _, window, cx| this.finish_text(window, cx))),
                )
            })
    }
    pub(super) fn canvas_element(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let transform = self.transform(window);
        let scale_x = transform.width / transform.crop.width as f64;
        let scale_y = transform.height / transform.crop.height as f64;
        let mut layer = div().size_full().overflow_hidden();
        if let Some((frame, key)) = self.canvas.frame.as_ref().zip(self.canvas.frame_key) {
            let left = ((key.crop.x as f64 - transform.crop.x as f64) * scale_x)
                .clamp(0., transform.width);
            let top = ((key.crop.y as f64 - transform.crop.y as f64) * scale_y)
                .clamp(0., transform.height);
            let right = (((key.crop.x + key.crop.width) as f64 - transform.crop.x as f64)
                * scale_x)
                .clamp(0., transform.width);
            let bottom = (((key.crop.y + key.crop.height) as f64 - transform.crop.y as f64)
                * scale_y)
                .clamp(0., transform.height);
            let regions = if right <= left || bottom <= top {
                vec![(0., 0., transform.width, transform.height)]
            } else {
                vec![
                    (0., 0., transform.width, top),
                    (0., bottom, transform.width, transform.height - bottom),
                    (0., top, left, bottom - top),
                    (right, top, transform.width - right, bottom - top),
                ]
            };
            for (x, y, width, height) in regions {
                if width <= 0. || height <= 0. {
                    continue;
                }
                layer = layer.child(
                    div()
                        .absolute()
                        .left(px(x as f32))
                        .top(px(y as f32))
                        .w(px(width as f32))
                        .h(px(height as f32))
                        .overflow_hidden()
                        .child(
                            img(self.image.render.clone())
                                .absolute()
                                .left(px((-(transform.crop.x as f64) * scale_x - x) as f32))
                                .top(px((-(transform.crop.y as f64) * scale_y - y) as f32))
                                .w(px((self.image.image.width() as f64 * scale_x) as f32))
                                .h(px((self.image.image.height() as f64 * scale_y) as f32)),
                        ),
                );
            }
            layer = layer.child(
                img(frame.render.clone())
                    .absolute()
                    .left(px(
                        (key.crop.x as f64 - transform.crop.x as f64) as f32 * scale_x as f32
                    ))
                    .top(px(
                        (key.crop.y as f64 - transform.crop.y as f64) as f32 * scale_y as f32
                    ))
                    .w(px((key.crop.width as f64 * scale_x) as f32))
                    .h(px((key.crop.height as f64 * scale_y) as f32)),
            );
        } else {
            layer = layer.child(
                img(self.image.render.clone())
                    .absolute()
                    .left(px(-(transform.crop.x as f64 * scale_x) as f32))
                    .top(px(-(transform.crop.y as f64 * scale_y) as f32))
                    .w(px((self.image.image.width() as f64 * scale_x) as f32))
                    .h(px((self.image.image.height() as f64 * scale_y) as f32)),
            );
        }
        let weak = cx.weak_entity();
        let preview = self
            .canvas
            .draft
            .as_ref()
            .or(self.canvas.preview.as_ref())
            .cloned();
        let dragging =
            self.canvas.draft.is_some() || self.crop_drag.is_some() || self.move_drag.is_some();
        let cursor = match self.canvas.tool {
            Tool::Move => self.crop_cursor(),
            Tool::Text => CursorStyle::IBeam,
            _ => CursorStyle::Crosshair,
        };
        layer = layer.child(
            canvas(
                |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                move |bounds, hitbox, window, _| {
                    window.set_cursor_style(cursor, &hitbox);
                    if dragging {
                        window.capture_pointer(hitbox.id);
                    }
                    if let Some(annotation) = &preview {
                        paint_preview(annotation, transform, bounds.origin, window);
                    }
                    let down_view = weak.clone();
                    let down_hitbox = hitbox.clone();
                    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble
                            && event.button == MouseButton::Left
                            && down_hitbox.is_hovered(window)
                        {
                            let capture = down_view
                                .update(cx, |this, cx| {
                                    this.begin_mark(event.position - bounds.origin, window, cx)
                                })
                                .unwrap_or(false);
                            if capture {
                                window.capture_pointer(down_hitbox.id);
                            }
                            cx.stop_propagation();
                        }
                    });
                    let move_view = weak.clone();
                    let move_hitbox = hitbox.clone();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble
                            && (dragging || move_hitbox.is_hovered(window))
                        {
                            let _ = move_view.update(cx, |this, cx| {
                                if event.pressed_button != Some(MouseButton::Left)
                                    && (this.move_drag.is_some()
                                        || this.crop_drag.is_some()
                                        || this.canvas.draft.is_some())
                                {
                                    this.end_mark(window, cx);
                                    return;
                                }
                                this.move_mark(event.position - bounds.origin, window, cx)
                            });
                        }
                    });
                    window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                        if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                            let _ = weak.update(cx, |this, cx| {
                                if this.crop_drag.is_some() {
                                    this.move_crop(event.position - bounds.origin, window, cx);
                                }
                                this.end_mark(window, cx);
                            });
                        }
                    });
                },
            )
            .absolute()
            .size_full(),
        );
        if let Some(input) = &self.canvas.editor {
            let origin = transform
                .to_view(self.canvas.editor_origin)
                .unwrap_or(ImagePoint { x: 0., y: 0. });
            layer = layer.child(
                div()
                    .absolute()
                    .left(px(origin.x as f32))
                    .top(px(origin.y as f32))
                    .w(px((transform.width - origin.x).max(40.) as f32))
                    .font_family(rotor_canvas::FONT_FAMILY)
                    .text_size(px(16.))
                    .occlude()
                    .child(
                        Textarea::new(input)
                            .h(px(80.))
                            .disabled(self.canvas.rendering),
                    ),
            );
        }
        layer.into_any_element()
    }
}

fn retire_canvas_frame(retired: Arc<RenderImage>, window: &Window, cx: &mut App) {
    // The old scene still refers to this atlas entry. Defer until the view's
    // mutable borrow ends, then replace the scene before freeing its image.
    // Explicit drawing also handles hidden pins without waiting for a frame.
    window.defer(cx, move |window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
        if let Err(error) = window.drop_image(retired) {
            log::warn!("Pin image release: {error}");
        }
    });
}

fn paint_preview(
    annotation: &Annotation,
    transform: ViewTransform,
    origin: Point<Pixels>,
    window: &mut Window,
) {
    let point = |point| {
        transform
            .to_view(point)
            .map(|point| origin + gpui_kit::point(px(point.x as f32), px(point.y as f32)))
    };
    let (points, style, closed) = match annotation {
        Annotation::Pen { points, style } => (points.clone(), *style, false),
        Annotation::Rectangle { start, end, style } => (
            vec![
                *start,
                ImagePoint {
                    x: end.x,
                    y: start.y,
                },
                *end,
                ImagePoint {
                    x: start.x,
                    y: end.y,
                },
                *start,
            ],
            *style,
            true,
        ),
        Annotation::Arrow { start, end, style } => {
            let head = rotor_canvas::arrow_head(*start, *end, style.width);
            let mut fill = PathBuilder::fill();
            if let Some(tip) = point(head[0]) {
                fill.move_to(tip);
            }
            if let Some(left) = point(head[1]) {
                fill.line_to(left);
            }
            if let Some(right) = point(head[2]) {
                fill.line_to(right);
            }
            fill.close();
            if let Ok(path) = fill.build() {
                window.paint_path(path, rgba(u32::from_be_bytes(style.color.0)));
            }
            (vec![*start, *end], *style, false)
        }
        Annotation::Text { .. } => return,
    };
    let width = style.width * transform.width / transform.crop.width as f64;
    let mut path = PathBuilder::stroke(px(width as f32));
    for (index, position) in points.into_iter().filter_map(point).enumerate() {
        if index == 0 {
            path.move_to(position);
        } else {
            path.line_to(position);
        }
    }
    if closed {
        path.close();
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, rgba(u32::from_be_bytes(style.color.0)));
    }
}

fn is_canvas_undo(key: &Keystroke, editing: bool) -> bool {
    editing
        && key.key.eq_ignore_ascii_case("z")
        && (key.modifiers.control || key.modifiers.platform)
        && !key.modifiers.shift
        && !key.modifiers.alt
}

#[cfg(test)]
mod tests {
    use super::{CanvasState, FrameKey, PreparedFrame, Rollback};
    use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window};

    #[test]
    fn undo_only_claims_the_plain_editing_chord() {
        let undo = gpui_kit::Keystroke::parse("ctrl-z").unwrap();
        assert!(super::is_canvas_undo(&undo, true));
        assert!(!super::is_canvas_undo(&undo, false));
        assert!(super::super::shortcut_matches(
            &undo,
            Some(&"Ctrl+KeyZ".into())
        ));
        for chord in ["ctrl-shift-z", "ctrl-alt-z", "z"] {
            assert!(!super::is_canvas_undo(
                &gpui_kit::Keystroke::parse(chord).unwrap(),
                true
            ));
        }
    }
    use rotor_canvas::{Annotation, Color, ImagePoint, StrokeStyle};
    use rotor_runtime::ShotterConfig;
    use std::sync::Arc;

    fn state() -> (CanvasState, FrameKey) {
        let image = crate::prepare_image(Arc::new(image::RgbaImage::from_pixel(
            4,
            4,
            image::Rgba([10, 20, 30, 128]),
        )))
        .unwrap();
        let record = ShotterConfig {
            monitor_pos: (0, 0),
            monitor_size: (4, 4),
            rect: (0, 0, 4, 4),
            image_rect: None,
            offset: (0, 0),
            zoom_factor: 100,
            mask_label: "ssmask-1".into(),
            minimized: false,
        };
        let mut state = CanvasState::new(&image, &record);
        let key = FrameKey {
            revision: 0,
            crop: state.document.scene().crop,
            output: state.document.scene().size,
        };
        state.frame = Some(PreparedFrame::new(image.image).unwrap());
        state.frame_key = Some(key);
        state.requested = Some(key);
        (state, key)
    }
    #[test]
    fn stale_render_cannot_replace_the_current_frame_or_error() {
        let (mut state, key) = state();
        state.epoch = 2;
        state.rendering = true;
        assert!(
            state
                .accept_render(1, key, Err("late failure".into()))
                .is_none()
        );
        assert!(state.error.is_none());
        assert!(state.rendering);
        assert_eq!(
            state.frame.unwrap().rgba().get_pixel(0, 0).0,
            [10, 20, 30, 128]
        );
    }

    #[test]
    fn export_uses_document_pixels_after_display_frame_is_released() {
        let (mut state, _) = state();
        state
            .document
            .set_crop(rotor_canvas::ImageRect {
                x: 1,
                y: 1,
                width: 2,
                height: 3,
            })
            .unwrap();
        state.frame = None;
        let scene = state.export_scene();
        let source =
            image::RgbaImage::from_fn(4, 4, |x, y| image::Rgba([x as u8, y as u8, 80, 128]));
        let rendered = rotor_canvas::Renderer::without_fonts()
            .render(
                &source,
                &scene,
                rotor_canvas::ImageSize {
                    width: scene.crop.width,
                    height: scene.crop.height,
                },
            )
            .unwrap();
        assert_eq!(
            rendered,
            image::imageops::crop_imm(&source, 1, 1, 2, 3).to_image()
        );
    }
    #[test]
    fn failed_edit_restores_exportable_pixels_and_keeps_redo() {
        let (mut state, mut key) = state();
        let original = state.frame.as_ref().unwrap().render.clone();
        state
            .document
            .add(Annotation::Pen {
                points: vec![ImagePoint { x: 1., y: 1. }],
                style: StrokeStyle {
                    color: Color::RED,
                    width: 1.,
                },
            })
            .unwrap();
        key.revision = state.document.revision();
        state.requested = Some(key);
        state.epoch = 1;
        state.rendering = true;
        state.rollback = Some((key.revision, Rollback::Undo));
        let accepted = state
            .accept_render(1, key, Err("renderer unavailable".into()))
            .unwrap();
        assert!(accepted.retired.is_none());
        assert!(Arc::ptr_eq(
            &state.frame.as_ref().unwrap().render,
            &original
        ));
        assert!(state.document.scene().annotations.is_empty());
        assert!(state.document.can_redo());
        assert!(state.ready());
        assert!(state.error.is_some());
    }
    #[test]
    fn failed_resize_does_not_undo_committed_annotations_or_export_stale_size() {
        let (mut state, mut key) = state();
        state
            .document
            .add(Annotation::Pen {
                points: vec![ImagePoint { x: 1., y: 1. }],
                style: StrokeStyle {
                    color: Color::RED,
                    width: 1.,
                },
            })
            .unwrap();
        key.revision = state.document.revision();
        key.output.width = 8;
        state.requested = Some(key);
        state.epoch = 3;
        state.accept_render(3, key, Err("resize failed".into()));
        assert_eq!(state.document.scene().annotations.len(), 1);
        assert!(!state.ready());
    }

    fn prepared(width: u32, height: u32) -> PreparedFrame {
        PreparedFrame::new(Arc::new(image::RgbaImage::from_pixel(
            width,
            height,
            image::Rgba([80, 90, 100, 255]),
        )))
        .unwrap()
    }

    #[test]
    fn only_successful_current_render_retires_the_previous_frame() {
        let (mut state, key) = state();
        let original = state.frame.as_ref().unwrap().render.clone();
        state.epoch = 2;
        let stale = prepared(4, 4);
        let stale_weak = Arc::downgrade(&stale.render);
        assert!(state.accept_render(1, key, Ok(stale)).is_none());
        assert!(stale_weak.upgrade().is_none());
        assert!(Arc::ptr_eq(
            &state.frame.as_ref().unwrap().render,
            &original
        ));

        let invalid = state.accept_render(2, key, Ok(prepared(8, 8))).unwrap();
        assert!(invalid.retired.is_none());
        assert!(state.error.is_some());
        assert!(Arc::ptr_eq(
            &state.frame.as_ref().unwrap().render,
            &original
        ));

        let next = prepared(4, 4);
        let next_render = next.render.clone();
        let accepted = state.accept_render(2, key, Ok(next)).unwrap();
        assert!(Arc::ptr_eq(&accepted.retired.unwrap(), &original));
        assert!(Arc::ptr_eq(
            &state.frame.as_ref().unwrap().render,
            &next_render
        ));
    }

    // Mock GPUI window/atlas only: no native window or visual UI validation.
    struct FrameView {
        source: crate::PreparedImage,
        state: CanvasState,
        retiring: Option<Arc<gpui::RenderImage>>,
    }

    impl Render for FrameView {
        fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            if let Some(retiring) = self.retiring.take() {
                // The previous scene's tile must survive until rebuilding begins.
                assert!(window.has_image_atlas_entry(&retiring));
            }
            gpui::div()
                .size_full()
                .child(
                    gpui::img(self.source.render.clone())
                        .w(gpui::px(4.))
                        .h(gpui::px(4.)),
                )
                .child(
                    gpui::img(self.state.frame.as_ref().unwrap().render.clone())
                        .w(gpui::px(4.))
                        .h(gpui::px(4.)),
                )
        }
    }

    #[gpui::test]
    fn replacement_scene_releases_retired_atlas_entries(cx: &mut gpui::TestAppContext) {
        let (state, mut key) = state();
        let window = cx.add_window(|_, _| FrameView {
            source: crate::prepare_image(prepared(4, 4).rgba()).unwrap(),
            state,
            retiring: None,
        });
        gpui::AnyWindowHandle::from(window)
            .update(cx, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();

        // Revisions, crops, and output sizes all use the same retirement path.
        for revision in 1..=24 {
            key.revision = revision;
            key.crop.x = (revision % 2) as u32;
            key.crop.width = 4 - key.crop.x;
            key.output.width = if revision % 3 == 0 { 8 } else { 4 };
            let old = window
                .update(cx, |view, window, cx| {
                    let old = view.state.frame.as_ref().unwrap().render.clone();
                    assert!(window.has_image_atlas_entry(&old));
                    view.retiring = Some(old.clone());
                    view.state.epoch = revision;
                    let accepted = view
                        .state
                        .accept_render(revision, key, Ok(prepared(key.output.width, 4)))
                        .unwrap();
                    super::retire_canvas_frame(accepted.retired.unwrap(), window, cx);
                    // Scheduling retirement must not invalidate the scene still in use.
                    assert!(window.has_image_atlas_entry(&old));
                    cx.notify();
                    old
                })
                .unwrap();
            cx.run_until_parked();
            window
                .update(cx, |view, window, _| {
                    assert!(!window.has_image_atlas_entry(&old));
                    assert!(window.has_image_atlas_entry(&view.source.render));
                    assert!(
                        window.has_image_atlas_entry(&view.state.frame.as_ref().unwrap().render)
                    );
                })
                .unwrap();
        }
    }

    #[gpui::test]
    fn closing_before_retirement_releases_the_pending_image(cx: &mut gpui::TestAppContext) {
        let (state, key) = state();
        let old = Arc::downgrade(&state.frame.as_ref().unwrap().render);
        let window = cx.add_window(|_, _| FrameView {
            source: crate::prepare_image(prepared(4, 4).rgba()).unwrap(),
            state,
            retiring: None,
        });
        gpui::AnyWindowHandle::from(window)
            .update(cx, |_, window, cx| window.draw(cx).clear(cx))
            .unwrap();
        window
            .update(cx, |view, window, cx| {
                let accepted = view
                    .state
                    .accept_render(0, key, Ok(prepared(4, 4)))
                    .unwrap();
                super::retire_canvas_frame(accepted.retired.unwrap(), window, cx);
                window.remove_window();
            })
            .unwrap();
        cx.run_until_parked();
        assert!(old.upgrade().is_none());
    }
}
