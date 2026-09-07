use super::*;
use rotor_canvas::{CropEdges, ImagePoint, ImageRect, ImageSize};

pub(super) struct CropDrag {
    start: ImageRect,
    edges: CropEdges,
    pointer: ImagePoint,
    bounds: PinBounds,
    scale: f32,
    ratio: f64,
    record: ShotterConfig,
}
impl PinView {
    pub(super) fn committed_crop_record(&self) -> ShotterConfig {
        self.crop_drag
            .as_ref()
            .map(|drag| &drag.record)
            .unwrap_or(&self.record)
            .clone()
    }
    pub(super) fn crop_edges(&self, local: Point<Pixels>, window: &Window) -> CropEdges {
        let size = window.viewport_size();
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
        if let Err(error) = self.apply_crop_at(crop, x, y, window) {
            self.message = error;
        }
        self.ensure_canvas(window, cx);
        cx.notify();
    }
    fn apply_crop_at(
        &mut self,
        crop: ImageRect,
        x: f64,
        y: f64,
        window: &Window,
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
        )?;
        self.dirty = true;
        self.record_position(window, cx);
        self.flush(cx);
        Ok(())
    }
    pub(super) fn finish_crop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
        if let Err(error) = self.apply_crop(crop, window, cx) {
            self.message = error;
        }
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
        self.ensure_canvas(window, cx);
        cx.notify();
        true
    }
}
