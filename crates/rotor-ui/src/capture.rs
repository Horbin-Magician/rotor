use gpui_kit::{component::ActiveTheme, prelude::*, *};
use image::RgbaImage;
use rotor_canvas::{ImagePoint, ImageRect, ImageSize};
use rotor_runtime::{CaptureBundle, MonitorConfig};
use std::{rc::Rc, sync::Arc};

#[derive(Clone)]
pub struct PreparedImage {
    pub image: Arc<RgbaImage>,
    pub render: Arc<RenderImage>,
}
pub fn prepare_image(image: Arc<RgbaImage>) -> Result<PreparedImage, String> {
    if image.width() == 0 || image.height() == 0 {
        return Err("Image is empty".into());
    }
    let mut bgra = image.as_ref().clone();
    for pixel in bgra.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Ok(PreparedImage {
        image,
        render: Arc::new(RenderImage::new(vec![image::Frame::new(bgra)])),
    })
}
pub struct PreparedCapture {
    pub monitor: MonitorConfig,
    pub image: PreparedScreenshot,
    windows: Vec<(i32, ImageRect)>,
}

pub struct PreparedScreenshot {
    pub render: Arc<RenderImage>,
    width: u32,
    height: u32,
}

impl PreparedScreenshot {
    fn new(image: rotor_runtime::BgraCapture) -> Result<Self, String> {
        let (width, height) = (image.width, image.height);
        if width == 0 || height == 0 {
            return Err("Image is empty".into());
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4));
        if expected != Some(image.bytes.len()) {
            return Err("Invalid BGRA capture size".into());
        }
        let bgra =
            RgbaImage::from_raw(width, height, image.bytes).ok_or("Invalid BGRA capture size")?;
        Ok(Self {
            render: Arc::new(RenderImage::new(vec![image::Frame::new(bgra)])),
            width,
            height,
        })
    }

    /// Copy only selected pixels at native resolution, preserving every channel.
    pub fn crop_rgba(&self, rect: ImageRect) -> Result<Arc<RgbaImage>, String> {
        if rect.width == 0
            || rect.height == 0
            || rect
                .x
                .checked_add(rect.width)
                .is_none_or(|x| x > self.width)
            || rect
                .y
                .checked_add(rect.height)
                .is_none_or(|y| y > self.height)
        {
            return Err("Capture source rectangle is invalid".into());
        }
        let bytes = self.render.as_bytes(0).expect("capture has one frame");
        Ok(Arc::new(RgbaImage::from_fn(
            rect.width,
            rect.height,
            |x, y| {
                let offset =
                    ((rect.y + y) as usize * self.width as usize + (rect.x + x) as usize) * 4;
                image::Rgba([
                    bytes[offset + 2],
                    bytes[offset + 1],
                    bytes[offset],
                    bytes[offset + 3],
                ])
            },
        )))
    }

    fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let offset = (y as usize * self.width as usize + x as usize) * 4;
        let bytes = self.render.as_bytes(0).expect("capture has one frame");
        [
            bytes[offset + 2],
            bytes[offset + 1],
            bytes[offset],
            bytes[offset + 3],
        ]
    }
}

impl rotor_runtime::CapturePixels for PreparedCapture {
    fn pixels(&self) -> (&[u8], u32, u32, rotor_runtime::PixelFormat) {
        (
            self.image
                .render
                .as_bytes(0)
                .expect("capture has one frame"),
            self.image.width,
            self.image.height,
            rotor_runtime::PixelFormat::Bgra,
        )
    }
}

