use crate::{ShellState, WindowRole, WindowSlot, WindowView};
use gpui_kit::{component::Root, *};
use raw_window_handle::HasWindowHandle;
use rotor_runtime::{CaptureBundle, NativeSession, OperationId, ShotterConfig};
use rotor_ui::{MaskAction, PreparedCapture};
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::Arc,
    time::Instant,
};

#[derive(Default)]
pub struct CaptureState {
    pub session: NativeSession,
    frames: HashMap<u32, Arc<PreparedCapture>>,
    preparing: Option<Task<()>>,
    detecting: Option<Task<()>>,
    started: Option<Instant>,
    desktop_dirty: bool,
    shown: HashSet<u32>,
}
pub fn report(error: String, cx: &mut App) {
    eprintln!("Capture: {error}");
    cx.global_mut::<ShellState>().system.warning = Some(error);
    if cx
        .global::<ShellState>()
        .capture
        .session
        .generation()
        .is_none()
    {
        let _ = crate::show_settings(cx);
    }
}
pub fn hide(window: &Window) -> Result<(), String> {
    rotor_platform::overlay::hide_window(
        HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?,
    )
}
pub fn show(window: &mut Window) -> Result<(), String> {
    rotor_platform::overlay::show_window(
        HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?,
    )?;
    window.refresh();
    Ok(())
}

fn activate_mask(window: &mut Window) -> Result<(), String> {
    // GPUI 0.3.3 applies cached initial placement on the first activation of
    // show:false windows, overwriting fit_mask's exact physical client bounds.
    #[cfg(target_os = "windows")]
    rotor_platform::overlay::activate_window_in_place(
        HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?,
    )?;
    #[cfg(not(target_os = "windows"))]
    window.activate_window();
    Ok(())
}
fn close_masks(current: Option<&mut Window>, cx: &mut App) -> Result<(), String> {
    let current_id = current
        .as_ref()
        .map(|window| Window::window_handle(window).window_id());
    let roles: Vec<_> = cx
        .global::<ShellState>()
        .windows
        .keys()
        .copied()
        .filter(|role| matches!(role, WindowRole::Mask { session, .. } if *session != 0))
        .collect();
    if !roles.is_empty() {
        cx.global_mut::<ShellState>().capture.desktop_dirty = true;
    }
    let mut failure = current.and_then(|window| {
        let result = hide(window);
        window.remove_window();
        result.err()
    });
    for role in roles {
        let entry = cx.global_mut::<ShellState>().windows.remove(&role).unwrap();
        if Some(entry.window.window_id()) == current_id {
            continue;
        }
        if let Ok(Err(error)) = entry.window.update(cx, |_, window, _| {
            let result = hide(window);
            window.remove_window();
            result
        }) {
            failure = Some(error);
        }
    }
    failure.map_or(Ok(()), Err)
}

fn mark(session: u64, stage: &str, cx: &App) {
    if let Some(started) = cx.global::<ShellState>().capture.started {
        log::debug!(target: "rotor_capture_latency", "capture_latency id={session} stage={stage} elapsed_us={}", started.elapsed().as_micros());
    }
}

fn create_mask_window(
    session: u64,
    monitor: &rotor_runtime::MonitorConfig,
    cx: &mut App,
) -> Result<AnyWindowHandle, String> {
    let role = WindowRole::Mask {
        session,
        monitor: monitor.id,
    };
    let display = cx
        .displays()
        .into_iter()
        .find(|display| u64::from(display.id()) as u32 == monitor.id)
        .ok_or("Captured display is no longer available")?;
    let chinese =
        rotor_common::i18n::language_for_config(&cx.global::<ShellState>().config) == "zh-CN";
    let frame = PreparedCapture::placeholder(monitor.clone());
    let handle = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(display.bounds())),
                display_id: Some(display.id()),
                titlebar: None,
                kind: WindowKind::PopUp,
                is_resizable: false,
                show: false,
                focus: false,
                window_min_size: Some(size(px(1.), px(1.))),
                app_id: Some(rotor_common::native_app::IDENTIFIER.into()),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| {
                    rotor_ui::MaskView::new(0, frame, Rc::new(mask_action), chinese, window, cx)
                });
                cx.global_mut::<ShellState>().windows.insert(
                    role,
                    WindowSlot {
                        window: Window::window_handle(window),
                        view: WindowView::Mask(view.downgrade()),
                        _appearance: None,
                    },
                );
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .map_err(|error| error.to_string())?;
    let handle = handle.into();
    if let Err(error) = fit_mask(handle, monitor, cx) {
        cx.global_mut::<ShellState>().windows.remove(&role);
        let _ = handle.update(cx, |_, window, _| window.remove_window());
        return Err(error);
    }
    Ok(handle)
}

