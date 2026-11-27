use super::*;
use rotor_canvas::ImageRect;
use rotor_runtime::{DEFAULT_RECORD_FPS, ScreenRecordControl, ScreenRecordEvent};
use rotor_ui::{
    SCREEN_RECORD_PANEL_HEIGHT, SCREEN_RECORD_PANEL_WIDTH, SCREEN_RECORD_REVIEW_WIDTH,
    ScreenRecordAction, ScreenRecordView,
};

const PANEL_GAP: f32 = 8.;
const FRAME_THICKNESS: f32 = 2.;
const RECORD_FRAME_COLOR: (u8, u8, u8) = (0xe8, 0x5d, 0x5d);

pub(super) struct ScreenRecord {
    session: u64,
    control: ScreenRecordControl,
    remember_directory: bool,
    source_monitor: rotor_runtime::MonitorConfig,
    source_rect: ImageRect,
    // Outlines the recorded region while recording; dropped once stopped or cancelled.
    frame: Option<rotor_platform::overlay::SelectionFrame>,
}

pub(super) fn stop(cx: &mut App) {
    let Some(record) = cx.global_mut::<ShellState>().capture.record.take() else {
        return;
    };
    let session = record.session;
    drop(record);
    if let Some(slot) = cx
        .global_mut::<ShellState>()
        .windows
        .remove(&WindowRole::ScreenRecord(session))
    {
        let _ = slot
            .window
            .update(cx, |_, window, _| window.remove_window());
    }
}

fn show_frame(
    monitor: &rotor_runtime::MonitorConfig,
    rect: ImageRect,
) -> Result<rotor_platform::overlay::SelectionFrame, String> {
    let (dx, dy) = rotor_platform::monitor::pixel_origin_offset(monitor);
    let (x, y) = (monitor.x + dx, monitor.y + dy);
    let scale = monitor.scale_factor;
    rotor_platform::overlay::SelectionFrame::show_with_color(
        (
            x + rect.x as i32,
            y + rect.y as i32,
            rect.width,
            rect.height,
        ),
        (x, y, monitor.width, monitor.height),
        (FRAME_THICKNESS * scale).round().max(1.) as u32,
        scale,
        RECORD_FRAME_COLOR,
    )
}

/// Center the bar below the selection, then above it. Only overlap the
/// recording when neither side fits, anchoring to its bottom edge. All inputs
/// are logical coordinates relative to the captured display; the work area
/// excludes the menu bar, Dock, and taskbar.
fn panel_origin(work: (f32, f32, f32, f32), selection: (f32, f32, f32, f32)) -> (f32, f32) {
    let (wx, wy, ww, wh) = work;
    let (x, y, rw, rh) = selection;
    let pw = SCREEN_RECORD_PANEL_WIDTH;
    let ph = SCREEN_RECORD_PANEL_HEIGHT;
    let gap = PANEL_GAP;
    let margin_x = gap.min(((ww - pw) / 2.).max(0.));
    let margin_y = gap.min(((wh - ph) / 2.).max(0.));
    let left = (x + (rw - pw) / 2.).clamp(wx + margin_x, wx + (ww - pw - margin_x).max(margin_x));
    let top = wy + margin_y;
    let bottom = wy + (wh - ph - margin_y).max(margin_y);
    let below = y + rh + gap;
    if below >= top && below <= bottom {
        return (left, below);
    }
    let above = y - ph - gap;
    if above >= top && above <= bottom {
        return (left, above);
    }
    (left, (y + rh - ph - gap).clamp(top, bottom))
}

