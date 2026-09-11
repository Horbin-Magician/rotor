use super::*;
use gpui_kit::component::input::{Textarea, TextareaState};
use rotor_canvas::{ImageRect, ImageSize};
use rotor_runtime::OcrTextResult;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct TextPosition {
    row: usize,
    byte: usize,
}
#[derive(Clone, Copy, Debug)]
struct Selection {
    anchor: TextPosition,
    head: TextPosition,
}
impl Selection {
    fn ordered(self) -> (TextPosition, TextPosition) {
        if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        }
    }
}
#[derive(Default)]
pub(super) struct OcrState {
    pub(super) active: bool,
    pending: Option<OperationId>,
    render_task: Option<Task<()>>,
    revision: u64,
    pub(super) signature: Option<(u64, ImageRect)>,
    size: Option<ImageSize>,
    rows: Vec<OcrTextResult>,
    lines: Vec<ShapedLine>,
    selection: Option<Selection>,
    input: Option<Entity<TextareaState>>,
    _input_observer: Option<Subscription>,
    dragging: bool,
    pub(super) error: Option<String>,
}
impl OcrState {
    pub(super) fn loading(&self) -> bool {
        self.render_task.is_some() || self.pending.is_some()
    }

    fn accepts_render(&self, revision: u64, signature: (u64, ImageRect)) -> bool {
        self.active && self.revision == revision && self.signature == Some(signature)
    }

