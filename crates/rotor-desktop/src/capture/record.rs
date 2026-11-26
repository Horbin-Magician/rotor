use super::*;
use rotor_canvas::ImageRect;
use rotor_runtime::{DEFAULT_RECORD_FPS, ScreenRecordControl, ScreenRecordEvent};
use rotor_ui::{
    SCREEN_RECORD_PANEL_HEIGHT, SCREEN_RECORD_PANEL_WIDTH, ScreenRecordAction, ScreenRecordView,
};

const PANEL_GAP: f32 = 16.;
const FRAME_THICKNESS: f32 = 2.;
const RECORD_FRAME_COLOR: (u8, u8, u8) = (0xe8, 0x5d, 0x5d);

pub(super) struct ScreenRecord {
    session: u64,
    control: ScreenRecordControl,
    remember_directory: bool,
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

/// Prefer placing the control panel outside the recorded region so it stays out
/// of the user's way. When the selection covers nearly the full display, place
/// it in the top-right corner (the window is excluded from OS capture).
fn panel_origin(screen: (f32, f32), selection: (f32, f32, f32, f32)) -> (f32, f32) {
    let (w, h) = screen;
    let (x, y, rw, rh) = selection;
    let pw = SCREEN_RECORD_PANEL_WIDTH;
    let ph = SCREEN_RECORD_PANEL_HEIGHT;
    let gap = PANEL_GAP;
    if w >= pw + gap * 2. && h >= ph + gap * 2. {
        let left = x.clamp(gap, w - pw - gap);
        let top = y.clamp(gap, h - ph - gap);
        if y >= ph + gap * 2. {
            return (left, y - ph - gap);
        }
        if h - y - rh >= ph + gap * 2. {
            return (left, y + rh + gap);
        }
        if x >= pw + gap * 2. {
            return (x - pw - gap, top);
        }
        if w - x - rw >= pw + gap * 2. {
            return (x + rw + gap, top);
        }
    }
    ((w - pw - gap).max(0.), gap.min((h - ph).max(0.)))
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
    let origin = panel_origin(
        (monitor.width as f32 / scale, monitor.height as f32 / scale),
        (
            rect.x as f32 / scale,
            rect.y as f32 / scale,
            rect.width as f32 / scale,
            rect.height as f32 / scale,
        ),
    );
    let display = cx
        .displays()
        .into_iter()
        .find(|d| u64::from(d.id()) as u32 == monitor.id)
        .ok_or("Captured display is no longer available")?;
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
                        DEFAULT_RECORD_FPS,
                        Rc::new(move |action, window, cx| {
                            handle_action(session, action, window, cx);
                        }),
                        cx,
                    );
                    view.set_source(monitor.id, (monitor.width, monitor.height), scale, rect);
                    view
                });
                window.on_window_should_close(cx, move |window, cx| {
                    if is_current(session, cx) {
                        cx.global_mut::<ShellState>()
                            .windows
                            .remove(&WindowRole::ScreenRecord(session));
                        let _ = super::cancel(None, cx);
                    }
                    window.remove_window();
                    false
                });
                cx.global_mut::<ShellState>().windows.insert(
                    WindowRole::ScreenRecord(session),
                    WindowSlot {
                        window: Window::window_handle(window),
                        view: WindowView::ScreenRecord(view.downgrade()),
                        _appearance: None,
                    },
                );
                cx.new(|cx| Root::new(view, window, cx))
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
            .start_screen_record(OperationId(session), monitor, rect, DEFAULT_RECORD_FPS)
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

/// Pressing the capture shortcut while actively recording stops the recording
/// and moves to the export panel instead of discarding the clip.
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
            view.update(cx, |view, cx| view.stop(window, cx))
        })
        .is_ok_and(|result| result.unwrap_or(false))
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
            frames,
            unique_frames,
            spool_bytes,
            status,
        } => {
            if let Some((handle, view)) = record_view(id.0, cx) {
                let _ = handle.update(cx, |_, _, cx| {
                    let _ = view.update(cx, |view, cx| {
                        view.progress(elapsed_ms, frames, unique_frames, spool_bytes, status, cx)
                    });
                });
            }
        }
        ScreenRecordEvent::Stopped {
            duration_ms,
            frames,
            unique_frames,
            spool_bytes,
            limit,
        } => {
            // Recording is complete; remove the on-screen selection border.
            if let Some(record) = cx.global_mut::<ShellState>().capture.record.as_mut() {
                record.frame = None;
            }
            if let Some((handle, view)) = record_view(id.0, cx) {
                let _ = handle.update(cx, |_, window, cx| {
                    let _ = view.update(cx, |view, cx| {
                        view.stopped(duration_ms, frames, unique_frames, spool_bytes, limit, cx)
                    });
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
    use super::{SCREEN_RECORD_PANEL_HEIGHT, SCREEN_RECORD_PANEL_WIDTH, panel_origin};

    #[test]
    fn panel_prefers_outside_selection_and_falls_back_on_fullscreen() {
        let (x, y) = panel_origin((1920., 1080.), (100., 500., 800., 500.));
        assert!(x >= 0. && y >= 0.);
        assert!(y + SCREEN_RECORD_PANEL_HEIGHT < 500.);

        // Fullscreen recording still gets a valid on-screen corner position.
        let (fx, fy) = panel_origin((1920., 1080.), (0., 0., 1920., 1080.));
        assert!(fx >= 0. && fx + SCREEN_RECORD_PANEL_WIDTH <= 1920.);
        assert!(fy >= 0. && fy + SCREEN_RECORD_PANEL_HEIGHT <= 1080.);
    }
}