impl PreparedCapture {
    /// Tiny inert content while a new hidden window is being initialized.
    pub fn placeholder(monitor: MonitorConfig) -> Arc<Self> {
        Arc::new(Self {
            monitor,
            image: PreparedScreenshot::new(rotor_runtime::BgraCapture {
                width: 1,
                height: 1,
                bytes: vec![0, 0, 0, 255],
            })
            .expect("valid placeholder"),
            windows: Vec::new(),
        })
    }
}
pub fn prepare_capture(bundle: CaptureBundle) -> Result<Vec<Arc<PreparedCapture>>, String> {
    bundle
        .monitors
        .into_iter()
        .map(|capture| {
            let monitor = capture.monitor;
            if (capture.image.width, capture.image.height) != (monitor.width, monitor.height) {
                return Err("Monitor image dimensions changed".into());
            }
            let dimensions = ImageSize {
                width: monitor.width,
                height: monitor.height,
            };
            let windows = bundle
                .windows
                .iter()
                .filter_map(|&(x, y, z, width, height)| {
                    // Quartz window bounds and display origins are in points.
                    // Convert after subtracting the origin so mixed-DPI displays
                    // use the scale of the capture being prepared.
                    let scale = if cfg!(target_os = "macos") {
                        monitor.scale_factor as f64
                    } else {
                        1.
                    };
                    ImageRect::from_drag(
                        ImagePoint {
                            x: (x as f64 - monitor.x as f64) * scale,
                            y: (y as f64 - monitor.y as f64) * scale,
                        },
                        ImagePoint {
                            x: (x as f64 + width as f64 - monitor.x as f64) * scale,
                            y: (y as f64 + height as f64 - monitor.y as f64) * scale,
                        },
                        dimensions,
                    )
                    .map(|rect| (z, rect))
                })
                .collect();
            Ok(Arc::new(PreparedCapture {
                monitor,
                image: PreparedScreenshot::new(capture.image)?,
                windows,
            }))
        })
        .collect()
}
#[derive(Clone, Copy)]
pub enum MaskAction {
    Activate {
        session: u64,
        monitor: u32,
    },
    Cancel {
        session: u64,
    },
    Invalidated {
        session: u64,
    },
    Choose {
        session: u64,
        monitor: u32,
        rect: ImageRect,
    },
}
pub type MaskCallback = Rc<dyn Fn(MaskAction, &mut Window, &mut App)>;