fn fit_mask(
    handle: AnyWindowHandle,
    monitor: &rotor_runtime::MonitorConfig,
    cx: &mut App,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    handle
        .update(cx, |_, window, cx| {
            rotor_platform::overlay::disable_window_rounding(
                HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?,
            )?;
            rotor_platform::overlay::fit_client_bounds(
                HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?,
                monitor.x,
                monitor.y,
                monitor.width,
                monitor.height,
            )?;
            window.bounds_changed(cx);
            Ok::<_, String>(())
        })
        .map_err(|error| error.to_string())??;
    #[cfg(target_os = "macos")]
    handle
        .update(cx, |_, window, cx| {
            rotor_platform::overlay::fit_capture_screen(
                HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?,
                monitor.id,
            )?;
            window.bounds_changed(cx);
            Ok::<_, String>(())
        })
        .map_err(|error| error.to_string())??;
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let _ = (handle, monitor, cx);
    Ok(())
}

pub fn stop(cx: &mut App) {
    let state = cx.global_mut::<ShellState>();
    state.capture.session.cancel();
    state.capture.preparing = None;
    state.capture.detecting = None;
    state.capture.frames.clear();
    state.capture.shown.clear();
    state.capture.started = None;
    state.services.cancel_capture();
}
pub fn cancel(current: Option<&mut Window>, cx: &mut App) -> Result<(), String> {
    stop(cx);
    let result = close_masks(current, cx);
    crate::pins::drain_deferred(cx);
    result
}
pub fn begin(started: Instant, cx: &mut App) -> Result<(), String> {
    stop(cx);
    close_masks(None, cx)?;
    let settle = cx.global::<ShellState>().capture.desktop_dirty;
    let id = cx
        .global::<ShellState>()
        .services
        .capture_after_overlay_change(settle, started)?;
    cx.global_mut::<ShellState>().capture.started = Some(started);
    mark(id.0, "capture_requested", cx);
    cx.global_mut::<ShellState>().capture.session.begin(id.0);
    Ok(())
}
pub fn prepare_masks(id: OperationId, monitors: Vec<rotor_runtime::MonitorConfig>, cx: &mut App) {
    if !cx.global::<ShellState>().capture.session.is_capturing(id.0) {
        return;
    }
    mark(id.0, "mask_windows_preparing", cx);
    // The worker can capture pixels concurrently. These windows stay hidden and
    // inert until completed() supplies a validated frame. Register them under
    // this generation immediately so cancellation also closes unfinished masks.
    for monitor in monitors {
        if let Err(error) = create_mask_window(id.0, &monitor, cx) {
            let _ = cancel(None, cx);
            report(error, cx);
            return;
        }
    }
    mark(id.0, "mask_windows_prepared", cx);
}

