use crate::{ShellState, WindowRole, WindowSlot, WindowView};
use gpui_kit::{component::Root, *};
use raw_window_handle::HasWindowHandle;
use rotor_runtime::{CaptureBundle, NativeSession, OperationId, ShotterConfig};
use rotor_ui::{MaskAction, PreparedCapture};
use std::{collections::HashMap, rc::Rc, sync::Arc};

#[derive(Default)]
pub struct CaptureState {
    pub session: NativeSession,
    frames: HashMap<u32, Arc<PreparedCapture>>,
    preparing: Option<Task<()>>,
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
fn close_masks(current: Option<&mut Window>, cx: &mut App) -> Result<(), String> {
    let mut failure = None;
    let current_id = current
        .as_ref()
        .map(|window| Window::window_handle(window).window_id());
    if let Some(window) = current {
        if let Err(error) = hide(window) {
            failure = Some(error);
        }
        window.remove_window();
    }
    let handles: Vec<_> = cx
        .global::<ShellState>()
        .windows
        .iter()
        .filter(|(role, entry)| {
            matches!(role, WindowRole::Mask { .. }) && Some(entry.window.window_id()) != current_id
        })
        .map(|(_, entry)| entry.window)
        .collect();
    for handle in handles {
        match handle.update(cx, |_, window, _| {
            let result = hide(window);
            window.remove_window();
            result
        }) {
            Ok(Ok(())) => {}
            Ok(Err(error)) => failure = Some(error),
            Err(_) => {}
        }
    }
    failure.map_or(Ok(()), Err)
}
pub fn stop(cx: &mut App) {
    let state = cx.global_mut::<ShellState>();
    state.capture.session.cancel();
    state.capture.preparing = None;
    state.capture.frames.clear();
    state.services.cancel_capture();
}
pub fn cancel(current: Option<&mut Window>, cx: &mut App) -> Result<(), String> {
    stop(cx);
    let result = close_masks(current, cx);
    crate::pins::drain_deferred(cx);
    result
}
pub fn begin(cx: &mut App) -> Result<(), String> {
    stop(cx);
    close_masks(None, cx)?;
    let id = cx.global::<ShellState>().services.capture()?;
    cx.global_mut::<ShellState>().capture.session.begin(id.0);
    Ok(())
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
    let callback: rotor_ui::MaskCallback = Rc::new(mask_action);
    let displays = cx.displays();
    let mut opened = Vec::new();
    for frame in frames {
        let monitor = frame.monitor.clone();
        let display = displays
            .iter()
            .find(|display| u64::from(display.id()) as u32 == monitor.id)
            .ok_or("Captured display is no longer available")?;
        cx.global_mut::<ShellState>()
            .capture
            .frames
            .insert(monitor.id, frame.clone());
        let callback = callback.clone();
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
                    app_id: Some("cc.fluctus.rotor.gpui-dev".into()),
                    ..Default::default()
                },
                |window, cx| {
                    let view =
                        cx.new(|cx| rotor_ui::MaskView::new(session, frame, callback, window, cx));
                    cx.global_mut::<ShellState>().windows.insert(
                        WindowRole::Mask {
                            session,
                            monitor: monitor.id,
                        },
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
        #[cfg(target_os = "windows")]
        handle
            .update(cx, |_, window, _| {
                rotor_platform::overlay::fit_client_bounds(
                    HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?,
                    monitor.x,
                    monitor.y,
                    monitor.width,
                    monitor.height,
                )
            })
            .map_err(|error| error.to_string())??;
        opened.push((monitor.id, handle));
    }
    for (_, handle) in &opened {
        handle
            .update(cx, |_, window, _| show(window))
            .map_err(|error| error.to_string())??;
    }
    let (cursor_display, cursor) = crate::placement::cursor_display(cx);
    let focus_id = cursor_display
        .as_ref()
        .map(|display| u64::from(display.id()) as u32)
        .or_else(|| opened.first().map(|entry| entry.0));
    for (monitor, handle) in opened {
        let view = cx
            .global::<ShellState>()
            .windows
            .get(&WindowRole::Mask { session, monitor })
            .and_then(|entry| match &entry.view {
                WindowView::Mask(view) => Some(view.clone()),
                _ => None,
            })
            .ok_or("Mask view is unavailable")?;
        handle
            .update(cx, |_, window, cx| {
                let armed = view.clone();
                window.on_next_frame(move |window, cx| {
                    let _ = armed.update(cx, |view, cx| view.arm(window, cx));
                });
                if Some(monitor) == focus_id {
                    window.activate_window();
                    let _ = view.update(cx, |view, cx| {
                        view.focus(window, cx);
                        if let Some((display, cursor)) = cursor_display.as_ref().zip(cursor) {
                            view.set_cursor(cursor - display.bounds().origin, window, cx);
                        }
                    });
                }
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
fn mask_action(action: MaskAction, window: &mut Window, cx: &mut App) {
    match action {
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
                monitor_pos: (frame.monitor.x, frame.monitor.y),
                monitor_size: (frame.monitor.width, frame.monitor.height),
                rect: (rect.x, rect.y, rect.width, rect.height),
                image_rect: Some((rect.x, rect.y, rect.width, rect.height)),
                offset,
                zoom_factor: 100,
                mask_label: format!("ssmask-{monitor}"),
                minimized: false,
            };
            match crate::pins::from_capture(frame.image.image.clone(), config, cx) {
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
