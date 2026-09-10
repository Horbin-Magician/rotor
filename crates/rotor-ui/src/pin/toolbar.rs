use gpui_kit::{
    component::{
        Sizable,
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
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let strokes: &[&[(f32, f32)]] = match glyph {
                        Glyph::Back => &[&[(16., 2.), (6., 12.), (16., 22.)]],
                        Glyph::Rectangle => {
                            &[&[(3., 3.), (21., 3.), (21., 21.), (3., 21.), (3., 3.)]]
                        }
                        Glyph::Arrow => {
                            &[&[(21., 3.), (3., 21.), (3., 10.)], &[(3., 21.), (14., 21.)]]
                        }
                        Glyph::Text => &[
                            &[(5., 6.), (5., 3.), (19., 3.), (19., 6.)],
                            &[(12., 3.), (12., 21.)],
                            &[(9., 21.), (15., 21.)],
                        ],
                        Glyph::Undo => &[
                            &[(3., 7.), (3., 15.), (11., 15.)],
                            &[
                                (3., 15.),
                                (7., 9.),
                                (12., 8.),
                                (17., 9.),
                                (21., 13.),
                                (22., 16.),
                            ],
                        ],
                        Glyph::Pen => &[
                            &[
                                (3., 21.),
                                (3., 18.),
                                (18., 3.),
                                (21., 6.),
                                (6., 21.),
                                (3., 21.),
                            ],
                            &[(15., 6.), (18., 9.)],
                        ],
                        Glyph::Ocr => &[
                            &[(2., 8.), (2., 2.), (8., 2.)],
                            &[(16., 2.), (22., 2.), (22., 8.)],
                            &[(22., 16.), (22., 22.), (16., 22.)],
                            &[(8., 22.), (2., 22.), (2., 16.)],
                            &[(7., 7.), (17., 7.)],
                            &[(5., 12.), (19., 12.)],
                            &[(7., 17.), (14., 17.)],
                        ],
                        Glyph::Minimize => &[&[(4., 12.), (20., 12.)]],
                        Glyph::Save => &[
                            &[(12., 2.), (12., 16.)],
                            &[(7., 11.), (12., 16.), (17., 11.)],
                            &[(3., 11.), (3., 21.), (21., 21.), (21., 11.)],
                        ],
                        Glyph::Close => &[&[(5., 5.), (19., 19.)], &[(19., 5.), (5., 19.)]],
                        Glyph::Copy => &[
                            &[(7., 2.), (21., 2.), (21., 19.), (7., 19.), (7., 2.)],
                            &[(3., 6.), (3., 23.), (17., 23.)],
                        ],
                    };
                    let mut path = PathBuilder::stroke(px(1.5));
                    for stroke in strokes {
                        for (index, &(x, y)) in stroke.iter().enumerate() {
                            let position = bounds.origin
                                + point(
                                    bounds.size.width * (x / 24.),
                                    bounds.size.height * (y / 24.),
                                );
                            if index == 0 {
                                path.move_to(position);
                            } else {
                                path.line_to(position);
                            }
                        }
                    }
                    if let Ok(path) = path.build() {
                        window.paint_path(path, window.text_style().color);
                    }
                },
            )
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
