/// Give a hidden pin a taskbar entry while preserving its borderless, topmost style.
#[cfg(target_os = "windows")]
pub fn enable_pin_taskbar(handle: raw_window_handle::WindowHandle<'_>) -> Result<(), String> {
    use raw_window_handle::RawWindowHandle;
    use windows::Win32::{
        Foundation::{GetLastError, SetLastError, HWND, WIN32_ERROR},
        UI::WindowsAndMessaging::{
            GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, GWL_STYLE, WS_EX_APPWINDOW,
            WS_EX_TOOLWINDOW, WS_MINIMIZEBOX, WS_SYSMENU,
        },
    };
    let RawWindowHandle::Win32(raw) = handle.as_raw() else {
        return Err("Expected a Windows window handle".into());
    };
    let hwnd = HWND(raw.hwnd.get() as *mut _);
    // Called on the owning UI thread before the window is first shown.
    unsafe {
        for (index, remove, add) in [
            (GWL_EXSTYLE, WS_EX_TOOLWINDOW.0, WS_EX_APPWINDOW.0),
            (GWL_STYLE, 0, WS_MINIMIZEBOX.0 | WS_SYSMENU.0),
        ] {
            let style = GetWindowLongW(hwnd, index) as u32;
            SetLastError(WIN32_ERROR(0));
            if SetWindowLongW(hwnd, index, ((style & !remove) | add) as i32) == 0 {
                let error = GetLastError();
                if error != WIN32_ERROR(0) {
                    return Err(std::io::Error::from_raw_os_error(error.0 as i32).to_string());
                }
            }
        }
    }
    Ok(())
}

/// Enable minimization while keeping GPUI's titlebar-less pin panels undecorated.
#[cfg(target_os = "macos")]
pub fn enable_pin_minimization(handle: raw_window_handle::WindowHandle<'_>) -> Result<(), String> {
    use objc2_app_kit::{NSWindowButton, NSWindowStyleMask};
    use raw_window_handle::RawWindowHandle;

    let RawWindowHandle::AppKit(raw) = handle.as_raw() else {
        return Err("Expected an AppKit window".into());
    };
    // Called on the owning UI thread before the pin is first shown. GPUI
    // ignores is_minimizable when titlebar is None; retain all other flags.
    let view = unsafe { &*raw.ns_view.as_ptr().cast::<objc2_app_kit::NSView>() };
    let window = view.window().ok_or("View is not attached to a window")?;
    window.setStyleMask(window.styleMask() | NSWindowStyleMask::Miniaturizable);
    // AppKit can recreate the traffic lights when the style mask changes.
    // Hide them afterwards; the pin toolbar owns its window controls.
    for kind in [
        NSWindowButton::NSWindowCloseButton,
        NSWindowButton::NSWindowMiniaturizeButton,
        NSWindowButton::NSWindowZoomButton,
    ] {
        if let Some(button) = window.standardWindowButton(kind) {
            button.setHidden(true);
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn window_minimized(handle: raw_window_handle::WindowHandle<'_>) -> Option<bool> {
    use raw_window_handle::RawWindowHandle;

    let RawWindowHandle::AppKit(raw) = handle.as_raw() else {
        return None;
    };
    // The borrowed handle refers to a live NSView on the owning UI thread.
    let view = unsafe { &*raw.ns_view.as_ptr().cast::<objc2_app_kit::NSView>() };
    let window = view.window()?;
    // Miniaturized windows are not visible. Only an unshown, non-miniaturized
    // window should retain its saved state during initialization.
    let minimized = window.isMiniaturized();
    (minimized || window.isVisible()).then_some(minimized)
}

#[cfg(target_os = "windows")]
pub fn window_minimized(handle: raw_window_handle::WindowHandle<'_>) -> Option<bool> {
    use raw_window_handle::RawWindowHandle;
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{IsIconic, IsWindowVisible},
    };
    let RawWindowHandle::Win32(raw) = handle.as_raw() else {
        return None;
    };
    let hwnd = HWND(raw.hwnd.get() as *mut _);
    // Hidden windows are still being initialized; retain their saved state.
    unsafe {
        IsWindowVisible(hwnd)
            .as_bool()
            .then(|| IsIconic(hwnd).as_bool())
    }
}

/// Keep capture overlays rectangular even when DWM rounds ordinary windows.
#[cfg(target_os = "windows")]
pub fn disable_window_rounding(handle: raw_window_handle::WindowHandle<'_>) -> Result<(), String> {
    set_window_corner_preference(handle, windows::Win32::Graphics::Dwm::DWMWCP_DONOTROUND)
}

/// Use DWM's smaller corner radius for compact floating windows such as pins.
#[cfg(target_os = "windows")]
pub fn use_small_window_corners(handle: raw_window_handle::WindowHandle<'_>) -> Result<(), String> {
    set_window_corner_preference(handle, windows::Win32::Graphics::Dwm::DWMWCP_ROUNDSMALL)
}

#[cfg(target_os = "windows")]
fn set_window_corner_preference(
    handle: raw_window_handle::WindowHandle<'_>,
    preference: windows::Win32::Graphics::Dwm::DWM_WINDOW_CORNER_PREFERENCE,
) -> Result<(), String> {
    use raw_window_handle::RawWindowHandle;
    use windows::Win32::{
        Foundation::{E_INVALIDARG, HWND},
        Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE},
    };
    let RawWindowHandle::Win32(raw) = handle.as_raw() else {
        return Err("Expected a Windows window handle".into());
    };
    // The borrowed handle keeps the native window alive for this synchronous call.
    let result = unsafe {
        DwmSetWindowAttribute(
            HWND(raw.hwnd.get() as *mut _),
            DWMWA_WINDOW_CORNER_PREFERENCE,
            std::ptr::from_ref(&preference).cast(),
            std::mem::size_of_val(&preference) as u32,
        )
    };
    match result {
        // Windows 10 does not support this attribute and already uses square corners.
        Err(error) if error.code() == E_INVALIDARG => Ok(()),
        result => result.map_err(|error| error.to_string()),
    }
}

pub fn settle_desktop() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    unsafe { windows::Win32::Graphics::Dwm::DwmFlush() }.map_err(|error| error.to_string())?;
    Ok(())
}

