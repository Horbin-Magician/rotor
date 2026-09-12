use super::WindowRect;
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

pub(super) fn window_rectangles() -> Result<Vec<WindowRect>, String> {
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
    // Preserve the existing selection filters, including the recording indicator
    // and windows that the OS does not allow to be shared.
    let name = value(window, "kCGWindowName")?.downcast::<CFString>()?;
    let owner = value(window, "kCGWindowOwnerName")?.downcast::<CFString>()?;
    if name == "StatusIndicator" && owner == "Window Server" {
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
        let bounds = CFDictionary::from_CFType_pairs(&[
            (CFString::new("X"), CFNumber::from(x)),
            (CFString::new("Y"), CFNumber::from(-20.)),
            (CFString::new("Width"), CFNumber::from(800.)),
            (CFString::new("Height"), CFNumber::from(600.)),
        ]);
        CFDictionary::from_CFType_pairs(&[
            (
                CFString::new("kCGWindowName"),
                CFString::new(name).as_CFType(),
            ),
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
        ])
        .into_untyped()
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
}
