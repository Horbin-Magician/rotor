//! Selection capture runs entirely on a worker. Cancellation still restores a
//! supported clipboard snapshot before returning; it never owns a GUI handle.
use arboard::{Clipboard, ImageData};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

static SELECTION: Mutex<()> = Mutex::new(());

pub struct SelectedText {
    pub text: String,
    pub restore_warning: Option<String>,
}

enum Snapshot {
    Text(String),
    Image(ImageData<'static>),
    Unsupported,
}

fn optional<T>(result: Result<T, arboard::Error>) -> Result<Option<T>, String> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(arboard::Error::ContentNotAvailable) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn snapshot(
    clipboard: &mut Clipboard,
) -> Result<(Snapshot, Option<String>, Option<isize>), String> {
    for _ in 0..3 {
        let before = crate::selection::clipboard_change_count();
        let text = optional(clipboard.get_text())?;
        let backup = if let Some(text) = &text {
            Snapshot::Text(text.clone())
        } else if let Some(image) = optional(clipboard.get_image())? {
            Snapshot::Image(image)
        } else {
            Snapshot::Unsupported
        };
        let after = crate::selection::clipboard_change_count();
        if before == after {
            return Ok((backup, text, after));
        }
    }
    Err("Clipboard kept changing before selection capture".into())
}

fn changed(
    before: Option<isize>,
    after: Option<isize>,
    old: &Option<String>,
    new: &Option<String>,
) -> bool {
    match (before, after) {
        (Some(before), Some(after)) => before != after,
        _ => new.is_some() && new != old,
    }
}

#[cfg(any(target_os = "windows", target_os = "macos"))]
pub fn capture_selected_text(cancelled: impl Fn() -> bool) -> Result<SelectedText, String> {
    let _guard = SELECTION.lock().unwrap_or_else(|error| error.into_inner());
    let check_cancelled = || {
        if cancelled() {
            Err("Selection capture cancelled".to_string())
        } else {
            Ok(())
        }
    };
    check_cancelled()?;
    crate::selection::wait_for_modifiers_release().map_err(|error| error.to_string())?;
    check_cancelled()?;
    let mut clipboard = Clipboard::new().map_err(|error| error.to_string())?;
    let (backup, previous, before) = snapshot(&mut clipboard)?;
    check_cancelled()?;
    crate::selection::simulate_copy_if(&cancelled).map_err(|error| error.to_string())?;

    let deadline = Instant::now() + Duration::from_millis(700);
    let mut captured = None;
    let mut observed = None;
    let mut sequence = before;
    let mut read_error = None;
    loop {
        let current_sequence = crate::selection::clipboard_change_count();
        match optional(clipboard.get_text()) {
            Ok(current) => {
                if current_sequence == crate::selection::clipboard_change_count()
                    && changed(before, current_sequence, &previous, &current)
                {
                    captured = current.clone();
                    observed = current;
                    sequence = current_sequence;
                    break;
                }
            }
            Err(error) => read_error = Some(error),
        }
        // A copy already sent must get its bounded restore opportunity even
        // when the UI cancels while the source app is processing Ctrl/Cmd+C.
        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    // Do not overwrite a later clipboard change by the user or another app.
    let mut restore_warning = None;
    if sequence != before || observed.is_some() {
        if crate::selection::clipboard_change_count() == sequence
            && optional(clipboard.get_text()).is_ok_and(|current| current == observed)
        {
            let restore = match backup {
                Snapshot::Text(text) => clipboard.set_text(text),
                Snapshot::Image(image) => clipboard.set_image(image),
                Snapshot::Unsupported => Ok(()),
            };
            if let Err(error) = restore {
                restore_warning = Some(format!("Clipboard restore failed: {error}"));
            }
        } else {
            restore_warning =
                Some("Clipboard changed again; previous content was not restored".into());
        }
    }
    check_cancelled()?;
    let text = captured
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .ok_or_else(|| read_error.unwrap_or_else(|| "No selected text was copied".into()))?;
    Ok(SelectedText {
        text,
        restore_warning,
    })
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn capture_selected_text(_: impl Fn() -> bool) -> Result<SelectedText, String> {
    Err("Selection capture is not supported on this platform".into())
}

#[cfg(test)]
mod tests {
    use super::changed;
    #[test]
    fn copy_of_identical_text_requires_a_new_sequence() {
        let text = Some("same".to_owned());
        assert!(changed(Some(7), Some(8), &text, &text));
        assert!(!changed(Some(7), Some(7), &text, &text));
        assert!(!changed(None, None, &text, &text));
        assert!(changed(None, None, &text, &Some("new".into())));
    }

    #[test]
    fn pre_cancelled_capture_never_accesses_the_clipboard() {
        let error = super::capture_selected_text(|| true).err().unwrap();
        assert_eq!(error, "Selection capture cancelled");
    }
}
