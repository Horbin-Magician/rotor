use super::*;
use rotor_canvas::{CropEdges, ImagePoint, ImageRect, ImageSize};

pub(super) struct MoveDrag {
    pointer: ImagePoint,
    bounds: PinBounds,
}

pub(super) struct CropDrag {
    start: ImageRect,
    edges: CropEdges,
    pointer: ImagePoint,
    bounds: PinBounds,
    scale: f32,
    ratio: f64,
    record: ShotterConfig,
    pending: Option<CropUpdate>,
    frame_token: Rc<()>,
}

#[derive(Clone, Copy)]
struct CropUpdate {
    crop: ImageRect,
    x: f64,
    y: f64,
}
impl PinView {
    pub(super) fn begin_move(
        &mut self,
        local: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.ocr.active || self.canvas.editing() {
            return false;
        }
        let Some(bounds) = self.current_bounds(window) else {
            return false;
        };
        let Some(pointer) = self.screen_pointer(local, window) else {
            return false;
        };
        if !self.capture_native_pointer(window) {
            return false;
        }
        self.move_drag = Some(MoveDrag { pointer, bounds });
        cx.notify();
        true
    }

    pub(super) fn move_pin(
        &mut self,
        local: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = &self.move_drag else { return };
        let Some(pointer) = self.screen_pointer(local, window) else {
            return;
        };
        let mut bounds = drag.bounds;
        bounds.x = (bounds.x as f64 + pointer.x - drag.pointer.x).round() as i32;
        bounds.y = (bounds.y as f64 + pointer.y - drag.pointer.y).round() as i32;
        if let Err(error) = (self.bounds)(window, bounds) {
            self.message = error;
        }
        sync_native_bounds(window, cx);
        self.record_position(window, cx);
        cx.notify();
    }

    pub(super) fn finish_move(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.move_drag.take().is_none() {
            return;
        }
        window.release_pointer();
        self.release_native_pointer(window);
        self.record_position(window, cx);
        self.flush(cx);
        cx.notify();
    }

    pub(super) fn committed_crop_record(&self) -> ShotterConfig {
        self.crop_drag
            .as_ref()
            .map(|drag| &drag.record)
            .unwrap_or(&self.record)
            .clone()
    }
    pub(super) fn crop_edges(&self, local: Point<Pixels>, window: &Window) -> CropEdges {
        let size = window.viewport_size();
        if self.ocr.active || self.canvas.editing() {
            return CropEdges::default();
        }
        if size.width < px(24.) || size.height < px(24.) {
            return CropEdges::default();
        }
        let left = local.x <= px(8.);
        let top = local.y <= px(8.);
        CropEdges {
            left,
            top,
            right: !left && local.x >= size.width - px(8.),
            bottom: !top && local.y >= size.height - px(8.),
        }
    }
    pub(super) fn crop_cursor(&self) -> CursorStyle {
        if self.move_drag.is_some() {
            return CursorStyle::ClosedHand;
        }
        let edges = self
            .crop_drag
            .as_ref()
            .map(|drag| drag.edges)
            .unwrap_or(self.crop_hover);
        if (edges.left && edges.top) || (edges.right && edges.bottom) {
            CursorStyle::ResizeUpLeftDownRight
        } else if (edges.right && edges.top) || (edges.left && edges.bottom) {
            CursorStyle::ResizeUpRightDownLeft
        } else if edges.left || edges.right {
            CursorStyle::ResizeLeftRight
        } else if edges.top || edges.bottom {
            CursorStyle::ResizeUpDown
        } else {
            CursorStyle::OpenHand
        }
    }
    pub(super) fn capture_native_pointer(&mut self, window: &Window) -> bool {
        match (self.pointer)(window, true) {
            Ok(()) => {
                self.pointer_owned = true;
                true
            }
            Err(error) => {
                self.message = error;
                false
            }
        }
    }
    pub(super) fn release_native_pointer(&mut self, window: &Window) {
        if self.pointer_owned {
            let _ = (self.pointer)(window, false);
            self.pointer_owned = false;
        }
    }
    fn current_bounds(&self, window: &Window) -> Option<PinBounds> {
        let (x, y) = (self.position)(window)?;
        let size = window.viewport_size();
        let scale = window.scale_factor();
        Some(PinBounds {
            x,
            y,
            width: (size.width.as_f32() * scale).round().max(1.) as u32,
            height: (size.height.as_f32() * scale).round().max(1.) as u32,
        })
    }
    fn screen_pointer(&self, local: Point<Pixels>, window: &Window) -> Option<ImagePoint> {
        let (x, y) = (self.position)(window)?;
        let scale = window.scale_factor() as f64;
        Some(ImagePoint {
            x: x as f64 + local.x.as_f32() as f64 * scale,
            y: y as f64 + local.y.as_f32() as f64 * scale,
        })
    }
    pub(super) fn begin_crop(
        &mut self,
        local: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.ocr.active || self.canvas.editing() {
            return false;
        }
        let edges = self.crop_edges(local, window);
        if !edges.any() {
            return false;
        }
        let Some(bounds) = self.current_bounds(window) else {
            return false;
        };
        let Some(pointer) = self.screen_pointer(local, window) else {
            return false;
        };
        if !self.capture_native_pointer(window) {
            return false;
        }
        let (x, y, width, height) = self.crop();
        self.crop_drag = Some(CropDrag {
            start: ImageRect {
                x,
                y,
                width,
                height,
            },
            edges,
            pointer,
            bounds,
            scale: window.scale_factor(),
            ratio: window.scale_factor() as f64 / self.content_scale as f64
                * self.record.zoom_factor as f64
                / 100.,
            record: self.record.clone(),
            pending: None,
            frame_token: Rc::new(()),
        });
        cx.notify();
        true
    }
    pub(super) fn move_crop(
        &mut self,
        local: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = &self.crop_drag else {
            return;
        };
        if (window.scale_factor() - drag.scale).abs() > 0.01 {
            self.finish_crop(window, cx);
            self.message = self
                .t(
                    "显示缩放改变，已结束裁剪",
                    "Display scale changed; crop finished",
                )
                .into();
            return;
        }
        let Some(pointer) = self.screen_pointer(local, window) else {
            return;
        };
        let delta = ImagePoint {
            x: (pointer.x - drag.pointer.x) / drag.ratio,
            y: (pointer.y - drag.pointer.y) / drag.ratio,
        };
        let size = ImageSize {
            width: self.image.image.width(),
            height: self.image.image.height(),
        };
        let Some(crop) = rotor_canvas::resize_crop(drag.start, drag.edges, delta, size, 12) else {
            return;
        };
        let x = drag.bounds.x as f64 + (crop.x as f64 - drag.start.x as f64) * drag.ratio;
        let y = drag.bounds.y as f64 + (crop.y as f64 - drag.start.y as f64) * drag.ratio;
        let drag = self.crop_drag.as_mut().unwrap();
        let schedule = drag.pending.replace(CropUpdate { crop, x, y }).is_none();
        if schedule {
            let token = drag.frame_token.clone();
            let view = cx.weak_entity();
            window.on_next_frame(move |window, cx| {
                let _ = view.update(cx, |this, cx| {
                    // A released/cancelled drag must not resize a later drag.
                    if this
                        .crop_drag
                        .as_ref()
                        .is_some_and(|drag| Rc::ptr_eq(&drag.frame_token, &token))
                    {
                        this.apply_pending_crop(window, cx);
                    }
                });
            });
        }
    }

