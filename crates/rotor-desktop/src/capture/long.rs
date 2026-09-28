use super::*;
use rotor_canvas::ImageRect;
use rotor_runtime::{LongCaptureControl, LongCaptureEvent};
use rotor_ui::{
    LONG_CAPTURE_PANEL_HEIGHT, LONG_CAPTURE_PANEL_WIDTH, LongCaptureAction, LongCaptureView,
};

pub(super) struct LongCapture {
    session: u64,
    control: LongCaptureControl,
    config: ShotterConfig,
    // Outlines the captured region for the session; dropped with it.
    _frame: Option<rotor_platform::overlay::SelectionFrame>,
}

pub(super) fn stop(cx: &mut App) {
    let Some(long) = cx.global_mut::<ShellState>().capture.long.take() else {
        return;
    };
    // Dropping the control cancels the worker before closing its window.
    let session = long.session;
    drop(long);
    if let Some(slot) = cx
        .global_mut::<ShellState>()
        .windows
        .remove(&WindowRole::LongCapture(session))
    {
        let _ = slot
            .window
            .update(cx, |_, window, _| window.remove_window());
    }
}

const PANEL_GAP: f32 = 16.;
const FRAME_THICKNESS: f32 = 2.;

// Drawn outside the selection so it never enters the captured pixels.
fn show_frame(
    monitor: &rotor_runtime::MonitorConfig,
    rect: ImageRect,
) -> Result<rotor_platform::overlay::SelectionFrame, String> {
    let (dx, dy) = rotor_platform::monitor::pixel_origin_offset(monitor);
    let (x, y) = (monitor.x + dx, monitor.y + dy);
    let scale = monitor.scale_factor;
    rotor_platform::overlay::SelectionFrame::show(
        (
            x + rect.x as i32,
            y + rect.y as i32,
            rect.width,
            rect.height,
        ),
        (x, y, monitor.width, monitor.height),
        (FRAME_THICKNESS * scale).round().max(1.) as u32,
        scale,
    )
}

// Reserve space for the control and its shadow outside the captured pixels.
// A fully covered display must be reselected, rather than capturing our own UI.
fn panel_origin(screen: (f32, f32), selection: (f32, f32, f32, f32)) -> Option<(f32, f32)> {
    let (w, h) = screen;
    let (x, y, rw, rh) = selection;
    let pw = LONG_CAPTURE_PANEL_WIDTH;
    let ph = LONG_CAPTURE_PANEL_HEIGHT;
    let gap = PANEL_GAP;
    if w < pw + gap * 2. || h < ph + gap * 2. {
        return None;
    }
    let left = x.clamp(gap, w - pw - gap);
    let top = y.clamp(gap, h - ph - gap);
    if y >= ph + gap * 2. {
        Some((left, y - ph - gap))
    } else if h - y - rh >= ph + gap * 2. {
        Some((left, y + rh + gap))
    } else if x >= pw + gap * 2. {
        Some((x - pw - gap, top))
    } else if w - x - rw >= pw + gap * 2. {
        Some((x + rw + gap, top))
    } else {
        None
    }
}