/// Activate a visible overlay without restoring or changing its calibrated bounds.
#[cfg(target_os = "windows")]
pub fn activate_window_in_place(handle: raw_window_handle::WindowHandle<'_>) -> Result<(), String> {
    use raw_window_handle::RawWindowHandle;
    use windows::Win32::{
        Foundation::HWND,
        UI::{
            Input::KeyboardAndMouse::{
                SendInput, SetActiveWindow, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
                KEYEVENTF_KEYUP, VK_MENU,
            },
            WindowsAndMessaging::SetForegroundWindow,
        },
    };
    let RawWindowHandle::Win32(raw) = handle.as_raw() else {
        return Err("Expected a Windows window handle".into());
    };
    let hwnd = HWND(raw.hwnd.get() as *mut _);
    // These calls run on the owning UI thread with a borrowed live handle.
    // The return handles describe the previous focus, which may legitimately be null.
    unsafe {
        let _ = SetActiveWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
        if !SetForegroundWindow(hwnd).as_bool() {
            // Retain GPUI's foreground activation fallback, but never apply its
            // cached initial placement after the capture client has been fitted.
            let inputs = [
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_MENU,
                            ..Default::default()
                        },
                    },
                },
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: VK_MENU,
                            dwFlags: KEYEVENTF_KEYUP,
                            ..Default::default()
                        },
                    },
                },
            ];
            SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
            if !SetForegroundWindow(hwnd).as_bool() {
                log::warn!("Windows declined capture overlay foreground activation");
            }
        }
    }
    Ok(())
}

/// Synchronously service a hidden window's paint on its owning UI thread.
/// Call outside any UI framework borrow: WM_PAINT re-enters the window's
/// renderer. The caller obtains the raw handle immediately before this call
/// and must keep the owning window alive until it returns.
#[cfg(target_os = "windows")]
pub fn paint_hidden_window(handle: raw_window_handle::RawWindowHandle) -> Result<(), String> {
    use raw_window_handle::RawWindowHandle;
    use windows::Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        System::Threading::{GetCurrentProcessId, GetCurrentThreadId},
        UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindow, SendMessageW, WM_PAINT},
    };
    let RawWindowHandle::Win32(raw) = handle else {
        return Err("Expected a Windows window".into());
    };
    let hwnd = HWND(raw.hwnd.get() as *mut _);
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return Err("Capture window is no longer available".into());
        }
        let mut process = 0;
        if GetWindowThreadProcessId(hwnd, Some(&mut process)) != GetCurrentThreadId()
            || process != GetCurrentProcessId()
        {
            return Err("Hidden paint must run on the owning UI thread".into());
        }
        // This is deliberately a direct call to our known GPUI paint handler,
        // not general-purpose Windows invalidation. RedrawWindow/UpdateWindow
        // do not service hidden windows (covered by a hidden native fixture).
        // GPUI 0.3.3 handles WM_PAINT by drawing its dirty scene even without
        // a visible update region. There are no next-frame callbacks here.
        SendMessageW(hwnd, WM_PAINT, Some(WPARAM(0)), Some(LPARAM(0)));
    }
    Ok(())
}