pub struct MaskView {
    session: u64,
    active: bool,
    capture: Arc<PreparedCapture>,
    callback: MaskCallback,
    point: ImagePoint,
    pointer_inside: bool,
    start: Option<ImagePoint>,
    click_selection: Option<ImageRect>,
    focus: FocusHandle,
    armed: bool,
    _bounds: Subscription,
    detected: Vec<ImageRect>,
    chinese: bool,
    copied: bool,
}
impl MaskView {
    pub fn new(
        session: u64,
        capture: Arc<PreparedCapture>,
        callback: MaskCallback,
        chinese: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        window.set_window_title(&format!(
            "Rotor · {} {}",
            if chinese { "截图" } else { "Capture" },
            capture.monitor.id
        ));
        let focus = cx.focus_handle();
        let bounds =
            cx.observe_window_bounds(window, |this, window, cx| this.check_geometry(window, cx));
        Self {
            session,
            active: session != 0,
            capture,
            callback,
            point: ImagePoint { x: 0., y: 0. },
            pointer_inside: false,
            start: None,
            click_selection: None,
            focus,
            armed: false,
            _bounds: bounds,
            detected: Vec::new(),
            chinese,
            copied: false,
        }
    }
    pub fn monitor(&self) -> &MonitorConfig {
        &self.capture.monitor
    }
    pub fn reset(
        &mut self,
        session: u64,
        capture: Arc<PreparedCapture>,
        chinese: bool,
        cx: &mut Context<Self>,
    ) {
        self.session = session;
        self.active = true;
        self.capture = capture;
        self.chinese = chinese;
        self.armed = false;
        self.point = ImagePoint { x: 0., y: 0. };
        self.pointer_inside = false;
        self.start = None;
        self.click_selection = None;
        self.detected.clear();
        self.copied = false;
        cx.notify();
    }
    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.focus.focus(window, cx);
    }
    pub fn set_cursor(
        &mut self,
        point: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(point) = ImagePoint::from_logical(
            point.x.as_f32() as f64,
            point.y.as_f32() as f64,
            window.scale_factor() as f64,
        ) {
            self.point = point;
            self.pointer_inside = self.contains_pointer(point);
            cx.notify();
        }
    }
    pub fn set_detected_rectangles(&mut self, rectangles: Vec<ImageRect>, cx: &mut Context<Self>) {
        let size = self.dimensions();
        self.detected = rectangles
            .into_iter()
            .filter_map(|rect| rect.clipped(size))
            .collect();
        cx.notify();
    }
    pub fn arm(&mut self, session: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.session != session || !self.active {
            return;
        }
        self.armed = true;
        self.check_geometry(window, cx);
    }
    fn check_geometry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.armed {
            return;
        }
        let scale = window.scale_factor();
        let viewport = window.viewport_size();
        let changed = (viewport.width.as_f32() * scale - self.capture.monitor.width as f32).abs()
            > 1.
            || (viewport.height.as_f32() * scale - self.capture.monitor.height as f32).abs() > 1.
            || (scale - self.capture.monitor.scale_factor).abs() > 0.01
            || window
                .display(cx)
                .is_none_or(|display| u64::from(display.id()) as u32 != self.capture.monitor.id);
        if changed {
            self.armed = false;
            let callback = self.callback.clone();
            callback(
                MaskAction::Invalidated {
                    session: self.session,
                },
                window,
                cx,
            );
        }
    }
    fn dimensions(&self) -> ImageSize {
        ImageSize {
            width: self.capture.image.width,
            height: self.capture.image.height,
        }
    }
    fn contains_pointer(&self, point: ImagePoint) -> bool {
        point.x >= 0.
            && point.y >= 0.
            && point.x < self.capture.image.width as f64
            && point.y < self.capture.image.height as f64
    }
    fn auto_selection(&self) -> Option<ImageRect> {
        choose_rectangle(
            self.point,
            self.capture
                .windows
                .iter()
                .copied()
                .chain(self.detected.iter().map(|rect| (-1, *rect))),
        )
    }
    fn selected(&self) -> Option<ImageRect> {
        if let Some(start) = self.start {
            ImageRect::from_drag(start, self.point, self.dimensions()).or(self.click_selection)
        } else {
            self.auto_selection()
        }
    }
    fn move_pointer(&mut self, point: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if !self.active || !self.armed {
            return;
        }
        if let Some(point) = ImagePoint::from_logical(
            point.x.as_f32() as f64,
            point.y.as_f32() as f64,
            window.scale_factor() as f64,
        ) {
            if self.point != point {
                self.copied = false;
            }
            self.point = point;
            self.pointer_inside = self.contains_pointer(point);
        }
        if self.pointer_inside && !window.is_window_active() {
            (self.callback)(
                MaskAction::Activate {
                    session: self.session,
                    monitor: self.capture.monitor.id,
                },
                window,
                cx,
            );
            self.focus.focus(window, cx);
        }
        cx.notify();
    }
    fn finish(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if !self.active || !self.armed || self.start.is_none() {
            return;
        }
        self.move_pointer(position, window, cx);
        let start = self.start.take().unwrap();
        let minimum = 5. * window.scale_factor() as f64;
        let rect = if (start.x - self.point.x).abs() > minimum
            && (start.y - self.point.y).abs() > minimum
        {
            ImageRect::from_drag(start, self.point, self.dimensions())
        } else {
            self.click_selection
        };
        if let Some(rect) = rect {
            let callback = self.callback.clone();
            callback(
                MaskAction::Choose {
                    session: self.session,
                    monitor: self.capture.monitor.id,
                    rect,
                },
                window,
                cx,
            );
        }
        cx.notify();
    }
    fn pixel(&self, dx: i32, dy: i32) -> [u8; 4] {
        let image = &self.capture.image;
        let x = (self.point.x.floor() as i64 + dx as i64).clamp(0, image.width as i64 - 1) as u32;
        let y = (self.point.y.floor() as i64 + dy as i64).clamp(0, image.height as i64 - 1) as u32;
        image.pixel(x, y)
    }
    fn color(&self) -> String {
        let [r, g, b, _] = self.pixel(0, 0);
        format!("#{r:02x}{g:02x}{b:02x}")
    }
}

fn choose_rectangle(
    point: ImagePoint,
    rectangles: impl Iterator<Item = (i32, ImageRect)>,
) -> Option<ImageRect> {
    rectangles
        .filter(|(_, rect)| rect.contains(point))
        .fold(None, |best: Option<(i32, ImageRect)>, current| {
            let Some(previous) = best else {
                return Some(current);
            };
            if current.0 >= 0 && current.0 != previous.0 {
                Some(if previous.0 > current.0 {
                    previous
                } else {
                    current
                })
            } else {
                let previous_area = previous.1.width as u64 * previous.1.height as u64;
                let current_area = current.1.width as u64 * current.1.height as u64;
                Some(if previous_area < current_area {
                    previous
                } else {
                    current
                })
            }
        })
        .map(|(_, rect)| rect)
}
fn shade(x: f32, y: f32, width: f32, height: f32) -> Div {
    div()
        .absolute()
        .left(px(x))
        .top(px(y))
        .w(px(width.max(0.)))
        .h(px(height.max(0.)))
        .bg(rgba(0x00000088))
}

const INSPECTOR_WIDTH: f32 = 145. * 0.7;
const INSPECTOR_HEIGHT: f32 = 217. * 0.7;
const INSPECTOR_PREVIEW_SIZE: f32 = INSPECTOR_WIDTH - 2.;
const INSPECTOR_PIXEL_SIZE: f32 = INSPECTOR_PREVIEW_SIZE / 11.;
const INSPECTOR_INFO_HEIGHT: f32 = INSPECTOR_HEIGHT - INSPECTOR_PREVIEW_SIZE - 2.;

