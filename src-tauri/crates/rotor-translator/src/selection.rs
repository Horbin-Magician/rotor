//! Simulates a copy keystroke (Ctrl+C / Cmd+C) so the currently selected
//! text in the foreground application lands on the clipboard.

use std::error::Error;

#[cfg(target_os = "windows")]
pub fn simulate_copy() -> Result<(), Box<dyn Error + Send + Sync>> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_C, VK_CONTROL,
    };

    wait_for_modifiers_release();

    let input = |vk: u16, key_up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: if key_up {
                    KEYEVENTF_KEYUP
                } else {
                    Default::default()
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    let inputs = [
        input(VK_CONTROL.0, false),
        input(VK_C.0, false),
        input(VK_C.0, true),
        input(VK_CONTROL.0, true),
    ];

    let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent as usize != inputs.len() {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(())
}

/// The global hotkey itself uses modifiers (e.g. Ctrl+Shift+D); while the
/// user is still physically holding them, a simulated Ctrl+C is delivered as
/// Ctrl+Shift+C (which opens DevTools in Chrome instead of copying). Wait
/// until all modifiers are released before injecting the copy keystroke.
#[cfg(target_os = "windows")]
fn wait_for_modifiers_release() {
    use std::time::{Duration, Instant};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT,
    };

    const WAIT_TIMEOUT: Duration = Duration::from_millis(1500);
    const POLL_INTERVAL: Duration = Duration::from_millis(30);

    let is_pressed = |vk: u16| unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 };

    let start = Instant::now();
    while start.elapsed() < WAIT_TIMEOUT {
        if !is_pressed(VK_SHIFT.0) && !is_pressed(VK_CONTROL.0) && !is_pressed(VK_MENU.0) {
            return;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(target_os = "macos")]
pub fn simulate_copy() -> Result<(), Box<dyn Error + Send + Sync>> {
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    // Virtual key code for the "C" key.
    const KEY_CODE_C: u16 = 8;

    if !request_accessibility_permission() {
        return Err("macOS Accessibility permission is required for selection translation".into());
    }

    wait_for_modifiers_release();

    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| std::io::Error::other("Failed to create CGEventSource"))?;

    let key_down = CGEvent::new_keyboard_event(source.clone(), KEY_CODE_C, true)
        .map_err(|_| std::io::Error::other("Failed to create Cmd+C key-down event"))?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(CGEventTapLocation::HID);

    let key_up = CGEvent::new_keyboard_event(source, KEY_CODE_C, false)
        .map_err(|_| std::io::Error::other("Failed to create Cmd+C key-up event"))?;
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.post(CGEventTapLocation::HID);

    Ok(())
}

#[cfg(target_os = "macos")]
fn request_accessibility_permission() -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::CFString;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    }

    let prompt_key = CFString::new("AXTrustedCheckOptionPrompt");
    let options = CFDictionary::from_CFType_pairs(&[(prompt_key, CFBoolean::true_value())]);

    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) }
}

#[cfg(target_os = "macos")]
fn wait_for_modifiers_release() {
    use core_graphics::event::{CGEvent, CGEventFlags};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
    use std::time::{Duration, Instant};

    const WAIT_TIMEOUT: Duration = Duration::from_millis(1500);
    const POLL_INTERVAL: Duration = Duration::from_millis(30);
    let modifier_flags = CGEventFlags::CGEventFlagShift
        | CGEventFlags::CGEventFlagControl
        | CGEventFlags::CGEventFlagAlternate
        | CGEventFlags::CGEventFlagCommand;

    let start = Instant::now();
    while start.elapsed() < WAIT_TIMEOUT {
        let modifiers_pressed = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
            .and_then(CGEvent::new)
            .is_ok_and(|event| event.get_flags().intersects(modifier_flags));

        if !modifiers_pressed {
            return;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(target_os = "macos")]
pub fn clipboard_change_count() -> Option<isize> {
    use objc2_app_kit::NSPasteboard;

    Some(unsafe { NSPasteboard::generalPasteboard().changeCount() })
}

#[cfg(not(target_os = "macos"))]
pub fn clipboard_change_count() -> Option<isize> {
    None
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn simulate_copy() -> Result<(), Box<dyn Error + Send + Sync>> {
    Err("Simulated copy is not supported on this platform".into())
}
