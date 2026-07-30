pub mod engine;
mod selection;

use std::error::Error;
use std::str::FromStr;
use std::thread;
use std::time::Duration;

use tauri::{Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::Shortcut;

use rotor_common::AppConfig;

const WINDOW_LABEL: &str = "translator";
const CLIPBOARD_POLL_INTERVAL: Duration = Duration::from_millis(50);
const CLIPBOARD_WAIT_TIMEOUT: Duration = Duration::from_millis(1000);
const CURSOR_OFFSET: i32 = 16;

pub struct Translator {
    app_handler: Option<tauri::AppHandle>,
}

impl Translator {
    pub fn new() -> Translator {
        Translator { app_handler: None }
    }

    pub fn flag(&self) -> &str {
        "translator"
    }

    pub fn init(&mut self, app: &tauri::AppHandle) -> Result<(), Box<dyn Error>> {
        self.app_handler = Some(app.clone());
        self.build_window()?;
        Ok(())
    }

    pub fn get_select_shortcut(&self) -> Option<Shortcut> {
        Self::read_shortcut("shortcut_translate_select")
    }

    pub fn get_input_shortcut(&self) -> Option<Shortcut> {
        Self::read_shortcut("shortcut_translate_input")
    }

    fn read_shortcut(key: &str) -> Option<Shortcut> {
        let app_config = AppConfig::lock_global();
        let shortcut = app_config.get(key).cloned();
        drop(app_config);

        if let Some(shortcut_str) = shortcut {
            match Shortcut::from_str(&shortcut_str) {
                Ok(shortcut) => return Some(shortcut),
                Err(error) => {
                    log::warn!("Invalid shortcut `{shortcut_str}` for {key}: {error}");
                    return None;
                }
            }
        }
        None
    }

    /// Selection translation: capture the selected text via a simulated copy,
    /// then show the translator window near the cursor with that text.
    pub fn run_select(&self) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_handler {
            Some(handle) => handle.clone(),
            None => return Err("AppHandle not initialized".into()),
        };

        // The copy needs time to land on the clipboard, so do the whole
        // capture off the hotkey dispatch thread.
        thread::Builder::new()
            .name("rotor-translate-select".to_string())
            .spawn(move || {
                match capture_selected_text(&app_handle) {
                    Ok(Some(text)) => {
                        if let Err(e) = show_window(&app_handle, Some(text)) {
                            log::error!("Translator show window error: {e}");
                        }
                    }
                    Ok(None) => log::debug!("Translator: no selected text captured"),
                    Err(e) => log::error!("Translator capture selection error: {e}"),
                }
            })?;

        Ok(())
    }

    /// Input translation: show the translator window near the cursor with an
    /// empty, focused input box.
    pub fn run_input(&self) -> Result<(), Box<dyn Error>> {
        let app_handle = match &self.app_handler {
            Some(handle) => handle.clone(),
            None => return Err("AppHandle not initialized".into()),
        };

        show_window(&app_handle, None)
    }

    fn build_window(&self) -> Result<(), Box<dyn Error>> {
        if let Some(ref app) = self.app_handler {
            let mut win_builder = WebviewWindowBuilder::new(
                app,
                WINDOW_LABEL,
                WebviewUrl::App("Translator".into()),
            )
            .always_on_top(true)
            .resizable(false)
            .visible(false);

            #[cfg(target_os = "windows")]
            {
                win_builder = win_builder.decorations(false).skip_taskbar(true);
            }

            #[cfg(target_os = "macos")]
            {
                win_builder = win_builder
                    .hidden_title(true)
                    .title_bar_style(tauri::TitleBarStyle::Overlay)
                    .traffic_light_position(tauri::LogicalPosition { x: (0), y: (-100) });
            }

            let _window = win_builder.build()?;
            Ok(())
        } else {
            Err("AppHandle not initialized".into())
        }
    }
}

fn capture_selected_text(
    app: &tauri::AppHandle,
) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
    let clipboard = app.clipboard();
    let previous_text = clipboard.read_text().ok();
    let previous_change_count = selection::clipboard_change_count();

    selection::simulate_copy()?;

    // Poll until the clipboard content changes; large selections and some
    // applications (e.g. browsers) take a while to land on the clipboard.
    // macOS change counts also detect a successful copy when the selected
    // text happens to equal the previous clipboard text.
    let mut captured = None;
    let wait_start = std::time::Instant::now();
    while wait_start.elapsed() < CLIPBOARD_WAIT_TIMEOUT {
        thread::sleep(CLIPBOARD_POLL_INTERVAL);
        let current = clipboard.read_text().ok();
        if clipboard_has_changed(
            &previous_text,
            &current,
            previous_change_count,
            selection::clipboard_change_count(),
        ) {
            captured = current;
            break;
        }
    }

    // Restore whatever was on the clipboard before the simulated copy.
    if let Some(previous) = previous_text {
        if let Err(e) = clipboard.write_text(previous) {
            log::warn!("Translator: failed to restore clipboard text: {e}");
        }
    }

    let text = captured
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());

    Ok(text)
}

fn clipboard_has_changed(
    previous_text: &Option<String>,
    current_text: &Option<String>,
    previous_change_count: Option<isize>,
    current_change_count: Option<isize>,
) -> bool {
    match (previous_change_count, current_change_count) {
        (Some(previous), Some(current)) => current != previous,
        _ => current_text.is_some() && current_text != previous_text,
    }
}

fn show_window(app: &tauri::AppHandle, selected_text: Option<String>) -> Result<(), Box<dyn Error>> {
    let window = app
        .get_webview_window(WINDOW_LABEL)
        .ok_or("Translator window not found")?;

    position_near_cursor(app, &window);

    match selected_text {
        Some(text) => app.emit_to(WINDOW_LABEL, "translate-select", text)?,
        None => app.emit_to(WINDOW_LABEL, "translate-input", ())?,
    }

    window.show()?;
    window.set_focus()?;
    Ok(())
}

fn position_near_cursor(app: &tauri::AppHandle, window: &WebviewWindow) {
    let Ok((cursor_x, cursor_y)) = rotor_platform::sys_util::get_cursor_position() else {
        return;
    };

    let window_size = window
        .outer_size()
        .unwrap_or_else(|_| tauri::PhysicalSize::new(560, 220));

    let mut x = cursor_x + CURSOR_OFFSET;
    let mut y = cursor_y + CURSOR_OFFSET;

    // Clamp the window inside the monitor under the cursor.
    if let Ok(Some(monitor)) = app.monitor_from_point(cursor_x as f64, cursor_y as f64) {
        let monitor_pos = monitor.position();
        let monitor_size = monitor.size();
        let max_x = monitor_pos.x + monitor_size.width as i32 - window_size.width as i32;
        let max_y = monitor_pos.y + monitor_size.height as i32 - window_size.height as i32;
        x = x.clamp(monitor_pos.x, max_x.max(monitor_pos.x));
        y = y.clamp(monitor_pos.y, max_y.max(monitor_pos.y));
    }

    if let Err(e) = window.set_position(PhysicalPosition::new(x, y)) {
        log::warn!("Translator: failed to position window: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::clipboard_has_changed;

    #[test]
    fn change_count_detects_copy_of_identical_text() {
        let text = Some("same text".to_string());

        assert!(clipboard_has_changed(&text, &text, Some(10), Some(11)));
    }

    #[test]
    fn text_difference_is_used_without_change_counts() {
        let previous = Some("before".to_string());
        let current = Some("after".to_string());

        assert!(clipboard_has_changed(&previous, &current, None, None));
        assert!(!clipboard_has_changed(&previous, &previous, None, None));
    }
}