    fn apply_pending_crop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(update) = self.crop_drag.as_mut().and_then(|drag| drag.pending.take()) else {
            return;
        };
        if self
            .crop_drag
            .as_ref()
            .is_some_and(|drag| (window.scale_factor() - drag.scale).abs() > 0.01)
        {
            // A queued position was measured using the previous display scale.
            // Keep the last applied crop when crossing a DPI boundary.
            return;
        }
        if self.crop()
            == (
                update.crop.x,
                update.crop.y,
                update.crop.width,
                update.crop.height,
            )
        {
            return;
        }
        if let Err(error) = self.apply_crop_at(update.crop, update.x, update.y, window, cx) {
            self.message = error;
        }
        self.clear_ocr(window);
        cx.notify();
    }
    fn apply_crop_at(
        &mut self,
        crop: ImageRect,
        x: f64,
        y: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let factor = self.record.zoom_factor as f64 / 100. / self.content_scale as f64;
        let width =
            ((crop.width as f64 * factor).round().max(1.) * window.scale_factor() as f64).round();
        let height =
            ((crop.height as f64 * factor).round().max(1.) * window.scale_factor() as f64).round();
        if width > 8192.
            || height > 8192.
            || x < i32::MIN as f64
            || x > i32::MAX as f64
            || y < i32::MIN as f64
            || y > i32::MAX as f64
        {
            return Err(self
                .t(
                    "贴图尺寸过大，请先缩小缩放",
                    "Pin is too large; reduce zoom first",
                )
                .into());
        }
        let before = self.current_bounds(window);
        let bounds = PinBounds {
            x: x.round() as i32,
            y: y.round() as i32,
            width: width as u32,
            height: height as u32,
        };
        if let Err(error) = (self.bounds)(window, bounds) {
            if let Some(before) = before {
                let _ = (self.bounds)(window, before);
            }
            sync_native_bounds(window, cx);
            return Err(error);
        }
        let (origin_x, origin_y) = self
            .record
            .image_rect
            .map(|rect| (rect.0, rect.1))
            .unwrap_or((0, 0));
        self.record.rect = (
            origin_x.checked_add(crop.x).ok_or("Crop X overflow")?,
            origin_y.checked_add(crop.y).ok_or("Crop Y overflow")?,
            crop.width,
            crop.height,
        );
        // Native resize callbacks can run while GPUI already borrows this window.
        // Synchronize the viewport before the next canvas layout.
        sync_native_bounds(window, cx);
        Ok(())
    }
    pub(super) fn apply_crop(
        &mut self,
        crop: ImageRect,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let current = self
            .current_bounds(window)
            .ok_or("Pin window position is unavailable")?;
        let (x, y, _, _) = self.crop();
        let ratio = window.scale_factor() as f64 / self.content_scale as f64
            * self.record.zoom_factor as f64
            / 100.;
        self.apply_crop_at(
            crop,
            current.x as f64 + (crop.x as f64 - x as f64) * ratio,
            current.y as f64 + (crop.y as f64 - y as f64) * ratio,
            window,
            cx,
        )?;
        self.dirty = true;
        self.record_position(window, cx);
        self.flush(cx);
        Ok(())
    }
    pub(super) fn finish_crop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Mouse-up may precede the next frame: commit the latest queued position.
        self.apply_pending_crop(window, cx);
        if self.crop_drag.take().is_none() {
            return;
        }
        self.release_native_pointer(window);
        window.release_pointer();
        let (x, y, width, height) = self.crop();
        let crop = ImageRect {
            x,
            y,
            width,
            height,
        };
        self.dirty = true;
        self.record_position(window, cx);
        self.flush(cx);
        self.commit_canvas_crop(crop, window, cx);
        cx.notify();
    }
    pub(super) fn cancel_crop(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self.crop_drag.take() else {
            return false;
        };
        self.release_native_pointer(window);
        window.release_pointer();
        match (self.bounds)(window, drag.bounds) {
            Ok(()) => {
                self.record = drag.record;
                self.dirty = true;
                self.record_position(window, cx);
                self.flush(cx);
            }
            Err(error) => {
                self.message = error;
                let (x, y, width, height) = self.crop();
                self.commit_canvas_crop(
                    ImageRect {
                        x,
                        y,
                        width,
                        height,
                    },
                    window,
                    cx,
                );
            }
        }
        sync_native_bounds(window, cx);
        self.ensure_canvas_after_bounds(window, cx);
        cx.notify();
        true
    }
}

