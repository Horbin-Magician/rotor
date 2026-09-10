use gpui_kit::{
    assets::IconName,
    component::{
        Icon, Sizable,
        button::{Button, ButtonCustomVariant, ButtonVariants},
    },
    prelude::*,
    *,
};

pub(super) enum Glyph {
    Back,
    Rectangle,
    Arrow,
    Text,
    Undo,
    Pen,
    Ocr,
    Minimize,
    Save,
    Close,
    Copy,
}

pub(super) fn separator() -> Div {
    div().flex_none().w(px(1.)).h(px(21.)).bg(rgba(0x8192a34d))
}

pub(super) fn button(id: &'static str, glyph: Glyph, cx: &App) -> Button {
    Button::new(id)
        .with_size(px(25.))
        .w(px(27.))
        .h(px(25.))
        .rounded(px(4.))
        .custom(
            ButtonCustomVariant::new(cx)
                .color(rgba(0x00000000).into())
                .foreground(rgb(0x4ba3e3).into())
                .hover(rgba(0x4ba3e324).into())
                .active(rgba(0xffffff30).into())
                .shadow(false),
        )
        .child(
            Icon::new(match glyph {
                Glyph::Back => IconName::ChevronLeft,
                Glyph::Rectangle => IconName::Square,
                Glyph::Arrow => IconName::ArrowDownLeft,
                Glyph::Text => IconName::Type,
                Glyph::Undo => IconName::Undo2,
                Glyph::Pen => IconName::Pencil,
                Glyph::Ocr => IconName::ScanText,
                Glyph::Minimize => IconName::WindowMinimize,
                Glyph::Save => IconName::Download,
                Glyph::Close => IconName::Close,
                Glyph::Copy => IconName::Copy,
            })
            .size(px(17.)),
        )
}

/// Slide the entire measured panel, including wrapped tools and status messages.
/// Offsetting during prepaint keeps painting and pointer hitboxes in sync.
pub(super) struct Slide {
    content: AnyElement,
    pub progress: f32,
}

impl Slide {
    pub fn new(content: impl IntoElement) -> Self {
        Self {
            content: content.into_any_element(),
            progress: 0.,
        }
    }
}

impl IntoElement for Slide {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Slide {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (self.content.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.progress > 0. {
            let offset = bounds.size.height * (1. - self.progress);
            window.with_element_offset(point(px(0.), offset), |window| {
                self.content.prepaint(window, cx);
            });
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.progress > 0. {
            self.content.paint(window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Slide;
    use gpui_kit::prelude::*;
    use gpui_kit::{
        Bounds, Context, IntoElement, Pixels, Render, TestAppContext, Window, canvas, div, px,
    };
    use std::{cell::Cell, rc::Rc};

    struct Panel {
        progress: f32,
        height: Pixels,
        painted: Rc<Cell<Option<Bounds<Pixels>>>>,
    }

    impl Render for Panel {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let painted = self.painted.clone();
            let mut slide = Slide::new(
                div()
                    .absolute()
                    .bottom(px(0.))
                    .w(px(200.))
                    .h(self.height)
                    .child(
                        canvas(
                            move |bounds, _, _| painted.set(Some(bounds)),
                            |_, _, _, _| {},
                        )
                        .size_full(),
                    ),
            );
            slide.progress = self.progress;
            div()
                .relative()
                .w(px(400.))
                .h(px(400.))
                .overflow_hidden()
                .child(slide)
        }
    }

    #[gpui::test]
    fn slide_moves_measured_content_and_does_not_paint_when_hidden(cx: &mut TestAppContext) {
        let painted = Rc::new(Cell::new(None));
        let (panel, cx) = cx.add_window_view(|_, _| Panel {
            progress: 1.,
            height: px(40.),
            painted: painted.clone(),
        });
        for height in [40., 120.] {
            for progress in [1., 0.5, 0., 0.5, 1.] {
                painted.set(None);
                panel.update(cx, |panel, cx| {
                    panel.height = px(height);
                    panel.progress = progress;
                    cx.notify();
                });
                cx.update(|window, cx| window.draw(cx).clear(cx));
                if progress == 0. {
                    assert!(painted.get().is_none());
                } else {
                    let bounds = painted.get().expect("visible toolbar should be prepainted");
                    assert_eq!(bounds.size.height, px(height));
                    assert_eq!(
                        bounds.origin.y,
                        px(400. - height + height * (1. - progress))
                    );
                }
            }
        }
    }
}
