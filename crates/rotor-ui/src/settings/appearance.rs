//! Settings layout helpers using the application theme.
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
    let theme = cx.theme();
    Palette {
        background: theme.background,
        surface: theme.muted,
        border: theme.border,
        foreground: theme.foreground,
        secondary: theme.muted_foreground,
        accent: theme.primary,
        hover: theme.list_hover,
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

pub(super) fn close_button(button: Button, cx: &App) -> Button {
    let colors = palette(cx);
    button.custom(
        ButtonCustomVariant::new(cx)
            .color(colors.background)
            .foreground(colors.secondary)
            .hover(cx.theme().danger)
            .active(colors.surface),
    )
}

pub(super) fn navigation(
    button: Button,
    label: impl Into<SharedString>,
    selected: bool,
    disabled: bool,
    highlight: f32,
    cx: &App,
) -> Button {
    let colors = palette(cx);
    let label = label.into();
    let foreground = colors.foreground.blend(colors.accent.opacity(highlight));
    button
        .group("settings-navigation-button")
        .font_weight(FontWeight::NORMAL)
        .toggled(selected)
        .disabled(disabled)
        .accessibility_label(label.clone())
        .child(
            div()
                .id("navigation-label")
                .min_w_0()
                .whitespace_nowrap()
                .text_ellipsis()
                .line_height(relative(1.))
                .when(!disabled, |label| label.text_color(foreground))
                .child(label),
        )
        .custom(
            ButtonCustomVariant::new(cx)
                .color(colors.background)
                .foreground(if selected {
                    colors.accent
                } else {
                    colors.foreground
                })
                .hover(colors.background)
                .active(colors.background),
        )
}

pub(super) fn control_row(label: impl Into<SharedString>, control: impl IntoElement) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .min_w_0()
        .min_h(px(36.))
        .pl(px(12.))
        .child(div().flex_1().min_w_0().child(label.into()))
        .child(div().w(relative(0.57)).flex_shrink_0().child(control))
}

pub(super) fn group(title: &'static str, cx: &App) -> Div {
    div()
        .flex()
        .flex_col()
        .min_w_0()
        .gap(px(6.))
        .child(heading(title, cx))
}

pub(super) fn control_button(button: Button, cx: &App) -> Button {
    let colors = palette(cx);
    button.w_full().h(px(30.)).rounded_sm().custom(
        ButtonCustomVariant::new(cx)
            .color(cx.theme().tokens.switch.color)
            .foreground(colors.secondary)
            .hover(colors.hover)
            .active(cx.theme().tokens.switch.color),
    )
}

#[cfg(test)]
mod tests {
    use super::navigation;
    use gpui::{Context, IntoElement, ParentElement, Render, Window, div};
    use gpui_kit::component::button::Button;

    #[gpui::test]
    fn navigation_buttons_render_without_conflicting_hover_styles(cx: &mut gpui::TestAppContext) {
        struct NavigationHarness;

        impl Render for NavigationHarness {
            fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
                div()
                    .child(navigation(
                        Button::new("idle"),
                        "General",
                        false,
                        false,
                        0.,
                        cx,
                    ))
                    .child(navigation(
                        Button::new("selected"),
                        "Pins",
                        true,
                        false,
                        1.,
                        cx,
                    ))
                    .child(navigation(
                        Button::new("disabled"),
                        "Updates",
                        false,
                        true,
                        0.,
                        cx,
                    ))
            }
        }

        cx.update(gpui_kit::component::init);
        let (_, cx) = cx.add_window_view(|_, _| NavigationHarness);
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
}