pub(super) fn begin(
    session: u64,
    frame: Arc<PreparedCapture>,
    rect: ImageRect,
    config: ShotterConfig,
    window: &mut Window,
    cx: &mut App,
) -> Result<(), String> {
    let locale = cx.global::<ShellState>().config.locale();
    if rect.width < 8
        || rect.height < 16
        || u64::from(rect.width) * u64::from(rect.height) > 16 * 1024 * 1024
    {
        return Err(locale
            .pick(
                "长截图选区太小或太大，请重新框选",
                "Long capture selection is too small or too large; select again",
            )
            .into());
    }
    let monitor = frame.monitor.clone();
    let scale = monitor.scale_factor;
    let origin = panel_origin(
        (monitor.width as f32 / scale, monitor.height as f32 / scale),
        (rect.x as f32 / scale, rect.y as f32 / scale, rect.width as f32 / scale, rect.height as f32 / scale),
    )
    .ok_or_else(|| {
        let (vertical, horizontal) = (
            LONG_CAPTURE_PANEL_HEIGHT + PANEL_GAP * 2.,
            LONG_CAPTURE_PANEL_WIDTH + PANEL_GAP * 2.,
        );
        match locale {
            rotor_common::Locale::Chinese => format!(
                "请缩小长截图选区，在一侧留出控制栏空间（上下约 {vertical} 像素，或左右约 {horizontal} 像素）"
            ),
            rotor_common::Locale::English => format!(
                "Select a smaller region, leaving room for the controls (about {vertical} logical pixels above/below or {horizontal} beside it)"
            ),
        }
    })?;
    let display = cx
        .displays()
        .into_iter()
        .find(|d| u64::from(d.id()) as u32 == monitor.id)
        .ok_or("Captured display is no longer available")?;
    let first = frame.image.crop_rgba(rect)?;
    let bounds = Bounds::new(
        display.bounds().origin + point(px(origin.0), px(origin.1)),
        size(px(LONG_CAPTURE_PANEL_WIDTH), px(LONG_CAPTURE_PANEL_HEIGHT)),
    );
    let handle = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                display_id: Some(display.id()),
                titlebar: None,
                kind: WindowKind::PopUp,
                is_resizable: false,
                focus: false,
                show: false,
                app_id: Some(rotor_common::native_app::IDENTIFIER.into()),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title(locale.pick("Rotor · 长截图", "Rotor · Long capture"));
                let view = cx.new(|cx| {
                    let mut view = LongCaptureView::new(
                        locale,
                        Rc::new(move |action, window, cx| {
                            if !is_current(session, cx) {
                                return;
                            }
                            match action {
                                LongCaptureAction::Finish => {
                                    if let Some(long) = &cx.global::<ShellState>().capture.long {
                                        long.control.finish();
                                    }
                                }
                                LongCaptureAction::KeepAccepted => {
                                    if let Some(long) = &cx.global::<ShellState>().capture.long {
                                        long.control.keep_accepted();
                                    }
                                }
                                LongCaptureAction::Resume => {
                                    if let Some(long) = &cx.global::<ShellState>().capture.long {
                                        long.control.resume();
                                    }
                                }
                                LongCaptureAction::Cancel => {
                                    cx.global_mut::<ShellState>()
                                        .windows
                                        .remove(&WindowRole::LongCapture(session));
                                    window.remove_window();
                                    let _ = super::cancel(None, cx);
                                }
                            }
                        }),
                        cx,
                    );
                    view.set_source(monitor.id, (monitor.width, monitor.height), rect);
                    view
                });
                window.on_window_should_close(cx, move |window, cx| {
                    if is_current(session, cx) {
                        cx.global_mut::<ShellState>()
                            .windows
                            .remove(&WindowRole::LongCapture(session));
                        let _ = super::cancel(None, cx);
                    }
                    window.remove_window();
                    false
                });
                cx.global_mut::<ShellState>().windows.insert(
                    WindowRole::LongCapture(session),
                    WindowSlot {
                        window: Window::window_handle(window),
                        view: WindowView::LongCapture(view.downgrade()),
                        _appearance: None,
                    },
                );
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .map_err(|e| e.to_string())?;
    // Keep the original capture generation alive while removing the masks.
    // Other capture requests or cancellation invalidate this worker's events.
    let setup = (|| {
        #[cfg(target_os = "windows")]
        handle
            .update(cx, |_, window, cx| {
                rotor_platform::overlay::fit_client_bounds(
                    HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?,
                    monitor.x + (origin.0 * scale).round() as i32,
                    monitor.y + (origin.1 * scale).round() as i32,
                    (LONG_CAPTURE_PANEL_WIDTH * scale).round() as u32,
                    (LONG_CAPTURE_PANEL_HEIGHT * scale).round() as u32,
                )?;
                window.bounds_changed(cx);
                Ok::<_, String>(())
            })
            .map_err(|e| e.to_string())??;
        close_masks(Some(window), cx)?;
        handle
            .update(cx, |_, window, _| show(window))
            .map_err(|e| e.to_string())??;
        // The outline is a visual aid only; capture proceeds without it.
        let frame = show_frame(&monitor, rect)
            .inspect_err(|error| log::warn!("Long capture frame: {error}"))
            .ok();
        cx.global::<ShellState>()
            .services
            .start_long_capture(OperationId(session), monitor, rect, first)
            .map(|control| (control, frame))
    })();
    match setup {
        Ok((control, frame)) => {
            let state = cx.global_mut::<ShellState>();
            state.capture.detecting = None;
            state.capture.frames.clear();
            state.capture.long = Some(LongCapture {
                session,
                control,
                config,
                _frame: frame,
            });
            Ok(())
        }
        Err(error) => {
            cx.global_mut::<ShellState>()
                .windows
                .remove(&WindowRole::LongCapture(session));
            let _ = handle.update(cx, |_, window, _| window.remove_window());
            let _ = super::cancel(None, cx);
            Err(error)
        }
    }
}