impl Render for MaskView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.active {
            return div().size_full().bg(rgb(0x000000)).into_any_element();
        }
        let scale = window.scale_factor();
        let width = self.capture.image.width as f32 / scale;
        let height = self.capture.image.height as f32 / scale;
        let selected = self.pointer_inside.then(|| self.selected()).flatten();
        let mut root = div()
            .id("capture-mask")
            .track_focus(&self.focus)
            .size_full()
            .overflow_hidden()
            .cursor(CursorStyle::Crosshair)
            .child(
                img(self.capture.image.render.clone())
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(px(width))
                    .h(px(height)),
            );
        if let Some(rect) = selected {
            let (x, y, w, h) = (
                rect.x as f32 / scale,
                rect.y as f32 / scale,
                rect.width as f32 / scale,
                rect.height as f32 / scale,
            );
            root = root
                .child(shade(0., 0., width, y))
                .child(shade(0., y + h, width, height - y - h))
                .child(shade(0., y, x, h))
                .child(shade(x + w, y, width - x - w, h))
                .child(
                    div()
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .w(px(w))
                        .h(px(h))
                        .border_1()
                        .border_color(cx.theme().primary),
                );
        } else {
            root = root.child(shade(0., 0., width, height));
        }
        let magnifier_x = inspector_axis(self.point.x as f32 / scale, INSPECTOR_WIDTH, width, 20.);
        let magnifier_y =
            inspector_axis(self.point.y as f32 / scale, INSPECTOR_HEIGHT, height, 24.);
        let [r, g, b, _] = self.pixel(0, 0);
        let sampled_color = rgb(((r as u32) << 16) | ((g as u32) << 8) | b as u32);
        let dimensions = selected
            .map(|rect| (rect.width, rect.height))
            .unwrap_or((self.capture.image.width, self.capture.image.height));
        let preview = div()
            .relative()
            .flex_none()
            .size(px(INSPECTOR_PREVIEW_SIZE))
            .overflow_hidden()
            .children(
                (-5..=5)
                    .flat_map(|dy| (-5..=5).map(move |dx| (dx, dy)))
                    .map(|(dx, dy)| {
                        let [r, g, b, _] = self.pixel(dx, dy);
                        div()
                            .absolute()
                            .left(px((dx + 5) as f32 * INSPECTOR_PIXEL_SIZE))
                            .top(px((dy + 5) as f32 * INSPECTOR_PIXEL_SIZE))
                            .size(px(INSPECTOR_PIXEL_SIZE))
                            .bg(rgb(((r as u32) << 16) | ((g as u32) << 8) | b as u32))
                    }),
            )
            // Both guides intersect at the center of the sampled source pixel.
            .child(
                div()
                    .absolute()
                    .left(px(INSPECTOR_PREVIEW_SIZE / 2.))
                    .top_0()
                    .w(px(1.))
                    .h_full()
                    .bg(rgb(0x50b7e8)),
            )
            .child(
                div()
                    .absolute()
                    .top(px(INSPECTOR_PREVIEW_SIZE / 2.))
                    .left_0()
                    .h(px(1.))
                    .w_full()
                    .bg(rgb(0x50b7e8)),
            );
        let magnifier = div()
            .absolute()
            .left(px(magnifier_x))
            .top(px(magnifier_y))
            .w(px(INSPECTOR_WIDTH))
            .h(px(INSPECTOR_HEIGHT))
            .flex()
            .flex_col()
            .items_center()
            .overflow_hidden()
            .border_1()
            .border_color(rgb(0x454849))
            .bg(rgb(0x080a0b))
            .text_color(rgb(0xeeeeee))
            .text_size(px(10.5))
            .line_height(px(14.))
            .child(preview)
            .child(
                div()
                    .w_full()
                    .h(px(INSPECTOR_INFO_HEIGHT))
                    .flex_none()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .border_t_1()
                    .border_color(rgb(0x454849))
                    .child(format!("{} × {}", dimensions.0, dimensions.1))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.2))
                            .child(div().flex_none().size(px(9.1)).bg(sampled_color))
                            .child(self.color()),
                    )
                    .child(if self.copied {
                        if self.chinese {
                            "颜色已复制"
                        } else {
                            "Color copied"
                        }
                    } else if self.chinese {
                        "复制颜色(C)"
                    } else {
                        "Copy color (C)"
                    }),
            );
        root.when(self.pointer_inside, |root| root.child(magnifier))
            // Native window hover changes also cover leaving without a final
            // mouse move (Windows reports this separately from MouseExitEvent).
            .on_hover(cx.listener(|this, hovered: &bool, window, cx| {
                // Hidden prepainting must preserve the primed cursor/selection;
                // it has no meaningful native hover state until shown and armed.
                if !this.active || !this.armed {
                    return;
                }
                if *hovered {
                    this.move_pointer(window.mouse_position(), window, cx);
                } else if this.pointer_inside {
                    this.pointer_inside = false;
                    this.copied = false;
                    cx.notify();
                }
            }))
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                this.move_pointer(event.position, window, cx)
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
                    if !this.active || !this.armed {
                        return;
                    }
                    this.move_pointer(event.position, window, cx);
                    this.click_selection = this.auto_selection();
                    this.start = Some(this.point);
                    cx.notify();
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                    this.finish(event.position, window, cx)
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, event: &MouseUpEvent, window, cx| {
                    this.finish(event.position, window, cx)
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if !this.active || !this.armed {
                    return;
                }
                if event.keystroke.key == "escape" {
                    let callback = this.callback.clone();
                    callback(
                        MaskAction::Cancel {
                            session: this.session,
                        },
                        window,
                        cx,
                    );
                    cx.stop_propagation();
                } else if this.pointer_inside && event.keystroke.key.eq_ignore_ascii_case("c") {
                    cx.write_to_clipboard(ClipboardItem::new_string(this.color()));
                    this.copied = true;
                    cx.notify();
                    cx.stop_propagation();
                }
            }))
            .into_any_element()
    }
}