pub(super) fn begin(
    session: u64,
    frame: Arc<PreparedCapture>,
    rect: ImageRect,
    window: &mut Window,
    cx: &mut App,
) -> Result<(), String> {
    let locale = cx.global::<ShellState>().config.locale();
    if rect.width < 16
        || rect.height < 16
        || u64::from(rect.width) * u64::from(rect.height) > 16 * 1024 * 1024
    {
        return Err(locale
            .pick(
                "录屏选区太小或太大，请重新框选（至少 16×16 像素）",
                "Recording selection is too small or too large; select at least 16×16 pixels",
            )
            .into());
    }
    let monitor = frame.monitor.clone();
    let scale = monitor.scale_factor;
    let display = cx
        .displays()
        .into_iter()
        .find(|d| u64::from(d.id()) as u32 == monitor.id)
        .ok_or("Captured display is no longer available")?;
    let work = display.visible_bounds();
    let screen_origin = display.bounds().origin;
    let origin = panel_origin(
        (
            (work.left() - screen_origin.x).as_f32(),
            (work.top() - screen_origin.y).as_f32(),
            work.size.width.as_f32(),
            work.size.height.as_f32(),
        ),
        (
            rect.x as f32 / scale,
            rect.y as f32 / scale,
            rect.width as f32 / scale,
            rect.height as f32 / scale,
        ),
    );
    let settings = cx.global::<ShellState>().config.clone();
    let bounds = Bounds::new(
        display.bounds().origin + point(px(origin.0), px(origin.1)),
        size(
            px(SCREEN_RECORD_PANEL_WIDTH),
            px(SCREEN_RECORD_PANEL_HEIGHT),
        ),
    );
    let handle = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                display_id: Some(display.id()),
                titlebar: None,
                app_owns_titlebar_drag: true,
                window_background: WindowBackgroundAppearance::Blurred,
                window_min_size: Some(size(px(1.), px(1.))),
                kind: WindowKind::PopUp,
                is_resizable: false,
                focus: false,
                show: false,
                app_id: Some(rotor_common::native_app::IDENTIFIER.into()),
                ..Default::default()
            },
            |window, cx| {
                window.set_window_title(locale.pick("Rotor · 录屏", "Rotor · Screen record"));
                let view = cx.new(|cx| {
                    let mut view = ScreenRecordView::new(
                        locale,
                        settings,
                        Rc::new(move |action, window, cx| {
                            handle_action(session, action, window, cx);
                        }),
                        cx,
                    );
                    view.set_source(scale, rect);
                    view.focus(window, cx);
                    view
                });
                let record_window = Window::window_handle(window);
                let mut previous_height = None;
                cx.observe(&view, move |view, cx| {
                    if !is_current(session, cx) {
                        return;
                    }
                    let height = view.read(cx).review_height();
                    if height == previous_height {
                        return;
                    }
                    previous_height = height;
                    if let Some(height) = height {
                        let _ = record_window.update(cx, |_, window, cx| {
                            expand_review_panel(window, height, cx);
                        });
                    }
                })
                .detach();
                let closing = view.downgrade();
                window.on_window_should_close(cx, move |window, cx| {
                    if is_current(session, cx) {
                        let _ = closing.update(cx, |view, cx| view.request_close(window, cx));
                        false
                    } else {
                        true
                    }
                });
                cx.global_mut::<ShellState>().windows.insert(
                    WindowRole::ScreenRecord(session),
                    WindowSlot {
                        window: Window::window_handle(window),
                        view: WindowView::ScreenRecord(view.downgrade()),
                        _appearance: None,
                    },
                );
                cx.new(|cx| {
                    Root::new(view, window, cx)
                        .bordered(false)
                        .bg(gpui::transparent_black())
                })
            },
        )
        .map_err(|e| e.to_string())?;

    let setup = (|| {
        #[cfg(target_os = "windows")]
        handle
            .update(cx, |_, window, cx| {
                rotor_platform::overlay::fit_client_bounds(
                    HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?,
                    monitor.x + (origin.0 * scale).round() as i32,
                    monitor.y + (origin.1 * scale).round() as i32,
                    (SCREEN_RECORD_PANEL_WIDTH * scale).round() as u32,
                    (SCREEN_RECORD_PANEL_HEIGHT * scale).round() as u32,
                )?;
                window.bounds_changed(cx);
                Ok::<_, String>(())
            })
            .map_err(|e| e.to_string())??;
        let _ = handle.update(cx, |_, window, _| {
            if let Ok(raw) = HasWindowHandle::window_handle(window)
                && let Err(error) = rotor_platform::overlay::exclude_from_capture(raw)
            {
                log::warn!("Screen record panel capture exclusion: {error}");
            }
        });
        close_masks(Some(window), cx)?;
        handle
            .update(cx, |_, window, _| show(window))
            .map_err(|e| e.to_string())??;
        let outline = show_frame(&monitor, rect)
            .inspect_err(|error| log::warn!("Screen record frame: {error}"))
            .ok();
        cx.global::<ShellState>()
            .services
            .start_screen_record(
                OperationId(session),
                monitor.clone(),
                rect,
                DEFAULT_RECORD_FPS,
            )
            .map(|control| (control, outline))
    })();

    match setup {
        Ok((control, outline)) => {
            let state = cx.global_mut::<ShellState>();
            state.capture.detecting = None;
            state.capture.frames.clear();
            state.capture.record = Some(ScreenRecord {
                session,
                control,
                remember_directory: false,
                source_monitor: monitor,
                source_rect: rect,
                frame: outline,
            });
            Ok(())
        }
        Err(error) => {
            cx.global_mut::<ShellState>()
                .windows
                .remove(&WindowRole::ScreenRecord(session));
            let _ = handle.update(cx, |_, window, _| window.remove_window());
            let _ = super::cancel(None, cx);
            Err(error)
        }
    }
}

