//! Settings-only surfaces; other tool windows keep their existing palette.
use super::*;
use gpui_kit::component::button::ButtonCustomVariant;

pub(super) struct Palette {
    pub background: Hsla,
    pub surface: Hsla,
    pub border: Hsla,
    pub foreground: Hsla,
    pub secondary: Hsla,
    pub accent: Hsla,
    pub hover: Hsla,
}

pub(super) fn palette(cx: &App) -> Palette {
    if cx.theme().is_dark() {
        Palette {
            background: rgb(0x111111).into(),
            surface: rgb(0x202020).into(),
            border: rgb(0x333333).into(),
            foreground: rgb(0xf1f1f1).into(),
            secondary: rgb(0xc2c2c2).into(),
            accent: rgb(0x29a8d8).into(),
            hover: rgb(0x282828).into(),
        }
    } else {
        Palette {
            background: rgb(0xffffff).into(),
            surface: rgb(0xf6f7f8).into(),
            border: rgb(0xe1e4e8).into(),
            foreground: rgb(0x202428).into(),
            secondary: rgb(0x59616a).into(),
            accent: rgb(0x087fa9).into(),
            hover: rgb(0xedf4f7).into(),
        }
    }
}

pub(super) fn card(cx: &App) -> Div {
    let colors = palette(cx);
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .gap_2()
        .p_3()
        .rounded_md()
        .border_1()
        .border_color(colors.border)
        .bg(colors.surface)
}

// Settings use a denser type scale without changing other tool windows.
pub(super) fn caption(value: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(13.))
        .text_color(palette(cx).secondary)
        .child(value.into())
}

pub(super) fn heading(title: &'static str, cx: &App) -> Div {
    div()
        .pb(px(4.))
        .border_b_1()
        .border_color(palette(cx).border)
        .text_size(px(14.))
        .font_weight(FontWeight::BOLD)
        .child(title)
}

pub(super) fn detail_row(label: impl Into<SharedString>, value: impl IntoElement, cx: &App) -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .justify_between()
        .gap_2()
        .min_h(px(32.))
        .pl(px(12.))
        .py(px(4.))
        .text_size(px(13.))
        .text_color(palette(cx).secondary)
        .child(div().flex_shrink_0().child(label.into()))
        .child(div().min_w_0().max_w_full().text_size(px(12.)).child(value))
}

pub(super) fn quiet_button(button: Button, cx: &App) -> Button {
    let colors = palette(cx);
    button.custom(
        ButtonCustomVariant::new(cx)
            .color(colors.background)
            .foreground(colors.secondary)
            .hover(colors.hover)
            .active(colors.surface),
    )
}

pub(super) fn navigation(button: Button, selected: bool, cx: &App) -> Button {
    let colors = palette(cx);
    button
        .font_weight(FontWeight::NORMAL)
        .toggled(selected)
        .custom(
            ButtonCustomVariant::new(cx)
                .color(colors.background)
                .foreground(if selected {
                    colors.accent
                } else {
                    colors.foreground
                })
                .hover(colors.hover)
                .active(colors.surface),
        )
}