#[cfg(all(test, target_os = "windows"))]
mod repaint_tests {
    use super::*;
    use std::cell::Cell;
    use windows::{
        core::w,
        Win32::{
            Foundation::{HWND, LPARAM, LRESULT, WPARAM},
            UI::WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, IsWindowVisible, RegisterClassW,
                UnregisterClassW, WINDOW_EX_STYLE, WM_PAINT, WNDCLASSW, WS_POPUP,
            },
        },
    };

    thread_local! { static PAINTS: Cell<u32> = const { Cell::new(0) }; }

    unsafe extern "system" fn fixture(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
        if message == WM_PAINT {
            PAINTS.set(PAINTS.get() + 1);
        }
        unsafe { DefWindowProcW(hwnd, message, w, l) }
    }

    #[test]
    fn pin_taskbar_preserves_topmost_and_tracks_minimization() {
        use windows::Win32::UI::WindowsAndMessaging::*;
        unsafe {
            let hwnd = CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                w!("STATIC"),
                w!("Rotor synthetic pin"),
                WS_POPUP,
                0,
                0,
                32,
                32,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            let result = std::panic::catch_unwind(|| {
                let raw = raw_window_handle::Win32WindowHandle::new(
                    std::num::NonZeroIsize::new(hwnd.0 as isize).unwrap(),
                );
                let handle = raw_window_handle::WindowHandle::borrow_raw(
                    raw_window_handle::RawWindowHandle::Win32(raw),
                );
                enable_pin_taskbar(handle).unwrap();
                let style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
                assert_eq!(style & WS_EX_TOOLWINDOW.0, 0);
                assert_ne!(style & WS_EX_APPWINDOW.0, 0);
                assert_ne!(style & WS_EX_TOPMOST.0, 0);
                assert_eq!(window_minimized(handle), None);
                let _ = ShowWindow(hwnd, SW_SHOWMINNOACTIVE);
                assert_eq!(window_minimized(handle), Some(true));
                let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                assert_eq!(window_minimized(handle), Some(false));
            });
            DestroyWindow(hwnd).unwrap();
            result.unwrap();
        }
    }

    #[test]
    fn hidden_window_paint_is_serviced_synchronously_without_showing_it() {
        unsafe {
            let class = w!("RotorSyntheticHiddenPaintTest");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(fixture),
                lpszClassName: class,
                ..Default::default()
            };
            assert_ne!(RegisterClassW(&wc), 0);
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class,
                w!(""),
                WS_POPUP,
                0,
                0,
                2,
                2,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            let result = std::panic::catch_unwind(|| {
                let raw = raw_window_handle::Win32WindowHandle::new(
                    std::num::NonZeroIsize::new(hwnd.0 as isize).unwrap(),
                );
                let before = PAINTS.get();
                assert!(!IsWindowVisible(hwnd).as_bool());
                paint_hidden_window(raw_window_handle::RawWindowHandle::Win32(raw)).unwrap();
                assert!(PAINTS.get() > before);
                assert!(!IsWindowVisible(hwnd).as_bool());
            });
            DestroyWindow(hwnd).unwrap();
            UnregisterClassW(class, None).unwrap();
            result.unwrap();
        }
    }
}

pub fn pointer_capture(
    handle: raw_window_handle::WindowHandle<'_>,
    capture: bool,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::RawWindowHandle;
        use windows::Win32::{
            Foundation::HWND,
            UI::Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture},
        };
        let RawWindowHandle::Win32(raw) = handle.as_raw() else {
            return Err("Expected a Windows window".into());
        };
        let hwnd = HWND(raw.hwnd.get() as *mut _);
        unsafe {
            if capture {
                SetCapture(hwnd);
                if GetCapture() != hwnd {
                    return Err("Could not capture the pin pointer".into());
                }
            } else if GetCapture() == hwnd {
                ReleaseCapture().map_err(|error| error.to_string())?;
            }
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (handle, capture);
    }
    Ok(())
}

