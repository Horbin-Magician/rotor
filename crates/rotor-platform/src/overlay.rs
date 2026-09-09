pub fn settle_desktop() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    unsafe { windows::Win32::Graphics::Dwm::DwmFlush() }.map_err(|error| error.to_string())?;
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
