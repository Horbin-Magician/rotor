use gpui_kit::*;

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
        app_id: Some("cc.fluctus.rotor.gpui-dev".into()),
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