fn handle_action(session: u64, action: ScreenRecordAction, window: &mut Window, cx: &mut App) {
    if !is_current(session, cx) {
        return;
    }
    match action {
        ScreenRecordAction::Move => {
            #[cfg(target_os = "windows")]
            if let Err(error) = HasWindowHandle::window_handle(window)
                .map_err(|error| error.to_string())
                .and_then(rotor_platform::overlay::start_window_move)
            {
                log::warn!("Move recording panel: {error}");
            }
            #[cfg(target_os = "macos")]
            window.start_window_move();
        }
        ScreenRecordAction::Pause => {
            if let Some(record) = &cx.global::<ShellState>().capture.record {
                record.control.pause();
            }
        }
        ScreenRecordAction::Resume => {
            if let Some(record) = &cx.global::<ShellState>().capture.record {
                record.control.resume();
            }
        }
        ScreenRecordAction::Stop => {
            if let Some(record) = &cx.global::<ShellState>().capture.record {
                record.control.stop();
            }
        }
        ScreenRecordAction::Export {
            format,
            path,
            gif_preset,
            remember_directory,
        } => {
            let result = {
                let state = cx.global_mut::<ShellState>();
                if let Some(record) = state.capture.record.as_mut() {
                    record.remember_directory = remember_directory;
                    record.control.export(format, path.clone(), gif_preset)
                } else {
                    Err("Screen recording session is no longer active".into())
                }
            };
            if let Err(error) = result
                && let Some((_, view)) = record_view(session, cx)
            {
                let _ = view.update(cx, |view, cx| {
                    view.export_finished(format, path, Err(error), cx)
                });
            }
        }
        ScreenRecordAction::CancelExport => {
            if let Some(record) = &cx.global::<ShellState>().capture.record {
                record.control.cancel_export();
            }
        }
        ScreenRecordAction::OpenFile(path) => {
            if let Err(error) = cx
                .global::<ShellState>()
                .services
                .open_file(path.to_string_lossy().into_owned(), false)
            {
                log::warn!("Open exported recording: {error}");
            }
        }
        ScreenRecordAction::RevealFile(path) => {
            if let Err(error) = cx
                .global::<ShellState>()
                .services
                .reveal_file(path.to_string_lossy().into_owned())
            {
                log::warn!("Reveal exported recording: {error}");
            }
        }
        ScreenRecordAction::Close => {
            cx.global_mut::<ShellState>()
                .windows
                .remove(&WindowRole::ScreenRecord(session));
            window.remove_window();
            let _ = super::cancel(None, cx);
        }
    }
}

