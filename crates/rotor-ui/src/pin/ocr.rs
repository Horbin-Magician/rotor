use super::*;
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
    revision: u64,
    pub(super) signature: Option<(u64, ImageRect)>,
    size: Option<ImageSize>,
    rows: Vec<OcrTextResult>,
    lines: Vec<ShapedLine>,
    selection: Option<Selection>,
    dragging: bool,
    pub(super) error: Option<String>,
}
impl OcrState {
    fn accepts(&self, id: OperationId, revision: u64, signature: (u64, ImageRect)) -> bool {
        self.active
            && self.pending == Some(id)
            && self.revision == revision
            && self.signature == Some(signature)
    }
    fn text(&self, all: bool) -> String {
        if all {
            return self
                .rows
                .iter()
                .map(|row| row.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
        }
        selected_text(&self.rows, self.selection)
    }
}
fn boundary(text: &str, byte: usize) -> usize {
    let mut byte = byte.min(text.len());
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}
fn selected_text(rows: &[OcrTextResult], selection: Option<Selection>) -> String {
    let Some(selection) = selection else {
        return String::new();
    };
    let (start, end) = selection.ordered();
    if start.row >= rows.len() || end.row >= rows.len() {
        return String::new();
    }
    let mut selected = Vec::new();
    for (row, item) in rows.iter().enumerate().take(end.row + 1).skip(start.row) {
        let from = if row == start.row {
            boundary(&item.text, start.byte)
        } else {
            0
        };
        let to = if row == end.row {
            boundary(&item.text, end.byte)
        } else {
            item.text.len()
        };
        if from <= to {
            selected.push(&item.text[from..to]);
        }
    }
    selected.join("\n")
}
impl PinView {
    pub(super) fn clear_ocr(&mut self, window: &mut Window) {
        if self.ocr.dragging {
            self.release_native_pointer(window);
            window.release_pointer();
        }
        self.ocr = OcrState::default();
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
    pub(super) fn start_ocr(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() || !self.canvas.ready() || self.crop_drag.is_some() {
            return;
        }
        self.cancel_editing(window, cx);
        self.clear_ocr(window);
        let Some(frame) = self.canvas.frame() else {
            return;
        };
        let image = frame.image.clone();
        let revision = self.canvas.frame_revision();
        self.ocr.active = true;
        self.ocr.revision = revision;
        self.ocr.signature = Some(self.ocr_signature());
        self.ocr.size = Some(ImageSize {
            width: image.width(),
            height: image.height(),
        });
        match self
            .services
            .recognize_text(self.id.unwrap_or(0), revision, image)
        {
            Ok(id) => self.ocr.pending = Some(id),
            Err(error) => self.ocr.error = Some(error),
        }
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
            }
            Err(error) => self.ocr.error = Some(error.clone()),
        }
        cx.notify();
    }
    fn ocr_copy(&mut self, all: bool, cx: &mut Context<Self>) {
        let text = self.ocr.text(all);
        if !text.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
            self.message = self.t("文字已复制", "Text copied").into();
            cx.notify();
        }
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
        } else if event.keystroke.modifiers.control || event.keystroke.modifiers.platform {
            if event.keystroke.key.eq_ignore_ascii_case("c") {
                self.ocr_copy(false, cx);
                cx.stop_propagation();
            } else if event.keystroke.key.eq_ignore_ascii_case("a") && !self.ocr.rows.is_empty() {
                let last = self.ocr.rows.len() - 1;
                self.ocr.selection = Some(Selection {
                    anchor: TextPosition { row: 0, byte: 0 },
                    head: TextPosition {
                        row: last,
                        byte: self.ocr.rows[last].text.len(),
                    },
                });
                cx.stop_propagation();
                cx.notify();
            }
        }
        true
    }
    pub(super) fn ocr_tools(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_wrap()
            .gap_1()
            .child(
                Button::new("ocr-copy-all")
                    .label(self.t("全部复制", "Copy all"))
                    .compact()
                    .disabled(self.ocr.rows.is_empty())
                    .on_click(cx.listener(|this, _, _, cx| this.ocr_copy(true, cx))),
            )
            .child(
                Button::new("ocr-copy-selection")
                    .label(self.t("复制选区", "Copy selection"))
                    .compact()
                    .disabled(self.ocr.text(false).is_empty())
                    .on_click(cx.listener(|this, _, _, cx| this.ocr_copy(false, cx))),
            )
            .child(
                Button::new("ocr-retry")
                    .icon(IconName::RotateCw)
                    .accessibility_label(self.t("重新识别", "Recognize again"))
                    .tooltip(self.t("重新识别", "Recognize again"))
                    .compact()
                    .disabled(self.ocr.pending.is_some() || !self.canvas.ready())
                    .on_click(cx.listener(|this, _, window, cx| this.start_ocr(window, cx))),
            )
            .child(
                Button::new("ocr-exit")
                    .label(self.t("退出 OCR", "Exit OCR"))
                    .compact()
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.clear_ocr(window);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .w_full()
                    .text_xs()
                    .text_color(if self.ocr.error.is_some() {
                        cx.theme().danger
                    } else {
                        cx.theme().muted_foreground
                    })
                    .child(if self.ocr.pending.is_some() {
                        self.t("识别中…", "Recognizing…").to_owned()
                    } else if let Some(error) = &self.ocr.error {
                        error.clone()
                    } else if self.ocr.rows.is_empty() {
                        self.t("未识别到文字", "No text detected").into()
                    } else {
                        self.t(
                            "拖选文字 · 双击选行",
                            "Drag to select · Double-click a line",
                        )
                        .into()
                    }),
            )
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
            cx.notify();
            return false;
        };
        window.activate_window();
        self.focus.focus(window, cx);
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
            cx.notify();
        }
    }
    pub(super) fn ocr_layer(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.ocr.rows.clone();
        let lines = self.ocr.lines.clone();
        let selection = self.ocr.selection;
        let dragging = self.ocr.dragging;
        let size = self.ocr.size.unwrap_or(ImageSize {
            width: 1,
            height: 1,
        });
        let viewport = window.viewport_size();
        let sx = viewport.width.as_f32() / size.width as f32;
        let sy = viewport.height.as_f32() / size.height as f32;
        let view = cx.weak_entity();
        canvas(
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
        .size_full()
    }
}

#[cfg(test)]
mod tests {
    use super::{OcrState, Selection, TextPosition, selected_text};
    use rotor_canvas::ImageRect;
    use rotor_runtime::{OcrTextResult, OperationId};
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
    fn selections_cross_lines_and_never_slice_inside_utf8() {
        let selection = Selection {
            anchor: TextPosition { row: 1, byte: 3 },
            head: TextPosition { row: 0, byte: 3 },
        };
        assert_eq!(selected_text(&rows(), Some(selection)), "文hello\n第");
        let selection = Selection {
            anchor: TextPosition { row: 0, byte: 1 },
            head: TextPosition { row: 0, byte: 5 },
        };
        assert_eq!(selected_text(&rows(), Some(selection)), "中");
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
        assert!(!state.accepts(OperationId(1), 7, (3, crop)));
        assert!(!state.accepts(OperationId(2), 7, (4, crop)));
        assert!(!OcrState::default().accepts(OperationId(2), 7, (3, crop)));
    }
}
