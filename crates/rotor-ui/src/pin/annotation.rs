use super::*;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use rotor_canvas::{
    Annotation, Color, Document, ImagePoint, ImageRect, ImageSize, Outline, StrokeStyle,
    ViewTransform,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tool {
    Pen,
    Rectangle,
    Arrow,
    Text,
}
/// Annotation editing: the selected tool and whatever is being drafted with
/// it. This is the payload of `Mode::Annotating`; when no tool is selected the
/// pin is idle and the pointer moves or crops the window instead.
pub(super) struct Editing {
    tool: Tool,
    draft: Draft,
}
enum Draft {
    None,
    /// A pen, rectangle or arrow being dragged; the canvas holds the pointer.
    Stroke(Annotation),
    /// An open text input; enter commits it and escape discards it.
    Text(TextEditor),
}
pub(super) struct TextEditor {
    pub(super) input: Entity<InputState>,
    _events: Subscription,
    origin: ImagePoint,
    suppress_enter: bool,
}
impl Editing {
    fn new(tool: Tool) -> Self {
        Self {
            tool,
            draft: Draft::None,
        }
    }
    pub(super) fn tool(&self) -> Tool {
        self.tool
    }
    /// A stroke or text is half-drawn; exports and undo must wait.
    pub(super) fn has_draft(&self) -> bool {
        !matches!(self.draft, Draft::None)
    }
    pub(super) fn stroke(&self) -> Option<&Annotation> {
        match &self.draft {
            Draft::Stroke(annotation) => Some(annotation),
            _ => None,
        }
    }
    fn take_stroke(&mut self) -> Option<Annotation> {
        match std::mem::replace(&mut self.draft, Draft::None) {
            Draft::Stroke(annotation) => Some(annotation),
            other => {
                self.draft = other;
                None
            }
        }
    }
    pub(super) fn text_editor(&self) -> Option<&TextEditor> {
        match &self.draft {
            Draft::Text(editor) => Some(editor),
            _ => None,
        }
    }
    pub(super) fn text_editor_mut(&mut self) -> Option<&mut TextEditor> {
        match &mut self.draft {
            Draft::Text(editor) => Some(editor),
            _ => None,
        }
    }
    fn close_text(&mut self) {
        if matches!(self.draft, Draft::Text(_)) {
            self.draft = Draft::None;
        }
    }
}
// Retain shaped text across pointer moves and unrelated window refreshes.
#[derive(Clone)]
struct DisplayMark {
    annotation: Annotation,
    lines: Vec<ShapedLine>,
    paths: Vec<(gpui::Path<Pixels>, Color)>,
}

/// The annotation document and its retained display data. Editing state
/// lives in `Mode::Annotating`, so this survives every mode change.
pub(super) struct CanvasState {
    document: Document,
    pub(super) error: Option<String>,
    display_key: Option<(u64, u64)>,
    display: Arc<Vec<Arc<DisplayMark>>>,
}
impl CanvasState {
    pub(super) fn content_revision(&self) -> u64 {
        self.document.revision()
    }
    pub(super) fn new(image: &PreparedImage, record: &ShotterConfig) -> Self {
        let (x, y, width, height) =
            rotor_runtime::pin_source_crop(record, image.width(), image.height())
                .expect("validated pin source");
        let mut document = Document::new(
            ImageSize {
                width: image.width(),
                height: image.height(),
            },
            ImageRect {
                x,
                y,
                width,
                height,
            },
        )
        .expect("validated crop");
        for annotation in &record.annotations {
            document
                .add(annotation.clone())
                .expect("validated annotation");
        }
        Self {
            document,
            error: None,
            display_key: None,
            display: Arc::new(Vec::new()),
        }
    }
    pub(super) fn export_scene(&self) -> rotor_canvas::Scene {
        self.document.scene().clone()
    }
}
impl PinView {
    /// Nothing is half-drawn, so the document may be exported or recognized.
    pub(super) fn can_request_export(&self) -> bool {
        self.mode.can_export()
    }
    /// The window lost the pointer: commit drags and discard a half-drawn stroke.
    pub(super) fn cancel_pointer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_move(window, cx);
        if self.mode.is_cropping() {
            self.finish_crop(window, cx);
        }
        if self
            .mode
            .editing_mut()
            .and_then(Editing::take_stroke)
            .is_some()
        {
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
                self.ensure_canvas_after_bounds(window, cx);
            }
            Err(error) => self.canvas.error = Some(error),
        }
    }
    fn canvas_scale(&self) -> f64 {
        rotor_canvas::pin_scale(self.record.zoom_factor, self.content_scale as f64)
    }
    fn transform(&self) -> ViewTransform {
        let (x, y, width, height) = self.crop();
        // Cropping changes the clip, not the image magnification. Native bounds
        // are rounded and may lag a frame; deriving scale from them stretches
        // the entire image (and reshapes text) on every edge movement.
        let scale = self.canvas_scale();
        ViewTransform {
            crop: ImageRect {
                x,
                y,
                width,
                height,
            },
            width: width as f64 * scale,
            height: height as f64 * scale,
        }
    }
    pub(super) fn ensure_canvas(&mut self, window: &mut Window, _: &mut Context<Self>) {
        let signature = (self.canvas.document.revision(), self.transform().crop);
        if self.mode.ocr().is_some_and(|session| {
            session
                .signature
                .is_some_and(|previous| previous != signature)
        }) {
            self.clear_ocr(window);
        }
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
        self.select_tool(Some(tool), window, cx);
    }
    pub(super) fn cancel_editing(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.mode.is_annotating() {
            return false;
        }
        self.select_tool(None, window, cx);
        true
    }
    /// Enter annotation mode with `tool`, or leave it with `None`. Any drag in
    /// progress ends first; an open text input is committed unless it is
    /// still composing, in which case the current mode is kept.
    fn select_tool(&mut self, tool: Option<Tool>, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editing) = self.mode.editing()
            && let Some(editor) = editing.text_editor()
        {
            if Some(editing.tool) == tool {
                let input = editor.input.clone();
                input.update(cx, |input, cx| input.focus(window, cx));
                return;
            }
            self.finish_text(window, cx);
            if self.mode.text_editor().is_some() {
                return;
            }
        }
        self.finish_move(window, cx);
        self.cancel_crop(window, cx);
        self.crop_hover = Default::default();
        self.mode = match tool {
            Some(tool) => Mode::Annotating(Editing::new(tool)),
            None => Mode::Idle,
        };
        self.focus.focus(window, cx);
        window.release_pointer();
        self.release_native_pointer(window);
        cx.notify();
    }
    fn add_annotation(
        &mut self,
        annotation: Annotation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.canvas.document.add(annotation) {
            Ok(()) => {
                self.canvas.error = None;
                self.ensure_canvas(window, cx);
                self.persist_geometry(cx);
            }
            Err(error) => {
                self.canvas.error = Some(error);
            }
        }
        cx.notify();
    }
    fn undo_canvas(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || !self.can_request_export() || self.mode.is_cropping() {
            return;
        }
        let before_crop = self.canvas.document.scene().crop;
        let changed = self.canvas.document.undo();
        if changed {
            let next_crop = self.canvas.document.scene().crop;
            let crop_changed = next_crop != before_crop;
            if crop_changed && let Err(error) = self.apply_crop(next_crop, window, cx) {
                self.canvas.document.redo();
                self.message = error;
                cx.notify();
                return;
            }
            self.canvas.error = None;
            self.ensure_canvas(window, cx);
            // apply_crop already re-sampled the position and flushed the
            // undone record; a second flush would send a duplicate update.
            if !crop_changed {
                self.persist_geometry(cx);
            }
            cx.notify();
        }
    }
    pub(super) fn begin_mark(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.mode.is_ocr() || self.busy() {
            return false;
        }
        let Some(editing) = self.mode.editing() else {
            // Idle starts a crop or move; a drag in progress keeps its anchor.
            if self.crop_edges(point, window).any() {
                return self.begin_crop(point, window, cx);
            }
            return self.begin_move(point, window, cx);
        };
        let (tool, has_editor) = (editing.tool, editing.text_editor().is_some());
        if has_editor {
            self.finish_text(window, cx);
            return false;
        }
        let transform = self.transform();
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
        let annotation = match tool {
            Tool::Text => {
                let input = cx.new(|cx| InputState::new(window, cx));
                let events = cx.subscribe_in(&input, window, |this, input, event, window, cx| {
                    if matches!(event, InputEvent::PressEnter { .. })
                        && let Some(editor) = this.mode.text_editor()
                        && editor.input == *input
                    {
                        if !editor.suppress_enter {
                            this.finish_text(window, cx);
                        }
                        if let Some(editor) = this.mode.text_editor_mut() {
                            editor.suppress_enter = false;
                        }
                    }
                });
                input.update(cx, |input, cx| input.focus(window, cx));
                if let Some(editing) = self.mode.editing_mut() {
                    editing.draft = Draft::Text(TextEditor {
                        input,
                        _events: events,
                        origin,
                        suppress_enter: false,
                    });
                }
                cx.notify();
                return false;
            }
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
        };
        if let Some(editing) = self.mode.editing_mut() {
            editing.draft = Draft::Stroke(annotation);
        }
        if !self.capture_native_pointer(window) {
            if let Some(editing) = self.mode.editing_mut() {
                editing.draft = Draft::None;
            }
            cx.notify();
            return false;
        }
        cx.notify();
        true
    }
    fn move_mark(&mut self, point: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        match self.mode {
            Mode::MovingWindow(_) => return self.move_pin(point, window, cx),
            Mode::CroppingEdge(_) => return self.move_crop(point, window, cx),
            Mode::Idle | Mode::Ocr(_) => {
                let edges = self.crop_edges(point, window);
                if edges != self.crop_hover {
                    self.crop_hover = edges;
                    cx.notify();
                }
                return;
            }
            Mode::Annotating(_) => {}
        }
        let transform = self.transform();
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
        match self.mode.editing_mut().map(|editing| &mut editing.draft) {
            Some(Draft::Stroke(Annotation::Pen { points, .. })) => {
                if points.len() < 65536 && points.last() != Some(&point) {
                    points.push(point);
                }
            }
            Some(Draft::Stroke(
                Annotation::Rectangle { end, .. } | Annotation::Arrow { end, .. },
            )) => *end = point,
            _ => return,
        }
        cx.notify();
    }
    fn end_mark(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode.is_moving() {
            self.finish_move(window, cx);
            return;
        }
        if self.mode.is_cropping() {
            self.finish_crop(window, cx);
            return;
        }
        let Some(annotation) = self.mode.editing_mut().and_then(Editing::take_stroke) else {
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
        let Some(editor) = self.mode.text_editor() else {
            return;
        };
        let (input, origin) = (editor.input.clone(), editor.origin);
        if input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            return;
        }
        let text = input.read(cx).value().to_string();
        if text.trim().is_empty() {
            self.close_text_editor();
            self.focus.focus(window, cx);
            cx.notify();
            return;
        }
        let transform = self.transform();
        self.add_annotation(
            Annotation::Text {
                origin,
                text,
                font_size: 16. * transform.crop.height as f64 / transform.height,
                color: Color::RED,
            },
            window,
            cx,
        );
        if self.canvas.error.is_none() {
            self.close_text_editor();
            self.focus.focus(window, cx);
        }
    }
    fn close_text_editor(&mut self) {
        if let Some(editing) = self.mode.editing_mut() {
            editing.close_text();
        }
    }
    fn cancel_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.close_text_editor();
        self.canvas.error = None;
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub(super) fn canvas_keys(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.mode.is_cropping() && event.keystroke.key == "escape" {
            self.cancel_crop(window, cx);
            cx.stop_propagation();
            return true;
        }
        if let Some(editor) = self.mode.text_editor() {
            let input = editor.input.clone();
            let composing = input.update(cx, |input, cx| {
                input.marked_text_range(window, cx).is_some()
            });
            if event.keystroke.key == "enter"
                && let Some(editor) = self.mode.text_editor_mut()
            {
                editor.suppress_enter = composing;
            }
            if event.keystroke.key == "escape" && !composing {
                self.cancel_text(window, cx);
                cx.stop_propagation();
            }
            return true;
        }
        if event.keystroke.key == "escape" && self.mode.is_annotating() {
            self.cancel_editing(window, cx);
            cx.stop_propagation();
            return true;
        }
        if is_canvas_undo(&event.keystroke, self.mode.is_annotating()) {
            self.undo_canvas(window, cx);
            cx.stop_propagation();
            return true;
        }
        false
    }
    pub(super) fn canvas_tools(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let disabled = self.busy() || self.mode.is_cropping();
        let selected = self.mode.editing().map(Editing::tool);
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_center()
            .gap(px(2.))
            .child(
                toolbar::button("canvas-back", toolbar::Glyph::Back, cx)
                    .accessibility_label(self.t("返回", "Back"))
                    .tooltip(self.t("返回", "Back"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.cancel_editing(window, cx);
                    })),
            )
            .child(toolbar::separator())
            .children(
                [
                    (
                        "canvas-pen",
                        Tool::Pen,
                        toolbar::Glyph::Pen,
                        "自由绘制",
                        "Free draw",
                    ),
                    (
                        "canvas-rectangle",
                        Tool::Rectangle,
                        toolbar::Glyph::Rectangle,
                        "矩形",
                        "Rectangle",
                    ),
                    (
                        "canvas-arrow",
                        Tool::Arrow,
                        toolbar::Glyph::Arrow,
                        "箭头",
                        "Arrow",
                    ),
                    (
                        "canvas-text",
                        Tool::Text,
                        toolbar::Glyph::Text,
                        "文字",
                        "Text",
                    ),
                ]
                .into_iter()
                .map(|(id, tool, glyph, zh, en)| {
                    toolbar::button(id, glyph, cx)
                        .accessibility_label(self.t(zh, en))
                        .tooltip(self.t(zh, en))
                        .selected(selected == Some(tool))
                        .toggled(selected == Some(tool))
                        .disabled(disabled)
                        .on_click(
                            cx.listener(move |this, _, window, cx| this.set_tool(tool, window, cx)),
                        )
                }),
            )
            .child(toolbar::separator())
            .child(
                toolbar::button("canvas-undo", toolbar::Glyph::Undo, cx)
                    .accessibility_label(self.t("撤回", "Undo"))
                    .tooltip(self.t("撤回", "Undo"))
                    .disabled(
                        disabled
                            || !self.canvas.document.can_undo()
                            || self.mode.text_editor().is_some(),
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.undo_canvas(window, cx))),
            )
    }
    pub(super) fn canvas_element(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let transform = self.transform();
        let scale_x = self.canvas_scale();
        let scale_y = scale_x;
        let mut layer = div().size_full().overflow_hidden();
        layer = layer.child(
            img(self.image.render.clone())
                .absolute()
                .left(px(-(transform.crop.x as f64 * scale_x) as f32))
                .top(px(-(transform.crop.y as f64 * scale_y) as f32))
                .w(px((self.image.width() as f64 * scale_x) as f32))
                .h(px((self.image.height() as f64 * scale_y) as f32)),
        );
        let key = (self.canvas.document.revision(), scale_y.to_bits());
        if self.canvas.display_key != Some(key) {
            let reuse = self
                .canvas
                .display_key
                .is_some_and(|previous| previous.1 == key.1);
            self.canvas.display = Arc::new(
                self.canvas
                    .document
                    .scene()
                    .annotations
                    .iter()
                    .enumerate()
                    .map(|(index, annotation)| {
                        if let Some(mark) = self.canvas.display.get(index)
                            && (reuse || !matches!(annotation, Annotation::Text { .. }))
                            && mark.annotation == *annotation
                        {
                            return mark.clone();
                        }
                        Arc::new(DisplayMark::new(annotation.clone(), scale_y, window))
                    })
                    .collect(),
            );
            self.canvas.display_key = Some(key);
        }
        let display = self.canvas.display.clone();
        let weak = cx.weak_entity();
        let preview = self.mode.editing().and_then(Editing::stroke).cloned();
        let dragging = self.mode.canvas_drag();
        let cursor = match self.mode.editing().map(Editing::tool) {
            None => self.crop_cursor(),
            Some(Tool::Text) => CursorStyle::IBeam,
            Some(_) => CursorStyle::Crosshair,
        };
        layer = layer.child(
            canvas(
                |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
                move |bounds, hitbox, window, cx| {
                    window.set_cursor_style(cursor, &hitbox);
                    if dragging {
                        window.capture_pointer(hitbox.id);
                    }
                    for mark in display.iter() {
                        mark.paint(transform, bounds.origin, window, cx);
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
                                    && this.mode.canvas_drag()
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
                                if this.mode.is_cropping() {
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
        if let Some(editor) = self.mode.text_editor() {
            let origin = transform
                .to_view(editor.origin)
                .unwrap_or(ImagePoint { x: 0., y: 0. });
            let (left, top, width, height) =
                text_editor_bounds(origin, transform.width, transform.height);
            layer = layer.child(
                div()
                    .id("canvas-text-editor")
                    .absolute()
                    .left(px(left as f32))
                    .top(px(top as f32))
                    .w(px(width as f32))
                    .font_family(rotor_canvas::FONT_FAMILY)
                    .text_size(px(16.))
                    .occlude()
                    .child(
                        Input::new(&editor.input)
                            .h(px(height as f32))
                            .aria_label(self.t("标注文字", "Annotation text")),
                    ),
            );
        }
        layer.into_any_element()
    }
}

fn text_editor_bounds(origin: ImagePoint, width: f64, height: f64) -> (f64, f64, f64, f64) {
    let editor_width = width.clamp(0., 320.);
    let editor_height = height.clamp(0., 40.);
    (
        origin.x.clamp(0., (width - editor_width).max(0.)),
        // Leave room for the annotation toolbar, including its wrapped second row.
        origin.y.clamp(0., (height - editor_height - 64.).max(0.)),
        editor_width,
        editor_height,
    )
}

impl DisplayMark {
    fn new(annotation: Annotation, scale: f64, window: &Window) -> Self {
        let lines = if let Annotation::Text {
            text,
            font_size,
            color,
            ..
        } = &annotation
        {
            text.lines()
                .map(|text| {
                    let text: SharedString = text.to_owned().into();
                    window.text_system().shape_line(
                        text.clone(),
                        px((font_size * scale) as f32),
                        &[TextRun {
                            len: text.len(),
                            font: font(rotor_canvas::FONT_FAMILY),
                            color: rgba(u32::from_be_bytes(color.0)).into(),
                            ..Default::default()
                        }],
                        None,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };
        let paths = annotation_paths(&annotation);
        Self {
            annotation,
            lines,
            paths,
        }
    }

    fn paint(
        &self,
        transform: ViewTransform,
        offset: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Annotation::Text {
            origin, font_size, ..
        } = &self.annotation
        {
            let scale = transform.height / transform.crop.height as f64;
            if let Some(origin) = transform.to_view(*origin) {
                for (index, line) in self.lines.iter().enumerate() {
                    let position = offset
                        + point(
                            px(origin.x as f32),
                            px((origin.y + index as f64 * font_size * scale * 1.25) as f32),
                        );
                    // Align the font's top edge with SVG text-before-edge; line spacing
                    // is separate from GPUI line-box padding.
                    if let Err(error) = line.paint(
                        position,
                        line.ascent + line.descent,
                        gpui::TextAlign::Left,
                        None,
                        window,
                        cx,
                    ) {
                        log::error!("Cannot paint annotation text: {error}");
                    }
                }
            }
        } else {
            paint_paths(&self.paths, transform, offset, window);
        }
    }
}

fn paint_preview(
    annotation: &Annotation,
    transform: ViewTransform,
    origin: Point<Pixels>,
    window: &mut Window,
) {
    paint_paths(&annotation_paths(annotation), transform, origin, window);
}

fn paint_paths(
    paths: &[(gpui::Path<Pixels>, Color)],
    transform: ViewTransform,
    origin: Point<Pixels>,
    window: &mut Window,
) {
    // Paths are tessellated once in document coordinates. Crop and zoom only
    // transform the retained vertices; they never resample the base image.
    let point = |position: Point<Pixels>| {
        origin
            + gpui_kit::point(
                px(
                    ((position.x.as_f32() as f64 - transform.crop.x as f64) * transform.width
                        / transform.crop.width as f64) as f32,
                ),
                px(
                    ((position.y.as_f32() as f64 - transform.crop.y as f64) * transform.height
                        / transform.crop.height as f64) as f32,
                ),
            )
    };
    for (path, color) in paths {
        let mut path = path.clone();
        let end = point(path.bounds.bottom_right());
        path.bounds.origin = point(path.bounds.origin);
        path.bounds.size =
            gpui_kit::size(end.x - path.bounds.origin.x, end.y - path.bounds.origin.y);
        for vertex in &mut path.vertices {
            vertex.xy_position = point(vertex.xy_position);
        }
        window.paint_path(path, rgba(u32::from_be_bytes(color.0)));
    }
}

/// Tessellate the shared [`Outline`] in document coordinates. The shape rules
/// live in rotor-canvas so the export renders the same geometry.
fn annotation_paths(annotation: &Annotation) -> Vec<(gpui::Path<Pixels>, Color)> {
    let mut paths = Vec::new();
    let Some((outline, color)) = annotation.outline() else {
        return paths;
    };
    let point = |point: ImagePoint| gpui_kit::point(px(point.x as f32), px(point.y as f32));
    let trace = |builder: &mut PathBuilder, points: &[ImagePoint]| {
        for (index, position) in points.iter().enumerate() {
            if index == 0 {
                builder.move_to(point(*position));
            } else {
                builder.line_to(point(*position));
            }
        }
    };
    let built = match outline {
        Outline::Dot { center, radius } => {
            let center = point(center);
            let radius = px(radius as f32);
            let mut circle = PathBuilder::fill();
            circle.move_to(center + gpui_kit::point(radius, px(0.)));
            circle.arc_to(
                gpui_kit::point(radius, radius),
                px(0.),
                false,
                true,
                center - gpui_kit::point(radius, px(0.)),
            );
            circle.arc_to(
                gpui_kit::point(radius, radius),
                px(0.),
                false,
                true,
                center + gpui_kit::point(radius, px(0.)),
            );
            circle.close();
            circle.build()
        }
        Outline::Stroke {
            points,
            width,
            closed,
            rounded,
        } => {
            let mut options = gpui::StrokeOptions::default().with_line_width(width as f32);
            if rounded {
                options = options
                    .with_line_cap(lyon::path::LineCap::Round)
                    .with_line_join(lyon::path::LineJoin::Round);
            }
            let mut path =
                PathBuilder::stroke(px(width as f32)).with_style(gpui::PathStyle::Stroke(options));
            trace(&mut path, &points);
            if closed {
                path.close();
            }
            path.build()
        }
        Outline::Fill(points) => {
            let mut fill = PathBuilder::fill();
            trace(&mut fill, &points);
            fill.close();
            fill.build()
        }
    };
    if let Ok(path) = built {
        paths.push((path, color));
    }
    paths
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
    use super::CanvasState;

    #[test]
    fn arrow_overlay_is_one_filled_shape_without_a_tip_cap() {
        let paths = super::annotation_paths(&rotor_canvas::Annotation::Arrow {
            start: rotor_canvas::ImagePoint { x: 10., y: 20. },
            end: rotor_canvas::ImagePoint { x: 65., y: 20. },
            style: rotor_canvas::StrokeStyle {
                color: rotor_canvas::Color::RED,
                width: 6.,
            },
        });
        assert_eq!(paths.len(), 1);
        assert!(
            paths[0]
                .0
                .vertices
                .iter()
                .all(|vertex| vertex.xy_position.x.as_f32() <= 65.)
        );
    }

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

    fn state() -> CanvasState {
        let image = crate::prepare_image(Arc::new(image::RgbaImage::from_pixel(
            4,
            4,
            image::Rgba([10, 20, 30, 128]),
        )))
        .unwrap();
        let record = ShotterConfig {
            annotations: Vec::new(),
            monitor_pos: (0, 0),
            monitor_size: (4, 4),
            rect: (0, 0, 4, 4),
            image_rect: (0, 0, 4, 4),
            offset: (0, 0),
            zoom_factor: 100,
            mask_label: "ssmask-1".into(),
            minimized: false,
        };
        CanvasState::new(&image, &record)
    }
    #[test]
    fn text_editor_stays_inside_small_and_edge_viewports() {
        for (width, height) in [(640., 480.), (120., 70.), (0., 0.)] {
            for origin in [
                ImagePoint { x: 0., y: 0. },
                ImagePoint {
                    x: width,
                    y: height,
                },
            ] {
                let (left, top, w, h) = super::text_editor_bounds(origin, width, height);
                assert!(left >= 0. && top >= 0.);
                assert!(left + w <= width && top + h <= height);
            }
        }
    }

    #[test]
    fn committed_marks_are_ready_and_export_snapshots_survive_undo() {
        let mut state = state();
        state
            .document
            .add(Annotation::Text {
                origin: ImagePoint { x: 1., y: 1. },
                text: "中文 text".into(),
                font_size: 16.,
                color: Color::RED,
            })
            .unwrap();
        let snapshot = state.export_scene();
        assert_eq!(snapshot.annotations.len(), 1);
        assert!(state.document.undo());
        assert!(state.export_scene().annotations.is_empty());
        assert_eq!(snapshot.annotations.len(), 1);
        assert!(state.document.redo());
        assert_eq!(state.export_scene().annotations, snapshot.annotations);
    }

    #[test]
    fn export_composes_marks_at_crop_resolution_without_a_display_frame() {
        let mut state = state();
        state
            .document
            .add(Annotation::Pen {
                points: vec![ImagePoint { x: 2., y: 2. }],
                style: StrokeStyle {
                    color: Color::RED,
                    width: 2.,
                },
            })
            .unwrap();
        state
            .document
            .set_crop(rotor_canvas::ImageRect {
                x: 1,
                y: 1,
                width: 2,
                height: 3,
            })
            .unwrap();
        let scene = state.export_scene();
        let source = image::RgbaImage::new(4, 4);
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
        assert_eq!(rendered.dimensions(), (2, 3));
        assert!(rendered.pixels().any(|pixel| pixel[0] > 0));
        assert!(state.document.undo());
        assert_eq!(state.export_scene().crop.width, 4);
        assert_eq!(state.export_scene().annotations.len(), 1);
    }
    // Mock GPUI exercises input/focus and retained display data, not native visual fidelity.
    #[gpui::test]
    fn text_confirmation_and_tool_switch_do_not_wait_for_rendering(cx: &mut gpui::TestAppContext) {
        use super::*;
        use gpui_kit::component::Root;
        use std::sync::Mutex;
        let profile = tempfile::tempdir().unwrap();
        let (services, _events) = Services::new(
            Arc::new(Mutex::new(
                rotor_common::ConfigService::load_from(profile.path()).unwrap(),
            )),
            None,
            rotor_runtime::ServiceOptions { index_files: false },
        )
        .unwrap();
        let services = Arc::new(services);
        // Any accidental offscreen render would fail: editing must be independent.
        services.shutdown();
        cx.update(gpui_kit::component::init);
        let mut pin = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let entity = cx.new(|cx| {
                PinView::new(
                    services,
                    super::super::PinInit {
                        image: crate::prepare_image(Arc::new(image::RgbaImage::new(400, 400)))
                            .unwrap(),
                        config: ShotterConfig {
                            annotations: Vec::new(),
                            monitor_pos: (0, 0),
                            monitor_size: (400, 400),
                            rect: (0, 0, 400, 400),
                            image_rect: (0, 0, 400, 400),
                            offset: (0, 0),
                            zoom_factor: 100,
                            mask_label: "ssmask-test".into(),
                            minimized: false,
                        },
                        id: None,
                        pending: None,
                        error: None,
                        content_scale: 1.,
                        position: Rc::new(|_| None),
                        minimized: Rc::new(|_| None),
                        activate: Rc::new(|_| Ok(())),
                        bounds: Rc::new(|_, _| Ok(())),
                        pointer: Rc::new(|_, _| Ok(())),
                        cursor: Rc::new(|_| Some((0., 0.))),
                    },
                    window,
                    cx,
                )
            });
            pin = Some(entity.clone());
            Root::new(entity, window, cx)
        });
        let pin = pin.unwrap();
        cx.update(|window, cx| {
            pin.update(cx, |pin, cx| {
                pin.set_tool(Tool::Text, window, cx);
                for text in ["中文 first", "second line"] {
                    pin.begin_mark(point(px(20.), px(20.)), window, cx);
                    let editor = pin.mode.text_editor().unwrap().input.clone();
                    editor.update(cx, |input, cx| input.set_value(text, window, cx));
                    pin.finish_text(window, cx);
                    assert!(pin.mode.text_editor().is_none());
                    assert!(pin.can_request_export());
                    assert!(pin.canvas.error.is_none());
                    let previous = pin.canvas.display.first().cloned();
                    pin.canvas_element(window, cx);
                    if let Some(previous) = previous {
                        assert!(Arc::ptr_eq(&previous, &pin.canvas.display[0]));
                    }
                }
                let display = pin.canvas.display.clone();
                pin.canvas_element(window, cx);
                assert!(Arc::ptr_eq(&display, &pin.canvas.display));
                // A native viewport can still have the old size while the crop
                // has advanced. Fractional zoom must not stretch the content or
                // invalidate retained text shaping as crop dimensions change.
                let record = pin.record.clone();
                pin.record.zoom_factor = 67;
                pin.content_scale = 2.;
                pin.canvas_element(window, cx);
                let fractional_display = pin.canvas.display.clone();
                for inset in [1, 2, 3, 17, 99] {
                    pin.record.rect = (inset, inset, 400 - inset, 400 - inset);
                    let transform = pin.transform();
                    let a = transform.to_view(ImagePoint { x: 150., y: 150. }).unwrap();
                    let b = transform.to_view(ImagePoint { x: 250., y: 250. }).unwrap();
                    assert!((b.x - a.x - 33.5).abs() < 1e-9);
                    assert!((b.y - a.y - 33.5).abs() < 1e-9);
                    pin.canvas_element(window, cx);
                    assert!(Arc::ptr_eq(&fractional_display, &pin.canvas.display));
                }
                pin.record = record;
                pin.content_scale = 1.;
                assert_eq!(display.len(), 2);
                assert_eq!(display[1].lines.len(), 1);
                pin.undo_canvas(window, cx);
                assert_eq!(pin.canvas.export_scene().annotations.len(), 1);
                pin.begin_mark(point(px(30.), px(30.)), window, cx);
                pin.mode
                    .text_editor()
                    .unwrap()
                    .input
                    .clone()
                    .update(cx, |input, cx| input.set_value("switch", window, cx));
                pin.set_tool(Tool::Pen, window, cx);
                assert_eq!(pin.mode.editing().map(Editing::tool), Some(Tool::Pen));
                assert!(pin.mode.text_editor().is_none());
                assert_eq!(pin.canvas.export_scene().annotations.len(), 2);
                pin.id = Some(42);
                let (id, record) = pin.shutdown_record().unwrap();
                assert_eq!(id, 42);
                let restored = CanvasState::new(&pin.image, &record);
                assert_eq!(
                    restored.export_scene().annotations,
                    pin.canvas.export_scene().annotations
                );
                assert_eq!(restored.export_scene().crop, pin.canvas.export_scene().crop);
                pin.undo_canvas(window, cx);
                let (_, record) = pin.shutdown_record().unwrap();
                assert_eq!(record.annotations.len(), 1);
            });
            window.draw(cx).clear(cx);
        });
        cx.run_until_parked();
        pin.read_with(cx, |pin, _| {
            assert!(pin.canvas.error.is_none());
            assert!(pin.can_request_export());
        });
    }
}