// Prefer the opposite side at a display edge instead of covering the sampled pixel.
fn inspector_axis(pointer: f32, length: f32, viewport: f32, gap: f32) -> f32 {
    let position = if pointer + gap + length <= viewport {
        pointer + gap
    } else {
        pointer - gap - length
    };
    position.clamp(0., (viewport - length).max(0.))
}

#[cfg(test)]
mod tests {
    use super::{choose_rectangle, prepare_capture, prepare_image};
    use image::RgbaImage;
    use rotor_canvas::ImageRect;
    use rotor_runtime::{CaptureBundle, MonitorConfig};
    use std::sync::Arc;
    #[test]
    fn bgra_capture_moves_into_render_storage_and_converts_only_on_demand() {
        let bytes = vec![50, 100, 200, 255, 3, 2, 1, 128];
        let allocation = bytes.as_ptr();
        let prepared = super::PreparedScreenshot::new(rotor_runtime::BgraCapture {
            width: 2,
            height: 1,
            bytes,
        })
        .unwrap();
        assert_eq!(prepared.render.as_bytes(0).unwrap().as_ptr(), allocation);
        assert_eq!(prepared.pixel(0, 0), [200, 100, 50, 255]);
        assert_eq!(prepared.pixel(1, 0), [1, 2, 3, 128]);
        let rgba = prepared
            .crop_rgba(ImageRect {
                x: 1,
                y: 0,
                width: 1,
                height: 1,
            })
            .unwrap();
        assert_eq!(rgba.as_raw(), &[1, 2, 3, 128]);
        assert_eq!(
            prepared.render.as_bytes(0).unwrap(),
            &[50, 100, 200, 255, 3, 2, 1, 128]
        );
    }

    #[test]
    fn malformed_capture_buffers_are_rejected() {
        for (width, height, bytes) in [(0, 1, vec![]), (2, 1, vec![0; 4]), (1, 1, vec![0; 8])] {
            assert!(
                super::PreparedScreenshot::new(rotor_runtime::BgraCapture {
                    width,
                    height,
                    bytes,
                })
                .is_err()
            );
        }
    }

    #[test]
    fn capture_crop_preserves_rows_and_rejects_invalid_bounds() {
        let prepared = super::PreparedScreenshot::new(rotor_runtime::BgraCapture {
            width: 2,
            height: 2,
            bytes: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        })
        .unwrap();
        let rect = ImageRect {
            x: 1,
            y: 0,
            width: 1,
            height: 2,
        };
        assert_eq!(
            prepared.crop_rgba(rect).unwrap().as_raw(),
            &[7, 6, 5, 8, 15, 14, 13, 16]
        );
        for invalid in [
            ImageRect { width: 0, ..rect },
            ImageRect { x: 2, ..rect },
            ImageRect { y: 1, ..rect },
            ImageRect {
                x: u32::MAX,
                ..rect
            },
        ] {
            assert!(prepared.crop_rgba(invalid).is_err());
        }
    }