fn sync_native_bounds(window: &mut Window, cx: &mut App) {
    // Bounds observers update PinView, so run after its mutable borrow ends.
    window.defer(cx, |window, cx| window.bounds_changed(cx));
}

#[cfg(test)]
mod tests {
    use super::sync_native_bounds;
    use gpui_kit::{
        AppContext, Context, IntoElement, Pixels, Render, Size, Subscription, Window, div, point,
        px, size,
    };
    use std::{
        cell::Cell,
        rc::Rc,
        sync::{Arc, Mutex},
    };

    #[gpui::test]
    fn crop_drag_coalesces_frames_and_flushes_or_discards_pending_updates(
        cx: &mut gpui::TestAppContext,
    ) {
        let profile = tempfile::tempdir().unwrap();
        let (services, _events) = rotor_runtime::Services::new(
            Arc::new(Mutex::new(
                rotor_common::ConfigService::load_from(profile.path()).unwrap(),
            )),
            None,
            rotor_runtime::ServiceOptions { index_files: false },
        )
        .unwrap();
        let native = Rc::new(Cell::new(super::PinBounds {
            x: 0,
            y: 0,
            width: 400,
            height: 400,
        }));
        let calls = Rc::new(Cell::new(0));
        let mut pin = None;
        let handle = cx.add_window(|window, cx| {
            let read = native.clone();
            let write = native.clone();
            let calls = calls.clone();
            pin = Some(cx.new(|cx| {
                super::PinView::new(
                    Arc::new(services),
                    super::super::PinInit {
                        image: crate::prepare_image(Arc::new(image::RgbaImage::new(400, 400)))
                            .unwrap(),
                        config: rotor_runtime::ShotterConfig {
                            monitor_pos: (0, 0),
                            monitor_size: (400, 400),
                            rect: (0, 0, 400, 400),
                            image_rect: None,
                            offset: (0, 0),
                            zoom_factor: 100,
                            mask_label: "ssmask-1".into(),
                            minimized: false,
                        },
                        id: None,
                        pending: None,
                        error: None,
                        content_scale: 2.,
                        position: Rc::new(move |_| Some((read.get().x, read.get().y))),
                        minimized: Rc::new(|_| None),
                        bounds: Rc::new(move |_, bounds| {
                            write.set(bounds);
                            calls.set(calls.get() + 1);
                            Ok(())
                        }),
                        pointer: Rc::new(|_, _| Ok(())),
                    },
                    window,
                    cx,
                )
            }));
            BoundsView {
                observed: window.viewport_size(),
                _subscription: cx.observe_window_bounds(window, |_: &mut BoundsView, _, _| {}),
            }
        });
        let pin = pin.unwrap();
        gpui::AnyWindowHandle::from(handle)
            .update(cx, |_, window, cx| {
                window.resize(size(px(200.), px(200.)));
                window.bounds_changed(cx);
                pin.update(cx, |pin, cx| {
                    // OCR blocks geometry changes even when events bypass its text overlay.
                    pin.ocr.active = true;
                    assert!(!pin.begin_move(point(px(100.), px(100.)), window, cx));
                    assert!(!pin.begin_crop(point(px(0.), px(100.)), window, cx));
                    assert!(!pin.crop_edges(point(px(0.), px(100.)), window).any());
                    assert!(pin.move_drag.is_none());
                    assert!(pin.crop_drag.is_none());
                    assert_eq!(calls.get(), 0);
                    pin.toggle_ocr(window, cx);
                    assert!(!pin.ocr.active);
                    assert!(pin.begin_move(point(px(100.), px(100.)), window, cx));
                    pin.finish_move(window, cx);
                    pin.set_tool(super::super::annotation::Tool::Pen, window, cx);
                    assert!(pin.canvas.editing());
                    assert!(!pin.begin_move(point(px(100.), px(100.)), window, cx));
                    assert!(!pin.begin_crop(point(px(0.), px(100.)), window, cx));
                    pin.cancel_editing(window, cx);
                    assert!(!pin.canvas.editing());
                    assert!(pin.begin_crop(point(px(0.), px(100.)), window, cx));
                    let epoch = pin.canvas.content_revision();
                    for x in 1..=50 {
                        pin.move_crop(point(px(x as f32), px(100.)), window, cx);
                        pin.ensure_canvas(window, cx);
                    }
                    assert_eq!(calls.get(), 0);
                    assert_eq!(
                        pin.canvas.content_revision(),
                        epoch,
                        "dragging must not commit document changes"
                    );
                });
                window.simulate_next_frame(cx);
                assert_eq!(calls.get(), 1);
                assert_eq!((native.get().x, native.get().width), (100, 300));
                pin.update(cx, |pin, cx| {
                    // Screen X is 120: the window has already moved to X=100.
                    pin.move_crop(point(px(10.), px(100.)), window, cx);
                    pin.finish_crop(window, cx);
                    assert_eq!(pin.record.rect, (120, 0, 280, 400));
                });
                assert_eq!(
                    calls.get(),
                    2,
                    "release flushes the last position without a redundant resize"
                );

                pin.update(cx, |pin, cx| {
                    assert!(pin.begin_crop(point(px(0.), px(100.)), window, cx));
                    pin.move_crop(point(px(15.), px(100.)), window, cx);
                    pin.cancel_crop(window, cx);
                    assert_eq!(pin.record.rect, (120, 0, 280, 400));
                    assert!(pin.begin_crop(point(px(0.), px(100.)), window, cx));
                    pin.move_crop(point(px(20.), px(100.)), window, cx);
                });
                let before = calls.get();
                window.simulate_next_frame(cx);
                assert_eq!(
                    calls.get(),
                    before + 1,
                    "only the current drag's callback may resize"
                );
                assert_eq!((native.get().x, native.get().width), (160, 240));
                pin.update(cx, |pin, cx| {
                    pin.cancel_crop(window, cx);
                });
            })
            .unwrap();
    }

    struct BoundsView {
        observed: Size<Pixels>,
        _subscription: Subscription,
    }

    impl Render for BoundsView {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[gpui::test]
    fn native_crop_resize_syncs_viewport_after_releasing_view_borrow(
        cx: &mut gpui::TestAppContext,
    ) {
        let handle = cx.add_window(|window, cx| BoundsView {
            observed: window.viewport_size(),
            _subscription: cx.observe_window_bounds(window, |view: &mut BoundsView, window, _| {
                view.observed = window.viewport_size();
            }),
        });
        // TestWindow::resize changes the native size without dispatching a
        // callback, reproducing a resize during a borrowed GPUI window update.
        for dimensions in [
            size(px(160.), px(90.)),
            size(px(12.), px(24.)),
            size(px(320.), px(180.)),
        ] {
            handle
                .update(cx, |_, window, cx| {
                    window.resize(dimensions);
                    assert_ne!(window.viewport_size(), dimensions);
                    sync_native_bounds(window, cx);
                })
                .unwrap();
            cx.run_until_parked();
            handle
                .update(cx, |view, window, _| {
                    assert_eq!(window.viewport_size(), dimensions);
                    assert_eq!(view.observed, dimensions);
                })
                .unwrap();
        }
    }
}
