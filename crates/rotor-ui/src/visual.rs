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

/// Shared RGB values also used by the native tray menu without a GPUI context.
pub(crate) struct Palette {
    pub background: u32,
    pub surface: u32,
    pub border: u32,
    pub foreground: u32,
    pub secondary: u32,
    pub accent: u32,
    pub hover: u32,
}

pub(crate) fn surface_palette(dark: bool) -> Palette {
    if dark {
        Palette {
            background: 0x111111,
            surface: 0x202020,
            border: 0x333333,
            foreground: 0xf1f1f1,
            secondary: 0xc2c2c2,
            accent: 0x29a8d8,
            hover: 0x282828,
        }
    } else {
        Palette {
            background: 0xffffff,
            surface: 0xf6f7f8,
            border: 0xe1e4e8,
            foreground: 0x202428,
            secondary: 0x59616a,
            accent: 0x087fa9,
            hover: 0xedf4f7,
        }
    }
}

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
    let palette = surface_palette(dark);
    let hex = |value| format!("#{value:06x}");
    let primary = hex(palette.accent);
    let background = hex(palette.background);
    let foreground = hex(palette.foreground);
    let muted = hex(palette.surface);
    let border = hex(palette.border);
    let hover = hex(palette.hover);
    let selected = hover.clone();
    colors.background = Some(background.clone().into());
    colors.foreground = Some(foreground.clone().into());
    colors.muted = Some(muted.clone().into());
    colors.muted_foreground = Some(hex(palette.secondary).into());
    colors.border = Some(border.clone().into());
    colors.input = Some(border.clone().into());
    colors.popover = Some(background.clone().into());
    colors.popover_foreground = Some(foreground.clone().into());
    colors.primary = Some(primary.clone().into());
    colors.primary_foreground = Some(if dark { "#111111" } else { "#ffffff" }.into());
    colors.primary_hover = Some(if dark { "#50b9e0" } else { "#076e92" }.into());
    colors.primary_active = Some(if dark { "#2291bc" } else { "#065d7c" }.into());
    colors.button_primary = colors.primary.clone();
    colors.button_primary_foreground = colors.primary_foreground.clone();
    colors.button_primary_hover = colors.primary_hover.clone();
    colors.button_primary_active = colors.primary_active.clone();
    colors.button = Some(muted.into());
    colors.button_foreground = Some(foreground.clone().into());
    colors.button_hover = Some(hover.clone().into());
    colors.button_active = Some(selected.clone().into());
    colors.list = Some(background.clone().into());
    colors.list_active = Some(selected.clone().into());
    colors.list_active_border = Some(primary.clone().into());
    colors.list_hover = Some(hover.clone().into());
    colors.link = Some(primary.clone().into());
    colors.ring = Some(primary.clone().into());
    colors.accent = Some(hover.clone().into());
    colors.accent_foreground = Some(foreground.clone().into());
    colors.caret = colors.primary.clone();
    colors.link_hover = colors.primary_hover.clone();
    colors.link_active = colors.primary_active.clone();
    colors.selection = Some(format!("#{:06x}44", palette.accent).into());
    colors.progress_bar = colors.primary.clone();
    colors.slider_bar = colors.primary.clone();
    colors.slider_thumb = colors.primary.clone();
    colors.secondary = colors.muted.clone();
    colors.secondary_foreground = colors.foreground.clone();
    colors.secondary_hover = colors.list_hover.clone();
    colors.secondary_active = colors.list_active.clone();
    colors.button_secondary = colors.secondary.clone();
    colors.button_secondary_foreground = colors.secondary_foreground.clone();
    colors.button_secondary_hover = colors.secondary_hover.clone();
    colors.button_secondary_active = colors.secondary_active.clone();
    colors.tab = colors.muted.clone();
    colors.tab_bar = colors.background.clone();
    colors.tab_bar_segmented = colors.muted.clone();
    colors.tab_foreground = colors.muted_foreground.clone();
    colors.tab_active = colors.background.clone();
    colors.tab_active_foreground = colors.primary.clone();
    colors.title_bar = colors.background.clone();
    colors.title_bar_border = colors.border.clone();
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

pub(crate) fn caption(value: impl Into<SharedString>, cx: &App) -> Div {
    div()
        .text_size(px(12.))
        .text_color(cx.theme().muted_foreground)
        .child(value.into())
}
