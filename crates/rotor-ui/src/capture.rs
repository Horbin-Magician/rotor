use gpui_kit::{prelude::*, *};
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
    pub image: PreparedImage,
    windows: Vec<(i32, ImageRect)>,
}
pub fn prepare_capture(bundle: CaptureBundle) -> Result<Vec<Arc<PreparedCapture>>, String> {
    bundle
        .monitors
        .into_iter()
        .map(|capture| {
            let monitor = capture.monitor;
            if capture.image.dimensions() != (monitor.width, monitor.height) {
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
                    ImageRect::from_drag(
                        ImagePoint {
                            x: x as f64 - monitor.x as f64,
                            y: y as f64 - monitor.y as f64,
                        },
                        ImagePoint {
                            x: x as f64 + width as f64 - monitor.x as f64,
                            y: y as f64 + height as f64 - monitor.y as f64,
                        },
                        dimensions,
                    )
                    .map(|rect| (z, rect))
                })
                .collect();
            Ok(Arc::new(PreparedCapture {
                monitor,
                image: prepare_image(capture.image)?,
                windows,
            }))
        })
        .collect()
}
#[derive(Clone, Copy)]
pub enum MaskAction {
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
    capture: Arc<PreparedCapture>,
    callback: MaskCallback,
    point: ImagePoint,
    start: Option<ImagePoint>,
    click_selection: Option<ImageRect>,
    focus: FocusHandle,
    armed: bool,
    _bounds: Subscription,
    detected: Vec<ImageRect>,
}
impl MaskView {
    pub fn new(
        session: u64,
        capture: Arc<PreparedCapture>,
        callback: MaskCallback,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let bounds =
            cx.observe_window_bounds(window, |this, window, cx| this.check_geometry(window, cx));
        Self {
            session,
            capture,
            callback,
            point: ImagePoint { x: 0., y: 0. },
            start: None,
            click_selection: None,
            focus,
            armed: false,
            _bounds: bounds,
            detected: Vec::new(),
        }
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
        self.move_pointer(point, window, cx);
    }
    pub fn set_detected_rectangles(&mut self, rectangles: Vec<ImageRect>, cx: &mut Context<Self>) {
        let size = self.dimensions();
        self.detected = rectangles
            .into_iter()
            .filter_map(|rect| rect.clipped(size))
            .collect();
        cx.notify();
    }
    pub fn arm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            width: self.capture.image.image.width(),
            height: self.capture.image.image.height(),
        }
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
        if let Some(point) = ImagePoint::from_logical(
            point.x.as_f32() as f64,
            point.y.as_f32() as f64,
            window.scale_factor() as f64,
        ) {
            self.point = point;
        }
        if !window.is_window_active() {
            window.activate_window();
            self.focus.focus(window, cx);
        }
        cx.notify();
    }
    fn finish(&mut self, position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if self.start.is_none() {
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
        let image = &self.capture.image.image;
        let x = (self.point.x.floor() as i64 + dx as i64).clamp(0, image.width() as i64 - 1) as u32;
        let y =
            (self.point.y.floor() as i64 + dy as i64).clamp(0, image.height() as i64 - 1) as u32;
        image.get_pixel(x, y).0
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
impl Render for MaskView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scale = window.scale_factor();
        let width = self.capture.image.image.width() as f32 / scale;
        let height = self.capture.image.image.height() as f32 / scale;
        let selected = self.selected();
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
                        .border_color(rgba(0x3399ffff)),
                );
        } else {
            root = root.child(shade(0., 0., width, height));
        }
        let magnifier_x = (self.point.x as f32 / scale + 20.).clamp(0., (width - 112.).max(0.));
        let magnifier_y = (self.point.y as f32 / scale + 24.).clamp(0., (height - 140.).max(0.));
        let magnifier = div()
            .absolute()
            .left(px(magnifier_x))
            .top(px(magnifier_y))
            .p_2()
            .flex()
            .flex_col()
            .bg(rgba(0x111111ff))
            .text_color(rgba(0xffffffff))
            .text_xs()
            .children((-5..=5).map(|dy| {
                div().flex().children((-5..=5).map(|dx| {
                    let [r, g, b, a] = self.pixel(dx, dy);
                    let color =
                        ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | a as u32;
                    div()
                        .size(px(8.))
                        .bg(rgba(color))
                        .when(dx == 0 && dy == 0, |cell| {
                            cell.border_1().border_color(rgba(0xffffffff))
                        })
                }))
            }))
            .child(self.color())
            .child(
                selected
                    .map(|rect| format!("{} × {}", rect.width, rect.height))
                    .unwrap_or_default(),
            )
            .child("C · Copy / Esc");
        root.child(magnifier)
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                this.move_pointer(event.position, window, cx)
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, window, cx| {
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
                } else if event.keystroke.key.eq_ignore_ascii_case("c") {
                    cx.write_to_clipboard(ClipboardItem::new_string(this.color()));
                    cx.stop_propagation();
                }
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::{choose_rectangle, prepare_capture, prepare_image};
    use image::RgbaImage;
    use rotor_canvas::ImageRect;
    use rotor_runtime::{CaptureBundle, MonitorConfig};
    use std::sync::Arc;
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
                image: Arc::new(RgbaImage::new(4, 4)),
            }],
            windows: vec![(-99, 1, 2, 4, 4)],
        })
        .unwrap();
        assert_eq!(
            prepared[0].windows,
            vec![(
                2,
                ImageRect {
                    x: 1,
                    y: 1,
                    width: 3,
                    height: 3
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
