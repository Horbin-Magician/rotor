use gpui_kit::*;

/// Search is anchored to the primary screen, independent of cursor position and
/// result count. Keep its top edge at 30% while the list grows downwards.
pub fn search_options(cx: &App) -> WindowOptions {
    let requested = size(px(500.), px(50.));
    let display = cx.primary_display();
    let bounds = display.as_ref().map_or_else(
        || WindowBounds::centered(requested, cx).get_bounds(),
        |display| search_bounds(display.bounds(), requested),
    );
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        display_id: display.map(|display| display.id()),
        titlebar: None,
        kind: WindowKind::PopUp,
        is_resizable: false,
        app_id: Some(rotor_common::native_app::IDENTIFIER.into()),
        ..Default::default()
    }
}

fn search_bounds(screen: Bounds<Pixels>, requested: Size<Pixels>) -> Bounds<Pixels> {
    let dimensions = requested.min(&screen.size);
    Bounds::new(
        point(
            screen.left() + (screen.size.width - dimensions.width) / 2.,
            (screen.top() + screen.size.height * 0.3).min(screen.bottom() - dimensions.height),
        ),
        dimensions,
    )
}

pub fn utility_options(requested: Size<Pixels>, near_cursor: bool, cx: &App) -> WindowOptions {
    let (display, cursor) = cursor_display(cx);
    let bounds = if let Some(display) = &display {
        let work = display.visible_bounds();
        let dimensions = requested.min(&work.size);
        let origin = if near_cursor {
            cursor.unwrap_or(work.center()) + point(px(12.), px(12.))
        } else {
            work.center() - point(dimensions.width / 2., dimensions.height / 2.)
        };
        Bounds::new(
            point(
                origin
                    .x
                    .max(work.left())
                    .min(work.right() - dimensions.width),
                origin
                    .y
                    .max(work.top())
                    .min(work.bottom() - dimensions.height),
            ),
            dimensions,
        )
    } else {
        WindowBounds::centered(requested, cx).get_bounds()
    };
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        display_id: display.map(|display| display.id()),
        titlebar: None,
        kind: WindowKind::PopUp,
        is_resizable: false,
        app_id: Some(rotor_common::native_app::IDENTIFIER.into()),
        ..Default::default()
    }
}

pub(super) fn cursor_display(
    cx: &App,
) -> (
    Option<std::rc::Rc<dyn PlatformDisplay>>,
    Option<Point<Pixels>>,
) {
    #[cfg(target_os = "windows")]
    if let Ok(cursor) = rotor_platform::cursor::location() {
        return (
            cx.find_display(DisplayId::new(cursor.display_id)),
            Some(point(px(cursor.x), px(cursor.y))),
        );
    }
    #[cfg(not(target_os = "windows"))]
    if let Ok((x, y)) = rotor_platform::sys_util::get_cursor_position() {
        let cursor = point(px(x as f32), px(y as f32));
        let display = cx
            .displays()
            .into_iter()
            .find(|display| display.bounds().contains(&cursor));
        if display.is_some() {
            return (display, Some(cursor));
        }
    }
    (cx.primary_display(), None)
}

#[cfg(test)]
mod tests {
    use super::search_bounds;
    use gpui_kit::{Bounds, point, px, size};

    #[test]
    fn search_is_centered_horizontally_and_anchored_above_center() {
        let screen = Bounds::new(point(px(-1920.), px(120.)), size(px(1920.), px(1080.)));
        let collapsed = search_bounds(screen, size(px(500.), px(50.)));
        let expanded = search_bounds(screen, size(px(500.), px(470.)));
        assert_eq!(collapsed.origin, point(px(-1210.), px(444.)));
        assert_eq!(collapsed.origin, expanded.origin);
        assert_eq!(collapsed.size, size(px(500.), px(50.)));
    }

    #[test]
    fn search_fits_small_screens() {
        let screen = Bounds::new(point(px(0.), px(0.)), size(px(320.), px(40.)));
        assert_eq!(search_bounds(screen, size(px(500.), px(50.))), screen);
    }
}
