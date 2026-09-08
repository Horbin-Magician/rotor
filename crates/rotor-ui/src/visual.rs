//! Shared surfaces use semantic colors so every window follows the active theme.
use gpui_kit::{component::ActiveTheme, prelude::*, *};

pub(crate) fn card(cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .gap_3()
        .p_4()
        .rounded_xl()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
}

pub(crate) fn caption(value: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(value.into())
}

pub(crate) fn heading(title: &'static str, detail: &'static str, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(caption(detail, cx))
}