    fn accepts(&self, id: OperationId, revision: u64, signature: (u64, ImageRect)) -> bool {
        self.active
            && self.pending == Some(id)
            && self.revision == revision
            && self.signature == Some(signature)
    }
}
fn boundary(text: &str, byte: usize) -> usize {
    let mut byte = byte.min(text.len());
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}
fn text_offset(rows: &[OcrTextResult], position: TextPosition) -> usize {
    rows.iter()
        .take(position.row)
        .map(|row| row.text.len() + 1)
        .sum::<usize>()
        + boundary(&rows[position.row].text, position.byte)
}
fn text_position(rows: &[OcrTextResult], mut offset: usize) -> TextPosition {
    for (row, item) in rows.iter().enumerate() {
        if offset <= item.text.len() || row + 1 == rows.len() {
            return TextPosition {
                row,
                byte: boundary(&item.text, offset),
            };
        }
        offset -= item.text.len() + 1;
    }
    TextPosition { row: 0, byte: 0 }
}
impl PinView {
    pub(super) fn clear_ocr(&mut self, window: &mut Window) {
        if self.ocr.dragging {
            self.release_native_pointer(window);
            window.release_pointer();
        }
        self.ocr = OcrState {
            revision: self.ocr.revision.wrapping_add(1),
            ..Default::default()
        };
    }
    fn ocr_signature(&self) -> (u64, ImageRect) {
        let (x, y, width, height) = self.crop();
        (
            self.canvas.content_revision(),
            ImageRect {
                x,
                y,
                width,
                height,
            },
        )
    }
    pub(super) fn toggle_ocr(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.ocr.loading() {
            return;
        }
        if self.ocr.active {
            self.clear_ocr(window);
            self.focus.focus(window, cx);
            self.crop_hover = Default::default();
            cx.notify();
        } else {
            self.start_ocr(window, cx);
        }
    }
    pub(super) fn start_ocr(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || !self.canvas.ready() || self.crop_drag.is_some() {
            return;
        }
        self.finish_move(window, cx);
        self.crop_hover = Default::default();
        self.cancel_editing(window, cx);
        self.clear_ocr(window);
        let scene = self.canvas.export_scene();
        let output = ImageSize {
            width: scene.crop.width,
            height: scene.crop.height,
        };
        let source = self.image.image.clone();
        let services = self.services.clone();
        let revision = self.ocr.revision;
        let signature = self.ocr_signature();
        self.ocr.active = true;
        self.ocr.signature = Some(signature);
        self.ocr.size = Some(output);
        self.ocr.render_task = Some(cx.spawn_in(window, async move |view, cx| {
            // OCR consumes the same document as export, including floating annotations.
            let result = services.render_canvas(source, scene, output).await;
            let _ = view.update(cx, |this, cx| {
                if !this.ocr.accepts_render(revision, signature)
                    || this.ocr_signature() != signature
                {
                    return;
                }
                this.ocr.render_task = None;
                match result.and_then(|image| {
                    this.services
                        .recognize_text(this.id.unwrap_or(0), revision, image)
                }) {
                    Ok(id) => this.ocr.pending = Some(id),
                    Err(error) => this.ocr.error = Some(error),
                }
                cx.notify();
            });
        }));
        self.focus.focus(window, cx);
        cx.notify();
    }
    pub(super) fn ocr_event(
        &mut self,
        event: &RuntimeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let RuntimeEvent::OcrFinished {
            id,
            revision,
            result,
            ..
        } = event
        else {
            return;
        };
        if !self.ocr.accepts(*id, *revision, self.ocr_signature()) {
            return;
        }
        self.ocr.pending = None;
        match result {
            Ok(results) => {
                let size = self.ocr.size.unwrap();
                self.ocr.rows = results
                    .iter()
                    .filter(|row| {
                        row.width > 0
                            && row.height > 0
                            && !row.text.is_empty()
                            && row.left < size.width as i32
                            && row.top < size.height as i32
                            && row.left as i64 + row.width as i64 > 0
                            && row.top as i64 + row.height as i64 > 0
                    })
                    .cloned()
                    .collect();
                self.ocr.lines = self
                    .ocr
                    .rows
                    .iter()
                    .map(|row| {
                        let text: SharedString = row.text.replace(['\n', '\r'], " ").into();
                        window.text_system().shape_line(
                            text.clone(),
                            px((row.height as f32 * 0.6).clamp(1., 256.)),
                            &[TextRun {
                                len: text.len(),
                                font: font(rotor_canvas::FONT_FAMILY),
                                ..Default::default()
                            }],
                            None,
                        )
                    })
                    .collect();
                let text = self
                    .ocr
                    .rows
                    .iter()
                    .map(|row| row.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                let input = cx.new(|cx| TextareaState::new(window, cx).default_value(text));
                self.ocr._input_observer = Some(cx.observe(&input, |_, _, cx| cx.notify()));
                input.update(cx, |input, cx| input.focus(window, cx));
                self.ocr.input = Some(input);
            }
            Err(error) => self.ocr.error = Some(error.clone()),
        }
        cx.notify();
    }
    pub(super) fn ocr_keys(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.ocr.active {
            return false;
        }
        if event.keystroke.key == "escape" {
            self.clear_ocr(window);
            cx.stop_propagation();
            cx.notify();
            self.focus.focus(window, cx);
        }
        true
    }
    fn ocr_position(
        &self,
        point: Point<Pixels>,
        window: &Window,
        nearest: bool,
    ) -> Option<TextPosition> {
        let size = self.ocr.size?;
        let viewport = window.viewport_size();
        let x = point.x.as_f32() * size.width as f32 / viewport.width.as_f32().max(1.);
        let y = point.y.as_f32() * size.height as f32 / viewport.height.as_f32().max(1.);
        let row = self
            .ocr
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                nearest
                    || (x >= row.left as f32
                        && x <= row.left as f32 + row.width as f32
                        && y >= row.top as f32
                        && y <= row.top as f32 + row.height as f32)
            })
            .min_by(|(_, a), (_, b)| {
                let distance = |row: &OcrTextResult| {
                    let dx = (row.left as f32 - x)
                        .max(0.)
                        .max(x - row.left as f32 - row.width as f32);
                    let dy = (row.top as f32 - y)
                        .max(0.)
                        .max(y - row.top as f32 - row.height as f32);
                    dx * dx + dy * dy * 4.
                };
                distance(a).total_cmp(&distance(b))
            })
            .map(|(index, _)| index)?;
        let result = &self.ocr.rows[row];
        let line = &self.ocr.lines[row];
        let text_x = (x - result.left as f32) / result.width as f32 * line.width().as_f32();
        Some(TextPosition {
            row,
            byte: boundary(&result.text, line.closest_index_for_x(px(text_x))),
        })
    }
    fn ocr_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(position) = self.ocr_position(event.position, window, false) else {
            self.ocr.selection = None;
            if let Some(input) = &self.ocr.input {
                input.update(cx, |input, cx| input.set_selected_range(0..0, cx));
            }
            cx.notify();
            return false;
        };
        window.activate_window();
        if let Some(input) = &self.ocr.input {
            input.update(cx, |input, cx| input.focus(window, cx));
        }
        self.ocr.selection = Some(if event.click_count >= 2 {
            Selection {
                anchor: TextPosition {
                    row: position.row,
                    byte: 0,
                },
                head: TextPosition {
                    row: position.row,
                    byte: self.ocr.rows[position.row].text.len(),
                },
            }
        } else {
            Selection {
                anchor: position,
                head: position,
            }
        });
        self.sync_ocr_selection(cx);
        self.ocr.dragging = self.capture_native_pointer(window);
        cx.notify();
        self.ocr.dragging
    }
    fn ocr_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ocr.dragging {
            return;
        }
        if let Some(position) = self.ocr_position(event.position, window, true)
            && let Some(selection) = self.ocr.selection.as_mut()
        {
            selection.head = position;
            self.sync_ocr_selection(cx);
            cx.notify();
        }
    }
    fn sync_ocr_selection(&self, cx: &mut Context<Self>) {
        if let (Some(input), Some(selection)) = (&self.ocr.input, self.ocr.selection) {
            let (start, end) = selection.ordered();
            let range = text_offset(&self.ocr.rows, start)..text_offset(&self.ocr.rows, end);
            input.update(cx, |input, cx| input.set_selected_range(range, cx));
        }
    }
    pub(super) fn ocr_layer(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.ocr.rows.clone();
        let lines = self.ocr.lines.clone();
        let selection = self.ocr.input.as_ref().map(|input| {
            let range = input.read(cx).selected_range();
            Selection {
                anchor: text_position(&rows, range.start),
                head: text_position(&rows, range.end),
            }
        });
        let dragging = self.ocr.dragging;
        let size = self.ocr.size.unwrap_or(ImageSize {
            width: 1,
            height: 1,
        });
        let viewport = window.viewport_size();
        let sx = viewport.width.as_f32() / size.width as f32;
        let sy = viewport.height.as_f32() / size.height as f32;
        let view = cx.weak_entity();
        let overlay = canvas(
            |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::BlockMouse),
            move |bounds, hitbox, window, cx| {
                window.set_cursor_style(CursorStyle::IBeam, &hitbox);
                if dragging {
                    window.capture_pointer(hitbox.id);
                }
                for (index, row) in rows.iter().enumerate() {
                    let origin =
                        bounds.origin + point(px(row.left as f32 * sx), px(row.top as f32 * sy));
                    window.paint_quad(fill(
                        Bounds::new(
                            origin,
                            gpui_kit::size(px(row.width as f32 * sx), px(row.height as f32 * sy)),
                        ),
                        cx.theme().primary.opacity(0x22 as f32 / 255.),
                    ));
                    if let Some(selection) = selection {
                        let (start, end) = selection.ordered();
                        if index >= start.row && index <= end.row {
                            let from = if index == start.row {
                                boundary(&row.text, start.byte)
                            } else {
                                0
                            };
                            let to = if index == end.row {
                                boundary(&row.text, end.byte)
                            } else {
                                row.text.len()
                            };
                            let width = lines[index].width().as_f32().max(1.);
                            let x1 = lines[index].x_for_index(from).as_f32() / width
                                * row.width as f32
                                * sx;
                            let x2 = lines[index].x_for_index(to).as_f32() / width
                                * row.width as f32
                                * sx;
                            window.paint_quad(fill(
                                Bounds::new(
                                    origin + point(px(x1.min(x2)), px(0.)),
                                    gpui_kit::size(px((x2 - x1).abs()), px(row.height as f32 * sy)),
                                ),
                                cx.theme().primary.opacity(0x88 as f32 / 255.),
                            ));
                        }
                    }
                }
                let down = view.clone();
                let down_hitbox = hitbox.clone();
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble
                        && event.button == MouseButton::Left
                        && down_hitbox.is_hovered(window)
                    {
                        if down
                            .update(cx, |view, cx| view.ocr_down(event, window, cx))
                            .unwrap_or(false)
                        {
                            window.capture_pointer(down_hitbox.id);
                        }
                        cx.stop_propagation();
                    }
                });
                let moving = view.clone();
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble && hitbox.is_hovered(window) {
                        let _ = moving.update(cx, |view, cx| view.ocr_move(event, window, cx));
                    }
                });
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                        let _ = view.update(cx, |view, _| {
                            view.ocr.dragging = false;
                            view.release_native_pointer(window);
                            window.release_pointer();
                        });
                    }
                });
            },
        )
        .absolute()
        .top(px(0.))
        .left(px(0.))
        .size_full();
        // This sibling follows the full-height image. Explicit insets keep the
        // overlay and its hitbox at the image origin instead of its static position.
        div()
            .absolute()
            .top(px(0.))
            .left(px(0.))
            .size_full()
            .when_some(self.ocr.input.as_ref(), |root, input| {
                // Keep the control in the focus/action tree without painting its text.
                root.child(
                    div()
                        .absolute()
                        .size(px(1.))
                        .overflow_hidden()
                        .opacity(0.)
                        .child(Textarea::new(input).readonly(true).appearance(false)),
                )
            })
            .child(overlay)
    }
}