/// Keep the current recording session until the user saves or discards it.
/// Repeated capture shortcuts stop recording or bring its save panel forward.
pub fn stop_requested(cx: &mut App) -> bool {
    let Some(session) = cx
        .global::<ShellState>()
        .capture
        .record
        .as_ref()
        .map(|record| record.session)
    else {
        return false;
    };
    let Some((handle, view)) = record_view(session, cx) else {
        return false;
    };
    handle
        .update(cx, |_, window, cx| {
            let _ = view.update(cx, |view, cx| {
                view.stop(window, cx);
                view.focus(window, cx);
            });
            window.activate_window();
        })
        .is_ok()
}

pub fn sync_settings(config: &rotor_common::Config, cx: &mut App) {
    let Some(session) = cx
        .global::<ShellState>()
        .capture
        .record
        .as_ref()
        .map(|record| record.session)
    else {
        return;
    };
    if let Some((_, view)) = record_view(session, cx) {
        let _ = view.update(cx, |view, _| view.set_settings(config.clone()));
    }
}

fn is_current(session: u64, cx: &App) -> bool {
    let capture = &cx.global::<ShellState>().capture;
    capture.session.generation() == Some(session)
        && capture
            .record
            .as_ref()
            .is_some_and(|record| record.session == session)
}

fn record_view(session: u64, cx: &App) -> Option<(AnyWindowHandle, WeakEntity<ScreenRecordView>)> {
    cx.global::<ShellState>()
        .windows
        .get(&WindowRole::ScreenRecord(session))
        .and_then(|slot| match &slot.view {
            WindowView::ScreenRecord(view) => Some((slot.window, view.clone())),
            _ => None,
        })
}

/// Center the review on the original recording, only shifting it enough to
/// keep the save controls inside the display's usable area.
fn review_origin(
    work: (f32, f32, f32, f32),
    selection: (f32, f32, f32, f32),
    height: f32,
) -> (f32, f32) {
    let (wx, wy, ww, wh) = work;
    let (x, y, width, selection_height) = selection;
    let pw = SCREEN_RECORD_REVIEW_WIDTH;
    let ph = height;
    let mx = PANEL_GAP.min(((ww - pw) / 2.).max(0.));
    let my = PANEL_GAP.min(((wh - ph) / 2.).max(0.));
    (
        (x + (width - pw) / 2.).clamp(wx + mx, wx + (ww - pw - mx).max(mx)),
        (y + (selection_height - ph) / 2.).clamp(wy + my, wy + (wh - ph - my).max(my)),
    )
}