    #[test]
    fn inspector_stays_on_screen_and_away_from_edge_pixels() {
        for (length, gap) in [
            (super::INSPECTOR_WIDTH, 20.),
            (super::INSPECTOR_HEIGHT, 24.),
        ] {
            for viewport in [600., 1080., 1440.] {
                for pointer in [0., 1., viewport / 2., viewport - 1., viewport] {
                    let start = super::inspector_axis(pointer, length, viewport, gap);
                    assert!(start >= 0. && start + length <= viewport);
                    assert!(pointer < start || pointer > start + length);
                }
            }
            assert_eq!(super::inspector_axis(40., length, 80., gap), 0.);
        }
    }
    #[test]
    fn native_upload_is_straight_bgra_without_mutating_export_pixels() {
        let source = Arc::new(RgbaImage::from_pixel(
            2,
            1,
            image::Rgba([200, 100, 50, 128]),
        ));
        let prepared = prepare_image(source.clone()).unwrap();
        assert_eq!(
            prepared.render.as_bytes(0).unwrap(),
            &[50, 100, 200, 128, 50, 100, 200, 128]
        );
        assert_eq!(source.get_pixel(0, 0).0, [200, 100, 50, 128]);
    }
    #[test]
    fn auto_window_bounds_are_clipped_in_negative_desktop_coordinates() {
        let prepared = prepare_capture(CaptureBundle {
            monitors: vec![rotor_runtime::CapturedMonitor {
                monitor: MonitorConfig {
                    id: 1,
                    x: -100,
                    y: 0,
                    width: 4,
                    height: 4,
                    scale_factor: 2.,
                },
                image: rotor_runtime::BgraCapture {
                    width: 4,
                    height: 4,
                    bytes: vec![0; 4 * 4 * 4],
                },
            }],
            windows: vec![(-99, 1, 2, 4, 4)],
        })
        .unwrap();
        assert_eq!(
            prepared[0].windows,
            vec![(
                2,
                ImageRect {
                    x: if cfg!(target_os = "macos") { 2 } else { 1 },
                    y: if cfg!(target_os = "macos") { 2 } else { 1 },
                    width: if cfg!(target_os = "macos") { 2 } else { 3 },
                    height: if cfg!(target_os = "macos") { 2 } else { 3 }
                }
            )]
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn window_crossing_mixed_scale_displays_uses_each_capture_scale() {
        let prepared = prepare_capture(CaptureBundle {
            monitors: [(-4, 1., 4), (0, 2., 8)]
                .into_iter()
                .enumerate()
                .map(
                    |(id, (x, scale_factor, width))| rotor_runtime::CapturedMonitor {
                        monitor: MonitorConfig {
                            id: id as u32,
                            x,
                            y: -2,
                            width,
                            height: width,
                            scale_factor,
                        },
                        image: rotor_runtime::BgraCapture {
                            width,
                            height: width,
                            bytes: vec![0; (width * width * 4) as usize],
                        },
                    },
                )
                .collect(),
            windows: vec![(-2, -1, 7, 4, 2)],
        })
        .unwrap();
        assert_eq!(
            prepared[0].windows,
            vec![(
                7,
                ImageRect {
                    x: 2,
                    y: 1,
                    width: 2,
                    height: 2
                }
            )]
        );
        assert_eq!(
            prepared[1].windows,
            vec![(
                7,
                ImageRect {
                    x: 0,
                    y: 2,
                    width: 4,
                    height: 4
                }
            )]
        );
    }

    #[test]
    fn image_contours_can_refine_the_frontmost_window() {
        let background = ImageRect {
            x: 0,
            y: 0,
            width: 100,
            height: 100,
        };
        let foreground = ImageRect {
            x: 10,
            y: 10,
            width: 80,
            height: 80,
        };
        let contour = ImageRect {
            x: 20,
            y: 20,
            width: 20,
            height: 20,
        };
        let point = rotor_canvas::ImagePoint { x: 25., y: 25. };
        assert_eq!(
            choose_rectangle(point, [(1, background), (2, foreground)].into_iter()),
            Some(foreground)
        );
        assert_eq!(
            choose_rectangle(
                point,
                [(1, background), (2, foreground), (-1, contour)].into_iter()
            ),
            Some(contour)
        );
    }
}
