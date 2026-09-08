//! Shared surfaces use semantic colors so every window follows the active theme.
use gpui_kit::{
    component::{
        ActiveTheme, Selectable, Theme, ThemeConfig,
        button::{Button, ButtonCustomVariant, ButtonVariants},
    },
    prelude::*,
    *,
};
use std::rc::Rc;

/// Customize both stored modes so switching or following the OS keeps Rotor's palette.
pub fn configure_theme(cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.light_theme = Rc::new(palette(theme.light_theme.as_ref().clone(), false));
    theme.dark_theme = Rc::new(palette(theme.dark_theme.as_ref().clone(), true));
}

fn palette(mut theme: ThemeConfig, dark: bool) -> ThemeConfig {
    theme.name = if dark { "Rotor Dark" } else { "Rotor Light" }.into();
    theme.font_size = Some(14.);
    let colors = &mut theme.colors;
    let primary = if dark { "#70aaff" } else { "#2463bf" };
    let background = if dark { "#1c2027" } else { "#ffffff" };
    let foreground = if dark { "#eef2f8" } else { "#162033" };
    let muted = if dark { "#14171d" } else { "#f5f7fa" };
    let border = if dark { "#303744" } else { "#e1e6ef" };
    let hover = if dark { "#26303f" } else { "#edf3fa" };
    let selected = if dark { "#243f61" } else { "#e3efff" };
    colors.background = Some(background.into());
    colors.foreground = Some(foreground.into());
    colors.muted = Some(muted.into());
    colors.muted_foreground = Some(if dark { "#a6b1c2" } else { "#647084" }.into());
    colors.border = Some(border.into());
    colors.input = Some(border.into());
    colors.popover = Some(background.into());
    colors.popover_foreground = Some(foreground.into());
    colors.primary = Some(primary.into());
    colors.primary_foreground = Some(if dark { "#101925" } else { "#ffffff" }.into());
    colors.primary_hover = Some(if dark { "#94c0ff" } else { "#1d55a7" }.into());
    colors.primary_active = Some(if dark { "#5694ed" } else { "#17478e" }.into());
    colors.button_primary = colors.primary.clone();
    colors.button_primary_foreground = colors.primary_foreground.clone();
    colors.button_primary_hover = colors.primary_hover.clone();
    colors.button_primary_active = colors.primary_active.clone();
    colors.button = Some(if dark { "#252b35" } else { "#ffffff" }.into());
    colors.button_foreground = Some(foreground.into());
    colors.button_hover = Some(hover.into());
    colors.button_active = Some(selected.into());
    colors.list = Some(background.into());
    colors.list_active = Some(selected.into());
    colors.list_active_border = Some(primary.into());
    colors.list_hover = Some(hover.into());
    colors.link = Some(primary.into());
    colors.ring = Some(primary.into());
    colors.accent = Some(hover.into());
    colors.accent_foreground = Some(foreground.into());
    theme
}

pub(crate) fn choice(button: Button, selected: bool, cx: &App) -> Button {
    button
        .selected(selected)
        .toggled(selected)
        .when(selected, |button| {
            button.custom(
                ButtonCustomVariant::new(cx)
                    .color(cx.theme().list_active)
                    .foreground(cx.theme().primary)
                    .hover(cx.theme().list_active)
                    .active(cx.theme().list_active),
            )
        })
}

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
        .text_size(px(12.))
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
                .text_size(px(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(caption(detail, cx))
}