/// The capture shortcut finishes an active long capture instead of discarding
/// it, so the pointer can stay on the scrolled page.
pub fn finish_requested(cx: &mut App) -> bool {
    let Some(session) = cx
        .global::<ShellState>()
        .capture
        .long
        .as_ref()
        .map(|long| long.session)
    else {
        return false;
    };
    let target = cx
        .global::<ShellState>()
        .windows
        .get(&WindowRole::LongCapture(session))
        .and_then(|slot| match &slot.view {
            WindowView::LongCapture(view) => Some((slot.window, view.clone())),
            _ => None,
        });
    let Some((handle, view)) = target else {
        return false;
    };
    handle
        .update(cx, |_, window, cx| {
            view.update(cx, |view, cx| view.finish(window, cx))
        })
        .is_ok_and(|result| result.is_ok())
}

fn is_current(session: u64, cx: &App) -> bool {
    let capture = &cx.global::<ShellState>().capture;
    capture.session.generation() == Some(session)
        && capture
            .long
            .as_ref()
            .is_some_and(|long| long.session == session)
}

pub fn handle_event(id: OperationId, event: LongCaptureEvent, cx: &mut App) {
    if !is_current(id.0, cx) {
        return;
    }
    match event {
        LongCaptureEvent::Preview {
            current,
            tail,
            overview,
        } => {
            let target = cx
                .global::<ShellState>()
                .windows
                .get(&WindowRole::LongCapture(id.0))
                .and_then(|slot| match &slot.view {
                    WindowView::LongCapture(view) => Some((slot.window, view.clone())),
                    _ => None,
                });
            if let Some((handle, view)) = target {
                // Runtime thumbnails are bounded; only channel conversion runs here.
                if let (Ok(current), Ok(tail), Ok(overview)) = (
                    rotor_ui::prepare_image(current),
                    rotor_ui::prepare_image(tail),
                    rotor_ui::prepare_image(overview),
                ) {
                    let _ = handle.update(cx, |_, _, cx| {
                        let _ = view.update(cx, |view, cx| {
                            view.preview(current.render, tail.render, overview.render, cx)
                        });
                    });
                }
            }
        }
        LongCaptureEvent::Progress {
            frames,
            height,
            status,
            review,
        } => {
            if status.is_issue() {
                log::debug!("Long capture: {status:?}");
            }
            let target = cx
                .global::<ShellState>()
                .windows
                .get(&WindowRole::LongCapture(id.0))
                .and_then(|slot| {
                    if let WindowView::LongCapture(view) = &slot.view {
                        Some((slot.window, view.clone()))
                    } else {
                        None
                    }
                });
            if let Some((handle, view)) = target {
                let _ = handle.update(cx, |_, _, cx| {
                    let _ = view.update(cx, |view, cx| {
                        view.progress(frames, height, status, review, cx)
                    });
                });
            }
        }
        LongCaptureEvent::Finished(Err(error)) => {
            let _ = super::cancel(None, cx);
            report(error, cx);
        }
        LongCaptureEvent::Finished(Ok(image)) => {
            let mut config = cx
                .global::<ShellState>()
                .capture
                .long
                .as_ref()
                .unwrap()
                .config
                .clone();
            resize_record(&mut config, image.width(), image.height());
            let task = cx.spawn(async move |cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move {
                        rotor_ui::prepare_image(image.clone()).map(|prepared| (image, prepared))
                    })
                    .await;
                cx.update(|cx| {
                    if !is_current(id.0, cx) {
                        return;
                    }
                    match result {
                        Ok((image, prepared)) => {
                            crate::pins::from_capture(image, prepared, config, cx);
                            let _ = super::cancel(None, cx);
                        }
                        Err(error) => {
                            let _ = super::cancel(None, cx);
                            report(error, cx);
                        }
                    }
                });
            });
            cx.global_mut::<ShellState>().capture.selecting = Some(task);
        }
    }
}

