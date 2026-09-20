//! Route runtime events to windows and execute shell commands.
use crate::{
    capture, pins,
    shell::{
        ShellState, WindowRole, apply_theme, quit_in_progress, request_quit, show_search,
        show_settings, show_translator,
    },
    system::Command,
};
use gpui_kit::*;
use rotor_common::{Settings, settings::keys};
use rotor_runtime::RuntimeEvent;

pub(crate) fn handle_event(event: RuntimeEvent, cx: &mut App) {
    if matches!(&event, RuntimeEvent::Update(snapshot) if snapshot.phase == rotor_runtime::UpdatePhase::HandedOff)
    {
        request_quit(cx);
        return;
    }
    if let RuntimeEvent::SettingsCoordination(request) = event {
        match request {
            rotor_runtime::SettingsCoordination::Prepare {
                id,
                candidate,
                reply,
            } => {
                let result = cx.global_mut::<ShellState>().system.prepare(id, &candidate);
                let _ = reply.send(result);
            }
            rotor_runtime::SettingsCoordination::Finish {
                id,
                committed,
                reply,
            } => {
                let result = cx.global_mut::<ShellState>().system.finish(id, committed);
                let _ = reply.send(result);
            }
        }
        return;
    }
    if let RuntimeEvent::CapturePreparing { id, monitors } = event {
        capture::prepare_masks(id, monitors, cx);
        return;
    }
    if let RuntimeEvent::CaptureFinished { id, result } = event {
        capture::completed(id, result, cx);
        return;
    }
    pins::handle_event(&event, cx);
    if let RuntimeEvent::SelectionFinished { id, result } = event {
        if cx.global::<ShellState>().pending_selection != Some(id) {
            return;
        }
        cx.global_mut::<ShellState>().pending_selection = None;
        match result {
            Ok(selected) => {
                if let Err(error) = show_translator(cx) {
                    log::error!("Translator: {error}");
                    return;
                }
                if let Some((handle, view)) = cx.global::<ShellState>().translator() {
                    let _ = handle.update(cx, |_, window, cx| {
                        let _ = view.update(cx, |view, cx| {
                            view.translate_text(selected.text, selected.restore_warning, window, cx)
                        });
                    });
                }
            }
            Err(error) => {
                cx.global_mut::<ShellState>().system.warning = Some(error);
                if let Err(error) = show_settings(cx) {
                    log::error!("Selection: {error}");
                }
            }
        }
        return;
    }
    if let Some((handle, view)) = cx.global::<ShellState>().search() {
        let _ = handle.update(cx, |_, window, cx| {
            let _ = view.update(cx, |view, cx| view.handle_event(&event, window, cx));
        });
    }
    if let RuntimeEvent::SettingsSaved {
        result: Ok(config), ..
    } = &event
    {
        let theme_changed = cx.global::<ShellState>().config.theme() != config.theme();
        let language_changed = cx.global::<ShellState>().config.language() != config.language();
        let exclusions_changed = cx
            .global::<ShellState>()
            .config
            .get(keys::SEARCH_EXCLUDED_DIRS)
            != config.get(keys::SEARCH_EXCLUDED_DIRS);
        cx.global_mut::<ShellState>().config = config.clone();
        if exclusions_changed {
            let services = cx.global::<ShellState>().services.clone();
            let rebuild = cx.spawn(async move |cx| {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
                services.rebuild_search();
            });
            cx.global_mut::<ShellState>().index_rebuild = Some(rebuild);
        }
        if theme_changed {
            apply_theme(config, cx);
        }
        if language_changed || theme_changed {
            let state = cx.global_mut::<ShellState>();
            if let Err(error) = state.system.update_menu(state.commands.clone(), config) {
                log::error!("Tray menu: {error}");
            }
        }
        if language_changed
            && let Some(handle) = cx.global::<ShellState>().window(WindowRole::Settings)
        {
            let _ = handle.update(cx, |_, window, _| {
                window.set_window_title(rotor_ui::settings_title(config))
            });
        }
    }
    if let Some((handle, view)) = cx.global::<ShellState>().translator() {
        let _ = handle.update(cx, |_, window, cx| {
            let _ = view.update(cx, |view, cx| view.handle_event(&event, window, cx));
        });
    }
    if let Some((handle, view)) = cx.global::<ShellState>().settings() {
        let _ = handle.update(cx, |_, window, cx| {
            let _ = view.update(cx, |view, cx| view.handle_event(event, window, cx));
        });
    }
}

