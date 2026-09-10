use crate::{ShellState, WindowRole, WindowSlot, WindowView};
use gpui_kit::{component::Root, *};
use raw_window_handle::HasWindowHandle;
use rotor_runtime::{OperationId, PinEvent, RuntimeEvent, ShotterConfig};
use rotor_ui::PreparedImage;
use std::{collections::HashMap, rc::Rc, sync::Arc};

struct DeferredPin {
    image: PreparedImage,
    config: ShotterConfig,
    id: Option<u32>,
    error: Option<String>,
    activate: bool,
}
struct PendingImage {
    image: Arc<image::RgbaImage>,
    config: ShotterConfig,
    id: Option<u32>,
    error: Option<String>,
    activate: bool,
}
#[derive(Default)]
pub struct PinWindows {
    next: u64,
    deferred: Vec<DeferredPin>,
    restoring: Option<Task<()>>,
    reveal_request: Option<OperationId>,
    creating: HashMap<OperationId, PendingImage>,
    preparing: HashMap<u64, Task<()>>,
}
pub fn stop(cx: &mut App) {
    let state = cx.global_mut::<ShellState>();
    state.pins.restoring = None;
    state.pins.reveal_request = None;
    state.pins.deferred.clear();
    state.pins.creating.clear();
    state.pins.preparing.clear();
}
fn views(cx: &App) -> Vec<(AnyWindowHandle, WeakEntity<rotor_ui::PinView>)> {
    cx.global::<ShellState>()
        .windows
        .values()
        .filter_map(|entry| match &entry.view {
            WindowView::Pin(view) => Some((entry.window, view.clone())),
            _ => None,
        })
        .collect()
}
pub fn final_records(cx: &App) -> Vec<(u32, ShotterConfig)> {
    views(cx)
        .into_iter()
        .filter_map(|(_, view)| {
            view.upgrade()
                .and_then(|view| view.read(cx).shutdown_record())
        })
        .collect()
}
pub fn show_all(cx: &mut App) {
    drain_deferred(cx);
    if cx
        .global::<ShellState>()
        .capture
        .session
        .generation()
        .is_some()
    {
        return;
    }
    for (handle, view) in views(cx) {
        let _ = handle.update(cx, |_, window, cx| {
            let _ = crate::capture::show(window);
            let _ = view.update(cx, |view, cx| view.reveal(window, cx));
        });
    }
    if cx.global::<ShellState>().pins.reveal_request.is_none() {
        let excluded_ids = views(cx)
            .into_iter()
            .filter_map(|(_, view)| view.upgrade().and_then(|view| view.read(cx).persisted_id()))
            .collect();
        match cx
            .global::<ShellState>()
            .services
            .restore_hidden_pins(excluded_ids)
        {
            Ok(id) => cx.global_mut::<ShellState>().pins.reveal_request = Some(id),
            Err(error) => crate::capture::report(error, cx),
        }
    }
}
pub fn from_capture(
    image: Arc<image::RgbaImage>,
    config: ShotterConfig,
    cx: &mut App,
) -> Result<(), String> {
    let pending = PendingImage {
        image: image.clone(),
        config: config.clone(),
        id: None,
        error: None,
        activate: true,
    };
    match cx.global::<ShellState>().services.create_pin(image, config) {
        Ok(id) => {
            cx.global_mut::<ShellState>()
                .pins
                .creating
                .insert(id, pending);
            Ok(())
        }
        Err(error) => queue_image(
            PendingImage {
                error: Some(error),
                ..pending
            },
            cx,
        ),
    }
}
fn queue_image(pending: PendingImage, cx: &mut App) -> Result<(), String> {
    let token = cx
        .global::<ShellState>()
        .pins
        .next
        .checked_add(1)
        .ok_or("Pin task ID space exhausted")?;
    cx.global_mut::<ShellState>().pins.next = token;
    let task = cx.spawn(async move |cx| {
        let prepared = cx
            .background_executor()
            .spawn(async move {
                let image = pending.image;
                Ok::<_, String>(DeferredPin {
                    image: rotor_ui::prepare_image(image)?,
                    config: pending.config,
                    id: pending.id,
                    error: pending.error,
                    activate: pending.activate,
                })
            })
            .await;
        cx.update(|cx| {
            cx.global_mut::<ShellState>().pins.preparing.remove(&token);
            match prepared {
                Ok(pin) => {
                    cx.global_mut::<ShellState>().pins.deferred.push(pin);
                    drain_deferred(cx);
                }
                Err(error) => crate::capture::report(error, cx),
            }
        });
    });
    cx.global_mut::<ShellState>()
        .pins
        .preparing
        .insert(token, task);
    Ok(())
}
pub fn handle_event(event: &RuntimeEvent, cx: &mut App) {
    if let RuntimeEvent::Pin(PinEvent::Created { id, result }) = event
        && let Some(pending) = cx.global_mut::<ShellState>().pins.creating.remove(id)
    {
        let pending = match result {
            Ok(pin) => PendingImage {
                image: pin.image.clone(),
                config: pin.config.clone(),
                id: Some(pin.id),
                error: None,
                activate: true,
            },
            Err(error) => PendingImage {
                error: Some(format!(
                    "Pin is not persisted; Save or Copy is still available: {error}"
                )),
                ..pending
            },
        };
        if let Err(error) = queue_image(pending, cx) {
            crate::capture::report(error, cx);
        }
    }
    if let RuntimeEvent::Pin(PinEvent::Restored { id, reveal, result }) = event {
        let id = *id;
        let reveal = *reveal;
        if reveal && cx.global::<ShellState>().pins.reveal_request != Some(id) {
            return;
        }
        match result {
            Ok(restored) => {
                log::info!(
                    "Restoring {} pins; {} metadata warnings",
                    restored.pins.len(),
                    restored.warnings.len()
                );
                if !restored.warnings.is_empty() {
                    crate::publish_warning(restored.warnings.join("\n"), cx);
                }
                let pins = restored.pins.clone();
                let task = cx.spawn(async move |cx| {
                    let prepared = cx
                        .background_executor()
                        .spawn(async move {
                            let monitors = rotor_runtime::current_monitor_configs()
                                .map_err(|error| error.to_string())?;
                            let mut images = Vec::new();
                            for mut pin in pins {
                                if reveal {
                                    pin.config.minimized = false;
                                }
                                images.push(DeferredPin {
                                    image: rotor_ui::prepare_image(pin.image)?,
                                    config: pin.config,
                                    id: Some(pin.id),
                                    error: None,
                                    activate: reveal,
                                });
                            }
                            Ok::<_, String>((monitors, images))
                        })
                        .await;
                    cx.update(|cx| {
                        if reveal {
                            if cx.global::<ShellState>().pins.reveal_request != Some(id) {
                                return;
                            }
                            cx.global_mut::<ShellState>().pins.reveal_request = None;
                        }
                        match prepared {
                            Ok((monitors, images)) => {
                                log::info!(
                                    "Prepared {} restored pins across {} displays",
                                    images.len(),
                                    monitors.len()
                                );
                                if cx
                                    .global::<ShellState>()
                                    .capture
                                    .session
                                    .generation()
                                    .is_none()
                                {
                                    cx.global_mut::<ShellState>().monitors = monitors;
                                }
                                cx.global_mut::<ShellState>().pins.deferred.extend(images);
                                drain_deferred(cx);
                            }
                            Err(error) => crate::capture::report(error, cx),
                        }
                    });
                });
                cx.global_mut::<ShellState>().pins.restoring = Some(task);
            }
            Err(error) => {
                if reveal {
                    cx.global_mut::<ShellState>().pins.reveal_request = None;
                }
                crate::capture::report(error.clone(), cx);
            }
        }
    }
    for (handle, view) in views(cx) {
        let _ = handle.update(cx, |_, window, cx| {
            let _ = view.update(cx, |view, cx| view.handle_event(event, window, cx));
        });
    }
}
pub fn drain_deferred(cx: &mut App) {
    if cx
        .global::<ShellState>()
        .capture
        .session
        .generation()
        .is_some()
    {
        return;
    }
    let deferred = std::mem::take(&mut cx.global_mut::<ShellState>().pins.deferred);
    for pin in deferred {
        if pin.id.is_some()
            && views(cx).iter().any(|(_, view)| {
                view.upgrade()
                    .is_some_and(|view| view.read(cx).persisted_id() == pin.id)
            })
        {
            continue;
        }
        let visible = !pin.config.minimized;
        match open(pin, cx) {
            Ok((handle, activate)) if visible => {
                let _ = handle.update(cx, |_, window, _| {
                    if let Err(error) = crate::capture::show(window) {
                        log::warn!("Could not show restored pin: {error}");
                    }
                    if activate {
                        window.activate_window();
                    }
                });
            }
            Ok(_) => {}
            Err(error) => crate::capture::report(error, cx),
        }
    }
}
fn open(pin: DeferredPin, cx: &mut App) -> Result<(AnyWindowHandle, bool), String> {
    let DeferredPin {
        image,
        mut config,
        id,
        error,
        activate,
    } = pin;
    let preferred = config
        .mask_label
        .strip_prefix("ssmask-")
        .and_then(|id| id.parse::<u32>().ok());
    let saved_x = config.monitor_pos.0 as i64 + config.rect.0 as i64 + config.offset.0 as i64;
    let saved_y = config.monitor_pos.1 as i64 + config.rect.1 as i64 + config.offset.1 as i64;
    let displays = cx.displays();
    let state = cx.global::<ShellState>();
    let contains = |monitor: &&rotor_runtime::MonitorConfig| {
        displays
            .iter()
            .find(|display| u64::from(display.id()) as u32 == monitor.id)
            .is_some_and(|display| {
                let point = point(
                    px(saved_x as f32 / monitor.scale_factor),
                    px(saved_y as f32 / monitor.scale_factor),
                );
                display.bounds().contains(&point)
            })
    };
    let primary = cx
        .primary_display()
        .map(|display| u64::from(display.id()) as u32);
    let monitor = state
        .monitors
        .iter()
        .filter(contains)
        .find(|monitor| Some(monitor.id) == preferred)
        .or_else(|| state.monitors.iter().find(contains))
        .or_else(|| {
            state
                .monitors
                .iter()
                .find(|monitor| Some(monitor.id) == preferred)
        })
        .or_else(|| {
            state
                .monitors
                .iter()
                .find(|monitor| Some(monitor.id) == primary)
        })
        .or_else(|| state.monitors.first())
        .cloned()
        .ok_or("No display is available for this pin")?;
    let display = displays
        .iter()
        .find(|display| u64::from(display.id()) as u32 == monitor.id)
        .ok_or("Pin display is no longer available")?;
    let scale = monitor.scale_factor;
    if !scale.is_finite() || scale <= 0. {
        return Err("Invalid pin display scale".into());
    }
    let content_scale = state
        .monitors
        .iter()
        .find(|source| {
            (source.x, source.y) == config.monitor_pos
                && (source.width, source.height) == config.monitor_size
        })
        .map(|source| source.scale_factor)
        .filter(|scale| scale.is_finite() && *scale > 0.)
        .unwrap_or(scale);
    let (_, _, width, height) =
        rotor_runtime::pin_source_crop(&config, image.image.width(), image.image.height())?;
    let maximum_zoom = (8192. / width.max(height) as f32 * content_scale / scale * 100.)
        .floor()
        .clamp(1., 500.) as u32;
    config.zoom_factor = config.zoom_factor.clamp(5.min(maximum_zoom), maximum_zoom);
    config.mask_label = format!("ssmask-{}", monitor.id);
    let dimensions = size(
        px(
            (width as f32 * config.zoom_factor as f32 / 100. / content_scale)
                .round()
                .max(1.),
        ),
        px(
            (height as f32 * config.zoom_factor as f32 / 100. / content_scale)
                .round()
                .max(1.),
        ),
    );
    let position = point(px(saved_x as f32 / scale), px(saved_y as f32 / scale));
    let work = display.visible_bounds();
    let position = point(
        position
            .x
            .max(work.left())
            .min((work.right() - dimensions.width).max(work.left())),
        position
            .y
            .max(work.top())
            .min((work.bottom() - dimensions.height).max(work.top())),
    );
    config.offset = (
        i32::try_from(
            (position.x.as_f32() as f64 * scale as f64).round() as i64
                - config.monitor_pos.0 as i64
                - config.rect.0 as i64,
        )
        .map_err(|_| "Pin X position overflows its record")?,
        i32::try_from(
            (position.y.as_f32() as f64 * scale as f64).round() as i64
                - config.monitor_pos.1 as i64
                - config.rect.1 as i64,
        )
        .map_err(|_| "Pin Y position overflows its record")?,
    );
    let token = cx
        .global::<ShellState>()
        .pins
        .next
        .checked_add(1)
        .ok_or("Pin window ID space exhausted")?;
    cx.global_mut::<ShellState>().pins.next = token;
    let services = cx.global::<ShellState>().services.clone();
    let reader: rotor_ui::PinPositionReader = Rc::new(|window| {
        #[cfg(target_os = "windows")]
        {
            rotor_platform::overlay::client_origin(HasWindowHandle::window_handle(window).ok()?)
                .ok()
        }
        #[cfg(not(target_os = "windows"))]
        {
            let point = window.inner_window_bounds().get_bounds().origin;
            let scale = window.scale_factor();
            Some((
                (point.x.as_f32() * scale).round() as i32,
                (point.y.as_f32() * scale).round() as i32,
            ))
        }
    });
    let bounds_setter: rotor_ui::PinBoundsSetter = Rc::new(|window, bounds| {
        rotor_platform::overlay::set_client_bounds(
            HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?,
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            window.scale_factor(),
        )
    });
    let pointer: rotor_ui::PinPointerCapture = Rc::new(|window, capture| {
        rotor_platform::overlay::pointer_capture(
            HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?,
            capture,
        )
    });
    let handle = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(position, dimensions))),
                display_id: Some(display.id()),
                titlebar: None,
                kind: WindowKind::PopUp,
                is_resizable: false,
                show: false,
                focus: false,
                window_background: WindowBackgroundAppearance::Transparent,
                window_min_size: Some(size(px(1.), px(1.))),
                app_id: Some(rotor_common::native_app::IDENTIFIER.into()),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| {
                    rotor_ui::PinView::new(
                        services,
                        rotor_ui::PinInit {
                            image,
                            config,
                            id,
                            pending: None,
                            error,
                            position: reader,
                            content_scale,
                            bounds: bounds_setter,
                            pointer,
                        },
                        window,
                        cx,
                    )
                });
                let closing = view.downgrade();
                window.on_window_should_close(cx, move |window, cx| {
                    let _ = closing.update(cx, |view, cx| view.close(window, cx));
                    false
                });
                cx.global_mut::<ShellState>().windows.insert(
                    WindowRole::Pin(token),
                    WindowSlot {
                        window: Window::window_handle(window),
                        view: WindowView::Pin(view.downgrade()),
                        _appearance: None,
                    },
                );
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .map_err(|error| error.to_string())?;
    #[cfg(target_os = "windows")]
    let fitted = handle
        .update(cx, |_, window, _| {
            rotor_platform::overlay::fit_client_bounds(
                HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?,
                (position.x.as_f32() * scale).round() as i32,
                (position.y.as_f32() * scale).round() as i32,
                (dimensions.width.as_f32() * scale).round().max(1.) as u32,
                (dimensions.height.as_f32() * scale).round().max(1.) as u32,
            )
        })
        .map_err(|error| error.to_string())
        .and_then(|result| result);
    #[cfg(target_os = "windows")]
    if let Err(error) = fitted {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
        return Err(error);
    }
    if id.is_some()
        && let Some(view) = cx
            .global::<ShellState>()
            .windows
            .get(&WindowRole::Pin(token))
            .and_then(|entry| match &entry.view {
                WindowView::Pin(view) => Some(view.clone()),
                _ => None,
            })
    {
        let _ = view.update(cx, |view, cx| view.persist_geometry(cx));
    }
    Ok((*handle, activate))
}