#[cfg(test)]
mod tests {
    use super::{OcrState, TextPosition, text_offset, text_position};
    use gpui_kit::component::input::{Textarea, TextareaState};
    use gpui_kit::{
        Context, Entity, IntoElement, Render, TestAppContext, Window, div, prelude::*, px,
    };
    use rotor_canvas::ImageRect;
    use rotor_runtime::{OcrTextResult, OperationId};
    #[gpui::test]
    fn image_mouse_drag_selects_hidden_textbox(cx: &mut TestAppContext) {
        use super::super::{PinInit, PinView};
        use gpui_kit::{MouseButton, point, size};
        use std::{
            rc::Rc,
            sync::{Arc, Mutex},
        };
        let profile = tempfile::tempdir().unwrap();
        let (services, _events) = rotor_runtime::Services::new(
            Arc::new(Mutex::new(
                rotor_common::ConfigService::load_from(profile.path()).unwrap(),
            )),
            None,
            rotor_runtime::ServiceOptions { index_files: false },
        )
        .unwrap();
        cx.update(gpui_kit::component::init);
        let (pin, cx) = cx.add_window_view(|window, cx| {
            window.resize(size(px(400.), px(400.)));
            let mut pin = PinView::new(
                Arc::new(services),
                PinInit {
                    image: crate::prepare_image(Arc::new(image::RgbaImage::new(400, 400))).unwrap(),
                    config: rotor_runtime::ShotterConfig {
                        monitor_pos: (0, 0),
                        monitor_size: (400, 400),
                        rect: (0, 0, 400, 400),
                        image_rect: (0, 0, 400, 400),
                        offset: (0, 0),
                        zoom_factor: 100,
                        mask_label: "synthetic".into(),
                        minimized: false,
                    },
                    id: None,
                    pending: None,
                    error: None,
                    content_scale: 1.,
                    position: Rc::new(|_| Some((0, 0))),
                    minimized: Rc::new(|_| None),
                    bounds: Rc::new(|_, _| Ok(())),
                    pointer: Rc::new(|_, _| Ok(())),
                },
                window,
                cx,
            );
            pin.ocr.active = true;
            pin.ocr.size = Some(rotor_canvas::ImageSize {
                width: 400,
                height: 400,
            });
            pin.ocr.rows = vec![OcrTextResult {
                left: 40,
                top: 40,
                width: 200,
                height: 30,
                text: "hello world".into(),
            }];
            pin.ocr.lines = vec![window.text_system().shape_line(
                "hello world".into(),
                px(18.),
                &[gpui_kit::TextRun {
                    len: 11,
                    font: gpui_kit::font(rotor_canvas::FONT_FAMILY),
                    ..Default::default()
                }],
                None,
            )];
            let input = cx.new(|cx| TextareaState::new(window, cx).default_value("hello world"));
            pin.ocr._input_observer = Some(cx.observe(&input, |_, _, cx| cx.notify()));
            pin.ocr.input = Some(input);
            pin
        });
        cx.simulate_resize(size(px(400.), px(400.)));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_mouse_move(point(px(40.), px(50.)), None, Default::default());
        cx.simulate_mouse_down(
            point(px(40.), px(50.)),
            MouseButton::Left,
            Default::default(),
        );
        pin.read_with(cx, |pin, _| {
            assert!(pin.ocr.dragging, "mouse down must reach OCR overlay")
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_mouse_move(
            point(px(240.), px(50.)),
            MouseButton::Left,
            Default::default(),
        );
        cx.simulate_mouse_up(
            point(px(240.), px(50.)),
            MouseButton::Left,
            Default::default(),
        );
        pin.read_with(cx, |pin, cx| {
            assert!(!pin.ocr.dragging);
            assert_eq!(
                pin.ocr.input.as_ref().unwrap().read(cx).selected_range(),
                0..11
            );
        });
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-c"
        } else {
            "ctrl-c"
        });
        cx.update(|_, cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("hello world".into())
            )
        });
    }
    #[gpui::test]
    fn hidden_readonly_textbox_handles_copy_and_select_all(cx: &mut TestAppContext) {
        struct Harness(Entity<TextareaState>);
        impl Render for Harness {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div().size_full().child(
                    div()
                        .absolute()
                        .size(px(1.))
                        .overflow_hidden()
                        .opacity(0.)
                        .child(Textarea::new(&self.0).readonly(true).appearance(false)),
                )
            }
        }
        cx.update(gpui_kit::component::init);
        let (view, cx) = cx.add_window_view(|window, cx| {
            let input =
                cx.new(|cx| TextareaState::new(window, cx).default_value("中文hello\n第二行"));
            input.update(cx, |input, cx| {
                input.focus(window, cx);
                input.set_selected_range(3..15, cx);
            });
            Harness(input)
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-c"
        } else {
            "ctrl-c"
        });
        cx.update(|_, cx| {
            assert_eq!(
                cx.read_from_clipboard().and_then(|item| item.text()),
                Some("文hello\n第".into())
            )
        });
        cx.simulate_keystrokes(if cfg!(target_os = "macos") {
            "cmd-a"
        } else {
            "ctrl-a"
        });
        cx.simulate_keystrokes("backspace");
        view.read_with(cx, |view, cx| {
            assert_eq!(view.0.read(cx).value().as_ref(), "中文hello\n第二行");
            assert_eq!(view.0.read(cx).selected_range(), 0..21);
        });
    }
    fn rows() -> Vec<OcrTextResult> {
        ["中文hello", "第二行"]
            .into_iter()
            .map(|text| OcrTextResult {
                left: 0,
                top: 0,
                width: 100,
                height: 20,
                text: text.into(),
            })
            .collect()
    }
    #[test]
    fn positions_map_to_multiline_textbox_utf8_offsets() {
        let rows = rows();
        for position in [
            TextPosition { row: 0, byte: 3 },
            TextPosition { row: 1, byte: 3 },
        ] {
            assert_eq!(text_position(&rows, text_offset(&rows, position)), position);
        }
        assert_eq!(text_offset(&rows, TextPosition { row: 1, byte: 0 }), 12);
        assert_eq!(text_position(&rows, 1), TextPosition { row: 0, byte: 0 });
    }

    #[test]
    fn loading_covers_render_and_recognition_and_clears_after_completion() {
        let mut state = OcrState {
            active: true,
            render_task: Some(gpui_kit::Task::ready(())),
            ..Default::default()
        };
        assert!(state.loading());
        state.render_task = None;
        state.pending = Some(OperationId(1));
        assert!(state.loading());
        state.pending = None;
        assert!(!state.loading());
        state.error = Some("recognition failed".into());
        assert!(!state.loading());
        assert!(!OcrState::default().loading());
    }

    #[test]
    fn stale_request_or_changed_image_cannot_restore_ocr_mode() {
        let crop = ImageRect {
            x: 0,
            y: 0,
            width: 100,
            height: 20,
        };
        let state = OcrState {
            active: true,
            pending: Some(OperationId(2)),
            revision: 7,
            signature: Some((3, crop)),
            ..Default::default()
        };
        assert!(state.accepts(OperationId(2), 7, (3, crop)));
        assert!(state.accepts_render(7, (3, crop)));
        assert!(!state.accepts_render(6, (3, crop)));
        assert!(!state.accepts_render(7, (4, crop)));
        assert!(!OcrState::default().accepts_render(7, (3, crop)));
        let restarted = OcrState {
            revision: 8,
            ..state
        };
        assert!(!restarted.accepts_render(7, (3, crop)));
        let state = OcrState {
            revision: 7,
            ..restarted
        };
        assert!(!state.accepts(OperationId(1), 7, (3, crop)));
        assert!(!state.accepts(OperationId(2), 7, (4, crop)));
        assert!(!OcrState::default().accepts(OperationId(2), 7, (3, crop)));
    }
}