/// Execute one command from the tray, a hotkey or another process instance.
pub(crate) fn dispatch_command(command: Command, cx: &mut App) {
    // Keep draining runtime coordination while a save
    // finishes, but don't begin another tool operation.
    if quit_in_progress(cx) && !matches!(command, Command::Quit) {
        return;
    }
    let started = match command {
        Command::Shortcut { pressed_at, .. } => pressed_at,
        _ => std::time::Instant::now(),
    };
    let command = match command {
        Command::Shortcut {
            key, generation, ..
        } if cx.global::<ShellState>().services.is_shortcut_recording() => {
            Command::RecordedShortcut { key, generation }
        }
        other => other,
    };
    if let Command::RecordedShortcut { key, generation } = command {
        if let Some(value) = cx
            .global::<ShellState>()
            .system
            .shortcut_label(key, generation)
            && let Some((handle, view)) = cx.global::<ShellState>().settings()
        {
            let _ = handle.update(cx, |_, window, cx| {
                let _ = view.update(cx, |view, cx| {
                    view.receive_recorded_shortcut(value, window, cx)
                });
            });
        }
        return;
    }
    let command = if let Command::Shortcut {
        key, generation, ..
    } = command
    {
        if cx
            .global::<ShellState>()
            .services
            .shortcut_recording_flag()
            .quiet(std::time::Instant::now())
        {
            return;
        }
        use rotor_runtime::shortcuts::ShortcutAction;
        match cx.global::<ShellState>().system.resolve(key, generation) {
            Some(ShortcutAction::Settings) => Command::ShowSettings,
            Some(ShortcutAction::Search) => Command::ShowSearch,
            Some(ShortcutAction::Capture) => Command::Capture,
            Some(ShortcutAction::TranslateSelection) => Command::SelectText,
            Some(ShortcutAction::TranslateInput) => Command::ShowTranslator,
            Some(ShortcutAction::Quick(id)) => {
                if let Err(error) = cx.global::<ShellState>().services.run_quick_action(id) {
                    log::error!("Quick action: {error}");
                }
                return;
            }
            None => return,
        }
    } else {
        command
    };
    if !matches!(command, Command::Capture | Command::Quit)
        && cx
            .global::<ShellState>()
            .capture
            .session
            .generation()
            .is_some()
    {
        let _ = capture::cancel(None, cx);
    }
    if !matches!(command, Command::SelectText) {
        let state = cx.global_mut::<ShellState>();
        state.services.cancel_selection();
        state.pending_selection = None;
    }
    match command {
        Command::ShowSettings => {
            if let Err(error) = show_settings(cx) {
                log::error!("Settings: {error}");
            }
        }
        Command::Quit => request_quit(cx),
        Command::ShowTranslator => {
            if let Err(error) = show_translator(cx) {
                log::error!("Translator: {error}");
            }
        }
        Command::ShowSearch => {
            if let Err(error) = show_search(cx) {
                log::error!("Search: {error}");
            }
        }
        Command::SelectText => {
            let state = cx.global_mut::<ShellState>();
            if state.pending_selection.is_none() {
                match state.services.capture_selection() {
                    Ok(id) => state.pending_selection = Some(id),
                    Err(error) => {
                        state.system.warning = Some(error);
                        let _ = show_settings(cx);
                    }
                }
            }
        }
        Command::Capture => {
            if let Err(error) = capture::begin(started, cx) {
                capture::report(error, cx);
            }
        }
        Command::Shortcut { .. } | Command::RecordedShortcut { .. } => {
            unreachable!("shortcut was resolved before dispatch")
        }
    }
}