pub fn set_client_bounds(
    handle: raw_window_handle::WindowHandle<'_>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f32,
) -> Result<(), String> {
    if !scale.is_finite() || scale <= 0. || width == 0 || height == 0 {
        return Err("Invalid pin window bounds".into());
    }
    #[cfg(target_os = "windows")]
    {
        fit_client_bounds(handle, x, y, width, height)
    }
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::RawWindowHandle;
        let RawWindowHandle::AppKit(raw) = handle.as_raw() else {
            return Err("Expected an AppKit window".into());
        };
        let view = unsafe { &*raw.ns_view.as_ptr().cast::<objc2_app_kit::NSView>() };
        let window = view.window().ok_or("View is not attached to a window")?;
        let client = window.convertRectToScreen(view.convertRect_toView(view.bounds(), None));
        let mut frame = window.frame();
        let primary_height = core_graphics::display::CGDisplay::main()
            .bounds()
            .size
            .height;
        let width = width as f64 / scale as f64;
        let height = height as f64 / scale as f64;
        frame.origin.x += x as f64 / scale as f64 - client.origin.x;
        frame.origin.y += primary_height - y as f64 / scale as f64 - height - client.origin.y;
        frame.size.width += width - client.size.width;
        frame.size.height += height - client.size.height;
        window.setFrame_display(frame, true);
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (handle, x, y);
        Err("Pin window resizing is unavailable".into())
    }
}

/// Fit a capture mask to the entire display, including the menu bar and Dock.
#[cfg(target_os = "macos")]
pub fn fit_capture_screen(
    handle: raw_window_handle::WindowHandle<'_>,
    display_id: u32,
) -> Result<(), String> {
    use core_graphics::display::CGDisplay;
    use objc2_app_kit::{NSScreenSaverWindowLevel, NSWindowStyleMask};
    use objc2_foundation::{NSPoint, NSRect, NSSize};
    use raw_window_handle::RawWindowHandle;

    let RawWindowHandle::AppKit(raw) = handle.as_raw() else {
        return Err("Expected an AppKit window".into());
    };
    let bounds = CGDisplay::new(display_id).bounds();
    if bounds.size.width <= 0. || bounds.size.height <= 0. {
        return Err("Captured display is no longer available".into());
    }
    // The borrowed handle refers to a live NSView on the owning UI thread.
    let view = unsafe { &*raw.ns_view.as_ptr().cast::<objc2_app_kit::NSView>() };
    let window = view.window().ok_or("View is not attached to a window")?;
    // GPUI's titlebar: None still uses Titled | FullSizeContentView on macOS.
    // Remove those flags so AppKit does not constrain the mask below the menu
    // bar. Preserve the popup's nonactivating panel behavior.
    window.setStyleMask(
        window.styleMask() & !(NSWindowStyleMask::Titled | NSWindowStyleMask::FullSizeContentView),
    );
    window.setLevel(NSScreenSaverWindowLevel);
    // Quartz display bounds are global logical points with a top-left origin;
    // AppKit uses a bottom-left origin relative to the primary display.
    let primary_height = CGDisplay::main().bounds().size.height;
    window.setFrame_display(
        NSRect::new(
            NSPoint::new(
                bounds.origin.x,
                primary_height - bounds.origin.y - bounds.size.height,
            ),
            NSSize::new(bounds.size.width, bounds.size.height),
        ),
        true,
    );
    Ok(())
}

pub fn hide_window(handle: raw_window_handle::WindowHandle<'_>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::RawWindowHandle;
        use windows::Win32::{
            Foundation::HWND,
            UI::WindowsAndMessaging::{IsWindowVisible, ShowWindow, SW_HIDE},
        };
        let RawWindowHandle::Win32(raw) = handle.as_raw() else {
            return Err("Expected a Windows window".into());
        };
        let hwnd = HWND(raw.hwnd.get() as *mut _);
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
            if IsWindowVisible(hwnd).as_bool() {
                return Err("Could not hide the previous capture window".into());
            }
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::RawWindowHandle;
        let RawWindowHandle::AppKit(raw) = handle.as_raw() else {
            return Err("Expected an AppKit window".into());
        };
        // The borrowed AppKit handle owns a live NSView on this UI thread.
        let view = unsafe { &*raw.ns_view.as_ptr().cast::<objc2_app_kit::NSView>() };
        let window = view.window().ok_or("View is not attached to a window")?;
        window.orderOut(None);
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = handle;
        Err("Native overlay hiding is unavailable".into())
    }
}

