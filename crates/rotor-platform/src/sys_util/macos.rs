use super::{MemoryUsage, PermissionStatus, WindowRect};
use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};
use core_graphics::window::{
    copy_window_info, kCGNullWindowID, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionOnScreenOnly,
};

pub(super) fn get_all_window_rect() -> Result<Vec<WindowRect>, Box<dyn std::error::Error>> {
    window_rectangles().map_err(Into::into)
}

pub(super) fn get_cursor_position() -> Result<(i32, i32), Box<dyn std::error::Error>> {
    use core_graphics::event::CGEvent;
    use core_graphics::event_source::CGEventSource;
    use core_graphics::event_source::CGEventSourceStateID;

    // Create a CGEvent using a default event source to get the current cursor position
    if let Ok(event_source) = CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
        if let Ok(event) = CGEvent::new(event_source) {
            let location = event.location();
            return Ok((location.x as i32, location.y as i32));
        }
    }
    Err("Failed to get cursor position".into())
}

pub(super) fn get_memory_usage() -> Result<MemoryUsage, Box<dyn std::error::Error>> {
    let mut task_info = std::mem::MaybeUninit::<libc::proc_taskinfo>::uninit();
    let info_size = std::mem::size_of::<libc::proc_taskinfo>() as i32;
    let result = unsafe {
        libc::proc_pidinfo(
            std::process::id() as i32,
            libc::PROC_PIDTASKINFO,
            0,
            task_info.as_mut_ptr() as *mut libc::c_void,
            info_size,
        )
    };

    if result != info_size {
        return Err(std::io::Error::last_os_error().into());
    }

    let task_info = unsafe { task_info.assume_init() };
    Ok(MemoryUsage {
        resident_bytes: task_info.pti_resident_size,
    })
}

pub(super) fn get_permission_statuses() -> Vec<PermissionStatus> {
    vec![
        PermissionStatus {
            key: "accessibility".into(),
            name: "Accessibility".into(),
            granted: Some(crate::selection::accessibility_permission()),
            detail: "Required for selection translation".into(),
        },
        PermissionStatus {
            key: "screen_capture".to_string(),
            name: "Screen Capture".to_string(),
            granted: check_macos_screen_capture_permission(),
            detail: "Required for screenshot capture".to_string(),
        },
        PermissionStatus {
            key: "file_search".to_string(),
            name: "File Search".to_string(),
            granted: Some(true),
            detail: "Uses the current user's readable folders".to_string(),
        },
    ]
}

fn check_macos_screen_capture_permission() -> Option<bool> {
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
    }

    Some(unsafe { CGPreflightScreenCaptureAccess() })
}

fn window_rectangles() -> Result<Vec<WindowRect>, String> {
    let windows = copy_window_info(
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
        kCGNullWindowID,
    )
    .ok_or("Could not enumerate on-screen windows")?;
    rectangles_from_snapshot(&windows)
}

fn rectangles_from_snapshot(windows: &CFArray) -> Result<Vec<WindowRect>, String> {
    let count = i32::try_from(windows.len()).map_err(|_| "Too many on-screen windows")?;
    let mut rectangles = Vec::new();
    for (index, value) in windows.iter().enumerate() {
        if value.is_null() {
            continue;
        }
        // Quartz owns CF objects in this array. Retain before checking their
        // concrete type, so malformed entries can be skipped without casting.
        let value = unsafe { CFType::wrap_under_get_rule(*value) };
        let Some(window) = value.downcast::<CFDictionary>() else {
            continue;
        };
        // Quartz orders windows front to back. Larger z wins in auto-selection.
        // Use the original snapshot index even when intermediate entries fail.
        if let Some(rect) = window_rect(&window, count - index as i32 - 1) {
            rectangles.push(rect);
        }
    }
    Ok(rectangles)
}

fn value(dictionary: &CFDictionary, key: &str) -> Option<CFType> {
    let key = CFString::new(key);
    let raw = *dictionary.find(key.as_concrete_TypeRef().cast::<std::ffi::c_void>())?;
    if raw.is_null() {
        return None;
    }
    // Keys and values in Quartz window dictionaries are Core Foundation objects.
    Some(unsafe { CFType::wrap_under_get_rule(raw) })
}

fn number(dictionary: &CFDictionary, key: &str) -> Option<CFNumber> {
    value(dictionary, key)?.downcast::<CFNumber>()
}