pub fn completed(id: OperationId, result: Result<CaptureBundle, String>, cx: &mut App) {
    if !cx.global::<ShellState>().capture.session.is_capturing(id.0) {
        return;
    }
    let bundle = match result {
        Ok(bundle) => bundle,
        Err(error) => {
            let _ = cancel(None, cx);
            report(error, cx);
            return;
        }
    };
    cx.global_mut::<ShellState>().capture.desktop_dirty = false;
    mark(id.0, "capture_received", cx);
    let task = cx.spawn(async move |cx| {
        let prepared = cx
            .background_executor()
            .spawn(async move {
                let mut expected: Vec<_> = bundle
                    .monitors
                    .iter()
                    .map(|capture| capture.monitor.clone())
                    .collect();
                let frames = rotor_ui::prepare_capture(bundle)?;
                let mut current =
                    rotor_runtime::current_monitor_configs().map_err(|error| error.to_string())?;
                expected.sort_by_key(|monitor| monitor.id);
                current.sort_by_key(|monitor| monitor.id);
                if expected != current {
                    return Err("Display topology changed while preparing masks".to_owned());
                }
                Ok(frames)
            })
            .await;
        cx.update(|cx| {
            if !cx.global::<ShellState>().capture.session.is_capturing(id.0) {
                return;
            }
            mark(id.0, "images_prepared", cx);
            let result = prepared.and_then(|frames| open_masks(id.0, frames, cx));
            if let Err(error) = result {
                let _ = cancel(None, cx);
                report(error, cx);
            }
        });
    });
    cx.global_mut::<ShellState>().capture.preparing = Some(task);
}
fn open_masks(session: u64, frames: Vec<Arc<PreparedCapture>>, cx: &mut App) -> Result<(), String> {
    let monitors: Vec<_> = frames.iter().map(|frame| frame.monitor.clone()).collect();
    if !cx
        .global_mut::<ShellState>()
        .capture
        .session
        .complete(session, monitors.clone())?
    {
        return Ok(());
    }
    cx.global_mut::<ShellState>().monitors = monitors;
    let chinese =
        rotor_common::i18n::language_for_config(&cx.global::<ShellState>().config) == "zh-CN";
    let (cursor_display, cursor) = crate::placement::cursor_display(cx);
    let focus_id = cursor_display
        .as_ref()
        .map(|display| u64::from(display.id()) as u32)
        .or_else(|| frames.first().map(|frame| frame.monitor.id));
    for frame in frames {
        let monitor = frame.monitor.id;
        let entry = cx
            .global::<ShellState>()
            .windows
            .get(&WindowRole::Mask { session, monitor })
            .ok_or("Prepared mask is unavailable")?;
        let WindowView::Mask(view) = &entry.view else {
            return Err("Prepared mask view is unavailable".into());
        };
        if !view
            .upgrade()
            .is_some_and(|view| view.read(cx).monitor() == &frame.monitor)
        {
            return Err("Display topology changed while preparing masks".into());
        }
        let handle = entry.window;
        let view = view.clone();
        cx.global_mut::<ShellState>()
            .capture
            .frames
            .insert(monitor, frame.clone());
        let cursor = cursor_display
            .as_ref()
            .zip(cursor)
            .map(|(display, cursor)| cursor - display.bounds().origin);
        handle
            .update(cx, |_, window, cx| {
                view.update(cx, |view, cx| {
                    view.reset(session, frame, chinese, cx);
                    if Some(monitor) == focus_id
                        && let Some(cursor) = cursor
                    {
                        view.set_cursor(cursor, window, cx);
                    }
                })
                .map_err(|error| error.to_string())?;
                window.refresh();
                Ok::<_, String>(())
            })
            .map_err(|error| error.to_string())??;
        cx.spawn(async move |cx| {
            // The foreground task runs with no App/Window/Entity borrow held.
            // Obtain a fresh native handle, then let WM_PAINT re-enter GPUI.
            let ready = cx.update(|cx| {
                if !cx.global::<ShellState>().capture.session.is_ready(session, monitor) {
                    return None;
                }
                handle.update(cx, |_, window, _| {
                    HasWindowHandle::window_handle(window).map(|handle| handle.as_raw())
                        .map_err(|error| error.to_string())
                }).ok()
            });
            let Some(ready) = ready else { return; };
            #[cfg(target_os = "windows")]
            let painted = ready.and_then(rotor_platform::overlay::paint_hidden_window);
            #[cfg(not(target_os = "windows"))]
            let painted = ready.map(|_| ());
            cx.update(|cx| {
                if !cx.global::<ShellState>().capture.session.is_ready(session, monitor) { return; }
                let result = painted.and_then(|()| {
                    mark(session, "mask_paint_returned", cx);
                    handle.update(cx, |_, window, cx| {
                        show(window)?;
                        if Some(monitor) == focus_id {
                            activate_mask(window)?;
                        }
                        view.update(cx, |view, cx| {
                            if Some(monitor) == focus_id {
                                view.focus(window, cx);
                            }
                            view.arm(session, window, cx);
                        }).map_err(|error| error.to_string())?;
                        log::debug!(target: "rotor_capture_latency", "capture_latency id={session} stage=mask_show_requested monitor={monitor} elapsed_us={}",
                            cx.global::<ShellState>().capture.started.map_or(0, |started| started.elapsed().as_micros()));
                        Ok::<_, String>(())
                    }).map_err(|error| error.to_string()).and_then(|result| result)
                });
                if let Err(error) = result {
                    let _ = cancel(None, cx);
                    report(error, cx);
                } else if cx.global::<ShellState>().capture.session.is_ready(session, monitor) {
                    let state = &mut cx.global_mut::<ShellState>().capture;
                    state.shown.insert(monitor);
                    if state.shown.len() == state.frames.len() {
                        mark(session, "all_masks_show_requested", cx);
                        start_detection(session, cx);
                    }
                }
            });
        }).detach();
    }
    mark(session, "mask_frames_scheduled", cx);
    Ok(())
}

