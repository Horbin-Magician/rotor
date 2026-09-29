use gpui_kit::{prelude::*, *};
use rotor_common::Locale;
use rotor_runtime::LongCaptureStatus;
use std::{rc::Rc, sync::Arc};

pub const LONG_CAPTURE_PANEL_WIDTH: f32 = 280.;
pub const LONG_CAPTURE_PANEL_HEIGHT: f32 = 420.;

// Matches the floating pin toolbar so capture controls read as one family.
const BACKGROUND: u32 = 0x141c25;
const WELL: u32 = 0x0b1118;
const LINE: u32 = 0x2a3644;
const TEXT: u32 = 0xe4edf5;
const MUTED: u32 = 0x93a4b5;
const ACCENT: u32 = 0x4ba3e3;
const SUCCESS: u32 = 0x6fd6a3;
const WARNING: u32 = 0xffc36b;
const LOCATOR: (f32, f32) = (44., 26.);

#[derive(Clone, Copy)]
pub enum LongCaptureAction {
    Finish,
    KeepAccepted,
    Resume,
    Cancel,
}
pub type LongCaptureCallback = Rc<dyn Fn(LongCaptureAction, &mut Window, &mut App)>;

pub struct LongCaptureView {
    locale: Locale,
    callback: LongCaptureCallback,
    focus: FocusHandle,
    busy: bool,
    review: bool,
    resumed: bool,
    status: LongCaptureStatus,
    monitor: u32,
    locator: Option<(f32, f32, f32, f32)>,
    dimensions: (u32, u32),
    frames: usize,
    current: Option<Arc<RenderImage>>,
    tail: Option<Arc<RenderImage>>,
    overview: Option<Arc<RenderImage>>,
    show_current: bool,
}
impl LongCaptureView {
    pub fn new(locale: Locale, callback: LongCaptureCallback, cx: &mut Context<Self>) -> Self {
        Self {
            locale,
            callback,
            focus: cx.focus_handle(),
            busy: false,
            review: false,
            resumed: false,
            status: LongCaptureStatus::Capturing,
            monitor: 0,
            locator: None,
            dimensions: (0, 0),
            frames: 1,
            current: None,
            tail: None,
            overview: None,
            show_current: false,
        }
    }
    pub fn set_source(&mut self, monitor: u32, screen: (u32, u32), rect: rotor_canvas::ImageRect) {
        self.monitor = monitor;
        self.dimensions = (rect.width, rect.height);
        let (w, h) = (screen.0.max(1) as f32, screen.1.max(1) as f32);
        self.locator = Some((
            rect.x as f32 / w * LOCATOR.0,
            rect.y as f32 / h * LOCATOR.1,
            (rect.width as f32 / w * LOCATOR.0).max(2.),
            (rect.height as f32 / h * LOCATOR.1).max(2.),
        ));
    }
    pub fn preview(
        &mut self,
        current: Arc<RenderImage>,
        tail: Arc<RenderImage>,
        overview: Arc<RenderImage>,
        cx: &mut Context<Self>,
    ) {
        self.current = Some(current);
        self.tail = Some(tail);
        self.overview = Some(overview);
        cx.notify();
    }
    pub fn progress(
        &mut self,
        frames: usize,
        height: u32,
        status: LongCaptureStatus,
        review: bool,
        cx: &mut Context<Self>,
    ) {
        // Queued capture progress must not re-enable Finish while its request
        // is still being processed. Review is the worker's acknowledgement.
        if review {
            self.busy = false;
        }
        if frames != self.frames {
            self.resumed = false;
        }
        self.review = review;
        self.status = status;
        self.frames = frames;
        self.dimensions.1 = height;
        cx.notify();
    }
    /// Finish from the panel, Enter, or the capture shortcut pressed again.
    pub fn finish(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        (self.callback)(
            if self.review {
                LongCaptureAction::KeepAccepted
            } else {
                LongCaptureAction::Finish
            },
            window,
            cx,
        );
        cx.notify();
    }
    fn resume(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || !self.review {
            return;
        }
        self.review = false;
        self.resumed = true;
        self.status = LongCaptureStatus::Capturing;
        (self.callback)(LongCaptureAction::Resume, window, cx);
        cx.notify();
    }
    fn warning(&self) -> bool {
        self.review || self.status.is_issue()
    }
    fn badge(&self) -> (&'static str, u32) {
        let pick = |zh, en| self.locale.pick(zh, en);
        if self.busy {
            (pick("正在完成", "Finishing"), ACCENT)
        } else if self.review {
            (pick("已暂停", "Paused"), WARNING)
        } else if self.status.is_issue() {
            (pick("需要调整", "Needs attention"), WARNING)
        } else if self.status == LongCaptureStatus::ScrolledBack {
            (pick("已回滚", "Scrolled back"), ACCENT)
        } else if self.frames <= 1 {
            (pick("等待滚动", "Waiting"), ACCENT)
        } else {
            (pick("正在采集", "Capturing"), SUCCESS)
        }
    }
    fn hint(&self) -> &'static str {
        let pick = |zh, en| self.locale.pick(zh, en);
        if self.busy {
            return pick("正在生成长图…", "Creating the long image…");
        }
        if self.review {
            return pick(
                "最后一段未能拼接。可保留已拼接部分，或继续采集：先向上滚回一些，再慢慢向下补齐尾部。",
                "The final section could not be joined. Keep the captured part, or resume: scroll back a little, then slowly down to capture the tail.",
            );
        }
        match self.status {
            LongCaptureStatus::Capturing if self.resumed => pick(
                "请向上滚回一些，再缓慢向下滚动补齐尾部。",
                "Scroll back into the captured part, then slowly down to capture the tail.",
            ),
            LongCaptureStatus::Capturing if self.frames <= 1 => pick(
                "在选区内向下滚动页面，Rotor 会自动识别并拼接；固定标题栏和底部输入框只保留一份。",
                "Scroll down inside the selection. Rotor joins frames automatically and keeps fixed headers and footers once.",
            ),
            LongCaptureStatus::Capturing => pick(
                "继续向下滚动。到底后点击完成，或再次按截图快捷键。",
                "Keep scrolling down. At the end, click Finish or press the capture shortcut again.",
            ),
            LongCaptureStatus::ScrolledBack => pick(
                "已向上滚动，已拼接内容保持不变。继续向下滚动即可接上。",
                "Scrolled up; the captured image is unchanged. Scroll down again to continue.",
            ),
            LongCaptureStatus::NoOverlap => pick(
                "与上一帧没有足够重叠：可能滚动过快或内容在变化。请向上滚回一些，再慢慢向下。",
                "Not enough overlap with the previous frame: scrolling was too fast or content changed. Scroll back a little, then down slowly.",
            ),
            LongCaptureStatus::Ambiguous => pick(
                "重叠部分空白或重复内容太多，无法确定位置。向上滚回一些，让选区包含更多不同的文字。",
                "The overlap is too blank or repetitive to place. Scroll back so the selection includes more distinctive text.",
            ),
            LongCaptureStatus::Limit => pick(
                "已达到拼接计算上限。点击完成保留已拼接部分，或取消后缩小选区。",
                "Stitching reached its comparison limit. Click Finish to keep the captured part, or cancel and select a smaller region.",
            ),
            LongCaptureStatus::MemoryBudget => pick(
                "已达到长图像素缓冲的 256 MiB 内存预算，或内存分配失败。点击完成保留已拼接部分，或取消后缩小选区。",
                "The long image reached its 256 MiB pixel buffer budget, or allocation failed. Click Finish to keep the captured part, or cancel and select a smaller region.",
            ),
            LongCaptureStatus::ViewportChanged => pick(
                "窗口尺寸或显示缩放已变化，无法继续拼接。点击完成保留已拼接部分，或取消后重新截图。",
                "The window size or display scale changed. Click Finish to keep the captured part, or cancel and capture again.",
            ),
        }
    }
}