fn resize_record(config: &mut ShotterConfig, width: u32, height: u32) {
    config.image_rect = (config.rect.0, config.rect.1, width, height);
    config.rect.2 = width;
    config.rect.3 = height;
    // Fit the initial preview to the monitor; stored pixels keep full resolution.
    let fit = (config.monitor_size.1 as f64 * 0.8 / height as f64)
        .min(config.monitor_size.0 as f64 * 0.8 / width as f64)
        .min(1.);
    config.zoom_factor = (fit * 100.).floor().max(1.) as _;
}

#[cfg(test)]
mod tests {
    use super::{
        LONG_CAPTURE_PANEL_HEIGHT, LONG_CAPTURE_PANEL_WIDTH, ShotterConfig, panel_origin,
        resize_record,
    };
    #[test]
    fn long_pin_preserves_source_origin_and_all_pixels() {
        let mut config = ShotterConfig {
            annotations: vec![],
            monitor_pos: (-1920, 0),
            monitor_size: (1920, 1080),
            rect: (250, 180, 600, 400),
            image_rect: (250, 180, 600, 400),
            offset: (0, 0),
            zoom_factor: 100,
            mask_label: "ssmask-1".into(),
            minimized: false,
        };
        resize_record(&mut config, 600, 5000);
        assert_eq!(
            rotor_runtime::pin_source_crop(&config, 600, 5000).unwrap(),
            (0, 0, 600, 5000)
        );
        assert_eq!(config.rect, (250, 180, 600, 5000));
        assert_eq!(config.zoom_factor, 17);
    }
    #[test]
    fn control_stays_outside_selected_pixels() {
        for selection in [
            (50., 480., 900., 540.),
            (0., 0., 800., 800.),
            (500., 0., 1400., 1080.),
            (0., 0., 1040., 768.),
        ] {
            let (x, y) = panel_origin((1920., 1080.), selection).unwrap();
            let (sx, sy, sw, sh) = selection;
            assert!(
                x >= 0.
                    && y >= 0.
                    && x + LONG_CAPTURE_PANEL_WIDTH <= 1920.
                    && y + LONG_CAPTURE_PANEL_HEIGHT <= 1080.
            );
            assert!(
                x + LONG_CAPTURE_PANEL_WIDTH < sx
                    || y + LONG_CAPTURE_PANEL_HEIGHT < sy
                    || x > sx + sw
                    || y > sy + sh
            );
        }
        assert!(panel_origin((1920., 1080.), (0., 0., 1920., 1080.)).is_none());
        assert!(panel_origin((300., 200.), (0., 0., 100., 100.)).is_none());
        // A full-height page can leave a narrow sidebar on a laptop display.
        assert!(panel_origin((1366., 768.), (0., 0., 1040., 768.)).is_some());
    }
}