fn start_detection(session: u64, cx: &mut App) {
    let frames: Vec<_> = cx
        .global::<ShellState>()
        .capture
        .frames
        .values()
        .cloned()
        .collect();
    let services = cx.global::<ShellState>().services.clone();
    let task = cx.spawn(async move |cx| {
        for frame in frames {
            let monitor = frame.monitor.id;
            let rectangles = services.detect_capture_rectangles(frame).await;
            let current = cx.update(|cx| {
                if !cx
                    .global::<ShellState>()
                    .capture
                    .session
                    .is_ready(session, monitor)
                {
                    return false;
                }
                match rectangles {
                    Ok(rectangles) => {
                        if let Some(view) = cx
                            .global::<ShellState>()
                            .windows
                            .get(&WindowRole::Mask { session, monitor })
                            .and_then(|entry| match &entry.view {
                                WindowView::Mask(view) => Some(view.clone()),
                                _ => None,
                            })
                        {
                            let _ = view.update(cx, |view, cx| {
                                view.set_detected_rectangles(rectangles, cx)
                            });
                        }
                    }
                    Err(error) => eprintln!("Capture rectangle detection: {error}"),
                }
                true
            });
            if !current {
                break;
            }
        }
    });
    cx.global_mut::<ShellState>().capture.detecting = Some(task);
}
fn mask_action(action: MaskAction, window: &mut Window, cx: &mut App) {
    match action {
        MaskAction::Activate { session, monitor } => {
            if cx
                .global::<ShellState>()
                .capture
                .session
                .is_ready(session, monitor)
                && cx
                    .global::<ShellState>()
                    .windows
                    .get(&WindowRole::Mask { session, monitor })
                    .is_some_and(|slot| {
                        slot.window.window_id() == Window::window_handle(window).window_id()
                    })
                && let Err(error) = activate_mask(window)
            {
                report(error, cx);
            }
        }
        MaskAction::Cancel { session } | MaskAction::Invalidated { session } => {
            if cx.global::<ShellState>().capture.session.generation() != Some(session) {
                return;
            }
            let result = cancel(Some(window), cx);
            if let Err(error) = result {
                report(error, cx);
            }
            if matches!(action, MaskAction::Invalidated { .. }) {
                report("Display geometry changed; retry screenshot".into(), cx);
            }
        }
        MaskAction::Choose {
            session,
            monitor,
            rect,
        } => {
            if !cx
                .global::<ShellState>()
                .capture
                .session
                .is_ready(session, monitor)
            {
                return;
            }
            let Some(frame) = cx
                .global::<ShellState>()
                .capture
                .frames
                .get(&monitor)
                .cloned()
            else {
                return;
            };
            #[cfg(target_os = "macos")]
            let offset = (
                (frame.monitor.x as f64 * (frame.monitor.scale_factor as f64 - 1.)).round() as i32,
                (frame.monitor.y as f64 * (frame.monitor.scale_factor as f64 - 1.)).round() as i32,
            );
            #[cfg(not(target_os = "macos"))]
            let offset = (0, 0);
            let config = ShotterConfig {
                annotations: Vec::new(),
                monitor_pos: (frame.monitor.x, frame.monitor.y),
                monitor_size: (frame.monitor.width, frame.monitor.height),
                rect: (rect.x, rect.y, rect.width, rect.height),
                image_rect: (rect.x, rect.y, rect.width, rect.height),
                offset,
                zoom_factor: 100,
                mask_label: format!("ssmask-{monitor}"),
                minimized: false,
            };
            match frame
                .image
                .crop_rgba(rect)
                .and_then(|image| crate::pins::from_capture(image, config, cx))
            {
                Ok(()) => {
                    cx.global_mut::<ShellState>()
                        .capture
                        .session
                        .consume(session, monitor);
                    let _ = cancel(Some(window), cx);
                }
                Err(error) => report(error, cx),
            }
        }
    }
}