fn thousands(value: u32) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

fn key_hint(key: &'static str, color: u32) -> Div {
    div()
        .px(px(4.))
        .rounded(px(3.))
        .border_1()
        .border_color(rgba(color << 8 | 0x66))
        .text_size(px(10.))
        .text_color(rgba(color << 8 | 0xcc))
        .child(key)
}

fn button(id: &'static str, primary: bool) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(30.))
        .px(px(10.))
        .rounded(px(6.))
        .cursor_pointer()
        .text_size(px(12.))
        .when(primary, |button| {
            button
                .bg(rgb(0x2f7fc1))
                .text_color(rgb(0xffffff))
                .hover(|style| style.bg(rgb(0x3a8fd4)))
        })
        .when(!primary, |button| {
            button
                .border_1()
                .border_color(rgb(LINE))
                .text_color(rgb(TEXT))
                .hover(|style| style.bg(rgb(0x1e2935)))
        })
}

impl Render for LongCaptureView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale;
        let pick = move |zh, en| locale.pick(zh, en);
        let (badge, badge_color) = self.badge();
        let hint_color = if self.warning() {
            WARNING
        } else if self.status == LongCaptureStatus::ScrolledBack {
            ACCENT
        } else {
            0xc3d0dc
        };
        let tabs = div()
            .flex()
            .flex_none()
            .p(px(2.))
            .gap(px(2.))
            .rounded(px(6.))
            .bg(rgb(WELL))
            .children([false, true].map(|current| {
                let selected = self.show_current == current;
                div()
                    .id(if current {
                        "preview-current"
                    } else {
                        "preview-stitched"
                    })
                    .debug_selector(move || {
                        if current {
                            "preview-current".into()
                        } else {
                            "preview-stitched".into()
                        }
                    })
                    .flex_1()
                    .py(px(3.))
                    .rounded(px(4.))
                    .text_center()
                    .text_size(px(12.))
                    .cursor_pointer()
                    .when(selected, |tab| tab.bg(rgb(0x243242)).text_color(rgb(TEXT)))
                    .when(!selected, |tab| {
                        tab.text_color(rgb(MUTED))
                            .hover(|style| style.text_color(rgb(TEXT)))
                    })
                    .child(if current {
                        pick("当前选区", "Current region")
                    } else {
                        pick("长图预览", "Long image")
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.show_current = current;
                        cx.notify();
                    }))
            }));
        let card = preview_card(
            if self.show_current {
                self.current.clone()
            } else {
                self.tail.clone()
            },
            (!self.show_current)
                .then(|| self.overview.clone())
                .flatten(),
            if self.review { 150. } else { 196. },
            pick("等待画面…", "Waiting for frames…"),
        );
        let stats = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10.))
            .children(self.locator.map(|(x, y, width, height)| {
                div()
                    .relative()
                    .flex_none()
                    .w(px(LOCATOR.0))
                    .h(px(LOCATOR.1))
                    .rounded(px(3.))
                    .bg(rgb(WELL))
                    .border_1()
                    .border_color(rgb(LINE))
                    .child(
                        div()
                            .absolute()
                            .left(px(x))
                            .top(px(y))
                            .w(px(width))
                            .h(px(height))
                            .rounded(px(1.))
                            .border_1()
                            .border_color(rgb(ACCENT))
                            .bg(rgba(ACCENT << 8 | 0x40)),
                    )
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(format!(
                                "{} × {} px",
                                thousands(self.dimensions.0),
                                thousands(self.dimensions.1)
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child(format!(
                                "{} {} · {} {}",
                                self.frames,
                                pick("帧", "frames"),
                                pick("屏幕", "Display"),
                                self.monitor
                            )),
                    ),
            );
        let cancel = button("cancel-long-capture", false)
            .child(pick("取消", "Cancel"))
            .when(!self.review, |button| button.child(key_hint("Esc", MUTED)))
            .on_click(cx.listener(|this, _, window, cx| {
                (this.callback)(LongCaptureAction::Cancel, window, cx)
            }));
        let finish = button("finish-long-capture", true)
            .when(self.busy, |button| button.opacity(0.5).cursor_default())
            .child(if self.review {
                pick("保留已拼接部分", "Keep captured part")
            } else {
                pick("完成", "Finish")
            })
            .child(key_hint("↵", 0xffffff))
            .on_click(cx.listener(|this, _, window, cx| this.finish(window, cx)));
        // Review offers three choices; stack them so long labels never clip.
        let actions = if self.review {
            div()
                .flex()
                .flex_col()
                .flex_none()
                .gap(px(6.))
                .child(finish.justify_center())
                .child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(
                            button("resume-long-capture", false)
                                .flex_1()
                                .justify_center()
                                .child(pick("继续采集", "Resume"))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.resume(window, cx)),
                                ),
                        )
                        .child(cancel.flex_1().justify_center()),
                )
        } else {
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_between()
                .child(cancel)
                .child(finish)
        };
        div()
            .id("long-capture-control")
            .track_focus(&self.focus)
            .size_full()
            .bg(rgb(BACKGROUND))
            .border_1()
            .border_color(rgb(LINE))
            .text_color(rgb(TEXT))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .text_size(px(13.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.focus.focus(window, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(pick("长截图", "Long capture")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .px(px(8.))
                            .py(px(2.))
                            .rounded_full()
                            .bg(rgba(badge_color << 8 | 0x22))
                            .text_size(px(11.))
                            .text_color(rgb(badge_color))
                            .child(div().size(px(6.)).rounded_full().bg(rgb(badge_color)))
                            .child(badge),
                    ),
            )
            .child(card)
            .child(tabs)
            .child(stats)
            .child(
                div()
                    .id("long-capture-status")
                    .debug_selector(|| "long-capture-status".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .text_size(px(12.))
                    .line_height(px(18.))
                    .text_color(rgb(hint_color))
                    .child(self.hint()),
            )
            .child(actions)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.is_held {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "escape" => (this.callback)(LongCaptureAction::Cancel, window, cx),
                    "enter" => this.finish(window, cx),
                    "tab" => {
                        this.show_current = !this.show_current;
                        cx.notify();
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
    }
}

// The long image view shows its newest rows at readable width next to a narrow
// overview of the whole result, so progress stays visible on tall images.
fn preview_card(
    main: Option<Arc<RenderImage>>,
    overview: Option<Arc<RenderImage>>,
    height: f32,
    empty: &'static str,
) -> impl IntoElement {
    let well = || {
        div()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .rounded(px(6.))
            .bg(rgb(WELL))
            .border_1()
            .border_color(rgb(LINE))
    };
    div()
        .flex()
        .flex_none()
        .gap(px(6.))
        .h(px(height))
        .children(overview.map(|image| {
            well()
                .flex_none()
                .w(px(34.))
                .p(px(2.))
                .child(img(image).w_full().h_full().object_fit(ObjectFit::Contain))
        }))
        .child(
            well()
                .flex_1()
                .min_w_0()
                .when(main.is_none(), |well| {
                    well.text_size(px(12.)).text_color(rgb(MUTED)).child(empty)
                })
                .children(
                    main.map(|image| img(image).w_full().h_full().object_fit(ObjectFit::Contain)),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::{Locale, LongCaptureAction, LongCaptureStatus, LongCaptureView, Rc};
    use gpui_kit::{px, size};
    use std::cell::RefCell;

    #[test]
    fn sizes_use_grouped_digits() {
        assert_eq!(super::thousands(7), "7");
        assert_eq!(super::thousands(600), "600");
        assert_eq!(super::thousands(5000), "5,000");
        assert_eq!(super::thousands(16384), "16,384");
        assert_eq!(super::thousands(1234567), "1,234,567");
    }

    #[gpui::test]
    fn rejected_tail_requires_a_separate_keep_action(cx: &mut gpui::TestAppContext) {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let recorded = actions.clone();
        let (view, cx) =
            cx.add_window_view(|window, cx| {
                let mut view = LongCaptureView::new(
                    Locale::English,
                    Rc::new(move |action, _, _| recorded.borrow_mut().push(action)),
                    cx,
                );
                view.set_source(
                    1,
                    (1920, 1080),
                    rotor_canvas::ImageRect {
                        x: 200,
                        y: 100,
                        width: 600,
                        height: 400,
                    },
                );
                let image = crate::prepare_image(std::sync::Arc::new(
                    image::RgbaImage::from_pixel(60, 40, image::Rgba([30, 80, 120, 255])),
                ))
                .unwrap();
                view.preview(image.render.clone(), image.render.clone(), image.render, cx);
                view.focus.focus(window, cx);
                view
            });
        cx.simulate_resize(size(
            px(super::LONG_CAPTURE_PANEL_WIDTH),
            px(super::LONG_CAPTURE_PANEL_HEIGHT),
        ));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let tab = cx.debug_bounds("preview-current").unwrap().center();
        cx.simulate_mouse_down(tab, gpui::MouseButton::Left, Default::default());
        cx.simulate_mouse_up(tab, gpui::MouseButton::Left, Default::default());
        view.read_with(cx, |view, _| assert!(view.show_current));
        cx.simulate_keystrokes("tab");
        view.read_with(cx, |view, _| assert!(!view.show_current));
        assert!(
            actions.borrow().is_empty(),
            "preview tabs must not finish capture"
        );
        view.update(cx, |view, cx| {
            view.progress(2, 700, LongCaptureStatus::ScrolledBack, false, cx)
        });
        view.read_with(cx, |view, _| {
            assert!(!view.warning(), "scrolling back is guidance, not an error");
            assert_eq!(view.badge().0, "Scrolled back");
        });
        cx.simulate_keystrokes("enter");
        assert!(matches!(
            actions.borrow().as_slice(),
            [LongCaptureAction::Finish]
        ));
        view.update(cx, |view, cx| {
            view.progress(3, 1200, LongCaptureStatus::Capturing, false, cx)
        });
        cx.simulate_keystrokes("enter");
        assert_eq!(
            actions.borrow().len(),
            1,
            "queued progress must not submit Finish twice"
        );
        view.update(cx, |view, cx| {
            view.progress(3, 1200, LongCaptureStatus::NoOverlap, true, cx)
        });
        view.read_with(cx, |view, _| assert!(view.warning()));
        assert_eq!(
            actions.borrow().len(),
            1,
            "review must not export automatically"
        );
        cx.update(|window, cx| window.draw(cx).clear(cx));
        for selector in [
            "finish-long-capture",
            "resume-long-capture",
            "cancel-long-capture",
        ] {
            let bounds = cx.debug_bounds(selector).unwrap();
            assert!(bounds.origin.x >= px(0.) && bounds.origin.y >= px(0.));
            assert!(bounds.right() <= px(super::LONG_CAPTURE_PANEL_WIDTH));
            assert!(bounds.bottom() <= px(super::LONG_CAPTURE_PANEL_HEIGHT));
        }
        assert!(cx.debug_bounds("long-capture-status").unwrap().size.height >= px(32.));
        cx.simulate_keystrokes("enter");
        assert!(matches!(
            actions.borrow().as_slice(),
            [LongCaptureAction::Finish, LongCaptureAction::KeepAccepted]
        ));
    }

    #[gpui::test]
    fn resume_returns_to_capturing_and_guides_back_into_overlap(cx: &mut gpui::TestAppContext) {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let recorded = actions.clone();
        let (view, cx) = cx.add_window_view(|_, cx| {
            LongCaptureView::new(
                Locale::English,
                Rc::new(move |action, _, _| recorded.borrow_mut().push(action)),
                cx,
            )
        });
        cx.simulate_resize(size(
            px(super::LONG_CAPTURE_PANEL_WIDTH),
            px(super::LONG_CAPTURE_PANEL_HEIGHT),
        ));
        view.update(cx, |view, cx| {
            view.progress(4, 900, LongCaptureStatus::NoOverlap, true, cx)
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let resume = cx.debug_bounds("resume-long-capture").unwrap().center();
        cx.simulate_mouse_down(resume, gpui::MouseButton::Left, Default::default());
        cx.simulate_mouse_up(resume, gpui::MouseButton::Left, Default::default());
        assert!(matches!(
            actions.borrow().as_slice(),
            [LongCaptureAction::Resume]
        ));
        view.read_with(cx, |view, _| {
            assert!(!view.review);
            assert!(view.hint().starts_with("Scroll back"));
        });
        // The resumed hint gives way once a new frame is joined.
        view.update(cx, |view, cx| {
            view.progress(5, 1000, LongCaptureStatus::Capturing, false, cx)
        });
        view.read_with(cx, |view, _| {
            assert!(view.hint().starts_with("Keep scrolling"))
        });
    }
}