fn expand_review_panel(window: &mut Window, height: f32, cx: &mut App) {
    // The view fills the client area; the native window supplies its only
    // outline and corner mask. Stop blurring once recording is complete.
    window.set_background_appearance(WindowBackgroundAppearance::Transparent);
    let source = cx
        .global::<ShellState>()
        .capture
        .record
        .as_ref()
        .map(|record| (record.source_monitor.clone(), record.source_rect));
    let source_display = source.as_ref().and_then(|(monitor, _)| {
        cx.displays()
            .into_iter()
            .find(|display| u64::from(display.id()) as u32 == monitor.id)
    });
    let Some(display) = source_display
        .clone()
        .or_else(|| window.display(cx))
        .or_else(|| cx.primary_display())
    else {
        window.resize(size(px(SCREEN_RECORD_REVIEW_WIDTH), px(height)));
        return;
    };
    let work = display.visible_bounds();
    let display_bounds = display.bounds();
    let work_rect = (
        work.left().as_f32(),
        work.top().as_f32(),
        work.size.width.as_f32(),
        work.size.height.as_f32(),
    );
    let selection = match (&source, source_display) {
        (Some((monitor, rect)), Some(_)) => (
            display_bounds.left().as_f32() + rect.x as f32 / monitor.scale_factor,
            display_bounds.top().as_f32() + rect.y as f32 / monitor.scale_factor,
            rect.width as f32 / monitor.scale_factor,
            rect.height as f32 / monitor.scale_factor,
        ),
        // If the captured display was disconnected, use the current work area
        // rather than moving the review to stale off-screen coordinates.
        _ => work_rect,
    };
    let (x, y) = review_origin(work_rect, selection, height);
    let target_id = u64::from(display.id()) as u32;
    let target_monitor = cx
        .global::<ShellState>()
        .monitors
        .iter()
        .find(|monitor| monitor.id == target_id)
        .cloned()
        .or_else(|| {
            source
                .map(|(monitor, _)| monitor)
                .filter(|monitor| monitor.id == target_id)
        });
    let scale = target_monitor
        .as_ref()
        .map_or(window.scale_factor(), |monitor| monitor.scale_factor);
    #[cfg(target_os = "macos")]
    let (native_x, native_y) = (x * scale, y * scale);
    #[cfg(target_os = "windows")]
    let (native_x, native_y) = target_monitor
        .as_ref()
        .map_or((x * scale, y * scale), |monitor| {
            (
                monitor.x as f32 + (x - display_bounds.left().as_f32()) * scale,
                monitor.y as f32 + (y - display_bounds.top().as_f32()) * scale,
            )
        });
    let result = HasWindowHandle::window_handle(window)
        .map_err(|error| error.to_string())
        .and_then(|handle| {
            rotor_platform::overlay::set_client_bounds(
                handle,
                native_x.round() as i32,
                native_y.round() as i32,
                (SCREEN_RECORD_REVIEW_WIDTH * scale).round() as u32,
                (height * scale).round() as u32,
                scale,
            )
        });
    if let Err(error) = result {
        log::warn!("Position recording review: {error}");
        window.resize(size(px(SCREEN_RECORD_REVIEW_WIDTH), px(height)));
    }
    window.bounds_changed(cx);
}