pub fn show_window(handle: raw_window_handle::WindowHandle<'_>) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use raw_window_handle::RawWindowHandle;
        use windows::Win32::{
            Foundation::HWND,
            UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE},
        };
        let RawWindowHandle::Win32(raw) = handle.as_raw() else {
            return Err("Expected a Windows window".into());
        };
        unsafe {
            let _ = ShowWindow(HWND(raw.hwnd.get() as *mut _), SW_SHOWNOACTIVATE);
        }
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        use raw_window_handle::RawWindowHandle;
        let RawWindowHandle::AppKit(raw) = handle.as_raw() else {
            return Err("Expected an AppKit window".into());
        };
        let view = unsafe { &*raw.ns_view.as_ptr().cast::<objc2_app_kit::NSView>() };
        view.window()
            .ok_or("View is not attached to a window")?
            .orderFront(None);
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = handle;
        Err("Native overlay showing is unavailable".into())
    }
}

#[cfg(target_os = "windows")]
pub fn client_origin(handle: raw_window_handle::WindowHandle<'_>) -> Result<(i32, i32), String> {
    use raw_window_handle::RawWindowHandle;
    use windows::Win32::{
        Foundation::{HWND, POINT},
        Graphics::Gdi::ClientToScreen,
    };
    let RawWindowHandle::Win32(raw) = handle.as_raw() else {
        return Err("Expected a Windows window".into());
    };
    let mut origin = POINT::default();
    unsafe { ClientToScreen(HWND(raw.hwnd.get() as *mut _), &mut origin).ok() }
        .map_err(|error| error.to_string())?;
    Ok((origin.x, origin.y))
}

#[cfg(target_os = "windows")]
pub fn fit_client_bounds(
    handle: raw_window_handle::WindowHandle<'_>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<(), String> {
    use raw_window_handle::RawWindowHandle;
    use windows::Win32::{
        Foundation::{HWND, POINT, RECT},
        Graphics::Gdi::ClientToScreen,
        UI::WindowsAndMessaging::{
            GetClientRect, GetWindowRect, SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE,
        },
    };
    let RawWindowHandle::Win32(raw) = handle.as_raw() else {
        return Err("Expected a Windows window handle".into());
    };
    let hwnd = HWND(raw.hwnd.get() as *mut _);
    let width = i32::try_from(width).map_err(|_| "Capture width is too large")?;
    let height = i32::try_from(height).map_err(|_| "Capture height is too large")?;
    if width <= 0 || height <= 0 {
        return Err("Capture bounds are empty".into());
    }
    // The borrowed handle guarantees a live native window. All native output
    // buffers are stack values and no handle is retained beyond this call.
    let read = || -> Result<(RECT, RECT, POINT), String> {
        let (mut outer, mut client, mut origin) =
            (RECT::default(), RECT::default(), POINT::default());
        unsafe {
            GetWindowRect(hwnd, &mut outer).map_err(|error| error.to_string())?;
            GetClientRect(hwnd, &mut client).map_err(|error| error.to_string())?;
            ClientToScreen(hwnd, &mut origin)
                .ok()
                .map_err(|error| error.to_string())?;
        }
        Ok((outer, client, origin))
    };
    for _ in 0..2 {
        let (outer, client, origin) = read()?;
        if origin.x == x && origin.y == y && client.right == width && client.bottom == height {
            return Ok(());
        }
        let checked =
            |value: i64| i32::try_from(value).map_err(|_| "Window bounds overflow".to_string());
        unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                checked(outer.left as i64 + x as i64 - origin.x as i64)?,
                checked(outer.top as i64 + y as i64 - origin.y as i64)?,
                checked(
                    width as i64 + outer.right as i64 - outer.left as i64 - client.right as i64,
                )?,
                checked(
                    height as i64 + outer.bottom as i64 - outer.top as i64 - client.bottom as i64,
                )?,
                SWP_NOACTIVATE,
            )
            .map_err(|error| error.to_string())?;
        }
    }
    let (_, client, origin) = read()?;
    if origin.x != x || origin.y != y || client.right != width || client.bottom != height {
        return Err("Native client bounds differ from the captured pixels".into());
    }
    Ok(())
}