fn window_rect(window: &CFDictionary, z: i32) -> Option<WindowRect> {
    // Skip windows owned by the current process (mask/pin windows).
    if number(window, "kCGWindowOwnerPID")
        .and_then(|pid| pid.to_i64())
        .is_some_and(|pid| pid == i64::from(std::process::id()))
    {
        return None;
    }
    // Preserve the existing selection filters, including the recording indicator
    // and windows that the OS does not allow to be shared. kCGWindowName is
    // optional; untitled windows are still selectable.
    let name = value(window, "kCGWindowName").and_then(|name| name.downcast::<CFString>());
    let owner = value(window, "kCGWindowOwnerName")?.downcast::<CFString>()?;
    if owner == "Window Server" && name.is_some_and(|name| name == "StatusIndicator") {
        return None;
    }
    if number(window, "kCGWindowSharingState")?.to_i64()? == 0 {
        return None;
    }
    number(window, "kCGWindowNumber")?.to_i64()?;
    let bounds = value(window, "kCGWindowBounds")?.downcast::<CFDictionary>()?;
    let x = number(&bounds, "X")?.to_f64()?;
    let y = number(&bounds, "Y")?.to_f64()?;
    let width = number(&bounds, "Width")?.to_f64()?;
    let height = number(&bounds, "Height")?.to_f64()?;
    if [x, y]
        .iter()
        .any(|v| !v.is_finite() || *v < i32::MIN as f64 || *v > i32::MAX as f64)
        || [width, height]
            .iter()
            .any(|v| !v.is_finite() || *v < 1. || *v > u32::MAX as f64)
    {
        return None;
    }
    // Keep Quartz points. Each capture later applies its own display scale.
    Some((x as i32, y as i32, z, width as u32, height as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(name: &str, owner: &str, sharing: i32, x: f64) -> CFDictionary {
        window_entry(Some(name), owner, sharing, x, None)
    }

    fn window_entry(
        name: Option<&str>,
        owner: &str,
        sharing: i32,
        x: f64,
        pid: Option<i64>,
    ) -> CFDictionary {
        let bounds = CFDictionary::from_CFType_pairs(&[
            (CFString::new("X"), CFNumber::from(x)),
            (CFString::new("Y"), CFNumber::from(-20.)),
            (CFString::new("Width"), CFNumber::from(800.)),
            (CFString::new("Height"), CFNumber::from(600.)),
        ]);
        let mut pairs = vec![
            (
                CFString::new("kCGWindowOwnerName"),
                CFString::new(owner).as_CFType(),
            ),
            (
                CFString::new("kCGWindowSharingState"),
                CFNumber::from(sharing).as_CFType(),
            ),
            (
                CFString::new("kCGWindowNumber"),
                CFNumber::from(7).as_CFType(),
            ),
            (CFString::new("kCGWindowBounds"), bounds.as_CFType()),
        ];
        if let Some(name) = name {
            pairs.push((
                CFString::new("kCGWindowName"),
                CFString::new(name).as_CFType(),
            ));
        }
        if let Some(pid) = pid {
            pairs.push((
                CFString::new("kCGWindowOwnerPID"),
                CFNumber::from(pid).as_CFType(),
            ));
        }
        CFDictionary::from_CFType_pairs(&pairs).into_untyped()
    }

    #[test]
    fn snapshot_preserves_point_coordinates_and_front_to_back_order() {
        let windows = CFArray::from_CFTypes(&[
            window("Front", "App", 1, -400.).as_CFType(),
            CFString::new("malformed entry").as_CFType(),
            window("Back", "App", 1, 0.).as_CFType(),
        ])
        .into_untyped();
        assert_eq!(
            rectangles_from_snapshot(&windows).unwrap(),
            [(-400, -20, 2, 800, 600), (0, -20, 0, 800, 600),]
        );
    }

    #[test]
    fn indicator_private_and_invalid_windows_are_skipped() {
        let windows = CFArray::from_CFTypes(&[
            window("StatusIndicator", "Window Server", 1, 0.),
            window("Private", "App", 0, 0.),
            window("Invalid", "App", 1, f64::NAN),
            CFDictionary::from_CFType_pairs(&[(
                CFString::new("unrelated"),
                CFString::new("value"),
            )])
            .into_untyped(),
            window("Valid", "App", 1, 30.),
        ])
        .into_untyped();
        assert_eq!(
            rectangles_from_snapshot(&windows).unwrap(),
            [(30, -20, 0, 800, 600)]
        );
    }

    #[test]
    fn untitled_windows_are_kept_and_own_process_windows_are_skipped() {
        let own = i64::from(std::process::id());
        let windows = CFArray::from_CFTypes(&[
            window_entry(Some("Mask"), "Rotor", 1, 0., Some(own)),
            window_entry(None, "Rotor", 1, 0., Some(own)),
            window_entry(None, "App", 1, 10., Some(own + 1)),
            window_entry(None, "Window Server", 1, 20., None),
        ])
        .into_untyped();
        assert_eq!(
            rectangles_from_snapshot(&windows).unwrap(),
            [(10, -20, 1, 800, 600), (20, -20, 0, 800, 600)]
        );
    }
}