pub fn handle_event(id: OperationId, event: ScreenRecordEvent, cx: &mut App) {
    if !is_current(id.0, cx) {
        return;
    }
    match event {
        ScreenRecordEvent::Preview(image) => {
            if let Some((handle, view)) = record_view(id.0, cx)
                && let Ok(prepared) = rotor_ui::prepare_image(image)
            {
                let _ = handle.update(cx, |_, _, cx| {
                    let _ = view.update(cx, |view, cx| view.preview(prepared.render, cx));
                });
            }
        }
        ScreenRecordEvent::Progress {
            elapsed_ms,
            unique_frames,
            status,
            ..
        } => {
            if let Some((handle, view)) = record_view(id.0, cx) {
                let _ = handle.update(cx, |_, _, cx| {
                    let _ = view.update(cx, |view, cx| {
                        view.progress(elapsed_ms, unique_frames, status, cx)
                    });
                });
            }
        }
        ScreenRecordEvent::Stopped {
            duration_ms,
            unique_frames,
            limit,
            ..
        } => {
            // Recording is complete; remove the on-screen selection border.
            if let Some(record) = cx.global_mut::<ShellState>().capture.record.as_mut() {
                record.frame = None;
            }
            if let Some((handle, view)) = record_view(id.0, cx) {
                let _ = handle.update(cx, |_, window, cx| {
                    let _ = view.update(cx, |view, cx| {
                        view.stopped(duration_ms, unique_frames, limit, cx)
                    });
                    let _ = view.update(cx, |view, cx| view.focus(window, cx));
                    window.activate_window();
                });
            }
        }
        ScreenRecordEvent::ExportProgress { format, progress } => {
            if let Some((handle, view)) = record_view(id.0, cx) {
                let _ = handle.update(cx, |_, _, cx| {
                    let _ = view.update(cx, |view, cx| view.export_progress(format, progress, cx));
                });
            }
        }
        ScreenRecordEvent::ExportFinished {
            format,
            path,
            result,
        } => {
            let remember = cx
                .global_mut::<ShellState>()
                .capture
                .record
                .as_mut()
                .map(|record| std::mem::take(&mut record.remember_directory))
                .unwrap_or(false);
            if result.is_ok()
                && remember
                && let Some(parent) = path.parent().and_then(|dir| dir.to_str())
                && !parent.is_empty()
            {
                let _ = cx
                    .global::<ShellState>()
                    .services
                    .save_settings(vec![("save_path".into(), parent.to_owned())]);
            }
            if let Some((handle, view)) = record_view(id.0, cx) {
                let _ = handle.update(cx, |_, _, cx| {
                    let _ = view.update(cx, |view, cx| {
                        view.export_finished(format, path, result, cx)
                    });
                });
            }
        }
        ScreenRecordEvent::Failed(error) => {
            let _ = super::cancel(None, cx);
            report(error, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PANEL_GAP, SCREEN_RECORD_PANEL_HEIGHT as H, SCREEN_RECORD_PANEL_WIDTH as W, panel_origin,
        review_origin,
    };

    #[test]
    fn bar_prefers_below_then_above_the_selection() {
        let work = (0., 0., 1920., 1080.);
        let selection = (100., 300., 800., 400.);
        assert_eq!(
            panel_origin(work, selection),
            (100. + (800. - W) / 2., 700. + PANEL_GAP)
        );
        let selection = (100., 500., 800., 560.);
        assert_eq!(
            panel_origin(work, selection),
            (100. + (800. - W) / 2., 500. - H - PANEL_GAP)
        );
    }

    #[test]
    fn fullscreen_and_tall_regions_use_the_inside_bottom_edge() {
        let work = (0., 0., 1920., 1080.);
        assert_eq!(
            panel_origin(work, (0., 0., 1920., 1080.)),
            ((1920. - W) / 2., 1080. - H - PANEL_GAP)
        );
        assert_eq!(
            panel_origin(work, (400., 12., 800., 1050.)),
            (400. + (800. - W) / 2., 1062. - H - PANEL_GAP)
        );
    }

    #[test]
    fn placement_respects_work_area_and_clamps_narrow_edge_selections() {
        let work = (0., 25., 1440., 805.);
        for selection in [
            (0., 0., 1440., 900.),
            (0., 25., 20., 800.),
            (1420., 25., 20., 800.),
        ] {
            let (x, y) = panel_origin(work, selection);
            assert!(x >= 0. && x + W <= 1440.);
            assert!(y >= 25. && y + H <= 830.);
        }
        // Coordinates remain display-local, including HiDPI displays with a
        // taskbar along the left side. No physical pixel offsets leak in.
        let (x, y) = panel_origin((60., 0., 900., 600.), (0., 0., 960., 600.));
        assert!(x >= 60. && x + W <= 960.);
        assert_eq!(y, 600. - H - PANEL_GAP);
    }
    #[test]
    fn review_centers_on_recording_even_when_selection_is_smaller_than_panel() {
        let work = (0., 25., 1440., 805.);
        assert_eq!(
            review_origin(work, (360., 180., 640., 480.), 184.),
            (520., 328.)
        );
        assert_eq!(
            review_origin(work, (600., 400., 80., 40.), 184.),
            (480., 328.)
        );
        // Revealing GIF options grows around the same selection center.
        assert_eq!(
            review_origin(work, (360., 180., 640., 480.), 220.),
            (520., 310.)
        );
    }

    #[test]
    fn review_stays_in_work_area_near_edges_and_on_negative_origin_displays() {
        let work = (0., 25., 1440., 805.);
        assert_eq!(
            review_origin(work, (1400., 780., 20., 20.), 184.),
            (1112., 638.)
        );
        assert_eq!(review_origin(work, (0., 25., 20., 20.), 184.), (8., 33.));
        assert_eq!(
            review_origin((-1600., 25., 1600., 825.), (-1000., 200., 500., 400.), 184.),
            (-910., 308.)
        );
        assert_eq!(
            review_origin((0., 25., 1920., 1000.), (0., 0., 1920., 1080.), 184.),
            (800., 448.)
        );
    }
}
