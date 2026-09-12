#[cfg(target_os = "windows")]
mod win_imports {
    pub use is_root::is_root;
    pub use std::ffi::{CStr, CString};
    pub use windows::Win32::Foundation;
    pub use windows::Win32::Foundation::HWND;
    pub use windows::Win32::Storage::FileSystem;
    pub use windows::Win32::System::{ProcessStatus, Threading};
}
#[cfg(target_os = "windows")]
use win_imports::*;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryUsage {
    /// Private working set on Windows; total resident memory on macOS.
    pub resident_bytes: u64,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionStatus {
    pub key: String,
    pub name: String,
    pub granted: Option<bool>,
    pub detail: String,
}

// Check whether the disk represented by a drive letter is in ntfs format
#[cfg(target_os = "windows")]
pub fn is_ntfs(vol: char) -> bool {
    if let Ok(root_path_name) = CString::new(format!("{}:\\", vol)) {
        let mut volume_name_buffer = vec![0u8; Foundation::MAX_PATH as usize];
        let mut volume_serial_number: u32 = 0;
        let mut maximum_component_length: u32 = 0;
        let mut file_system_flags: u32 = 0;
        let mut file_system_name_buffer = vec![0u8; Foundation::MAX_PATH as usize];

        unsafe {
            if FileSystem::GetVolumeInformationA(
                windows::core::PCSTR(root_path_name.as_ptr() as *const u8),
                Some(&mut volume_name_buffer),
                Some(&mut volume_serial_number),
                Some(&mut maximum_component_length),
                Some(&mut file_system_flags),
                Some(&mut file_system_name_buffer),
            )
            .is_ok()
            {
                let result = CStr::from_ptr(file_system_name_buffer.as_ptr() as *const i8);
                return result.to_string_lossy() == "NTFS";
            }
        }
    }
    false
}

pub type WindowRect = (i32, i32, i32, u32, u32);

// On Windows, enumerate top-level windows directly so the rects cover the full
// visible frame (title bar included) in physical pixels, and a single bad
// window cannot fail the whole list.
#[cfg(target_os = "windows")]
pub fn get_all_window_rect() -> Result<Vec<WindowRect>, Box<dyn std::error::Error>> {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{LPARAM, RECT};
    use windows::Win32::Graphics::Dwm::{
        DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowThreadProcessId, IsWindowVisible,
    };

    unsafe extern "system" fn enum_window_callback(hwnd: HWND, state: LPARAM) -> BOOL {
        let hwnds = &mut *(state.0 as *mut Vec<HWND>);
        hwnds.push(hwnd);
        BOOL(1)
    }

    let mut hwnds: Vec<HWND> = Vec::new();
    unsafe {
        EnumWindows(
            Some(enum_window_callback),
            LPARAM(&mut hwnds as *mut Vec<HWND> as isize),
        )?;
    }

    let current_pid = unsafe { Threading::GetCurrentProcessId() };
    let window_count = hwnds.len() as i32;
    let mut res = Vec::new();

    // EnumWindows enumerates top-level windows in Z order, topmost first
    for (index, hwnd) in hwnds.into_iter().enumerate() {
        let rect = unsafe {
            if !IsWindowVisible(hwnd).as_bool() {
                continue;
            }

            // Skip windows owned by the current process (mask/pin windows)
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            if pid == current_pid {
                continue;
            }

            // Skip Program Manager
            let mut class_name = [0u16; 256];
            let class_name_len = GetClassNameW(hwnd, &mut class_name) as usize;
            if String::from_utf16_lossy(&class_name[..class_name_len]) == "Progman" {
                continue;
            }

            // Skip cloaked windows (other virtual desktops, hidden UWP windows)
            let mut cloaked = 0u32;
            let _ = DwmGetWindowAttribute(
                hwnd,
                DWMWA_CLOAKED,
                &mut cloaked as *mut u32 as *mut std::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            );
            if cloaked != 0 {
                continue;
            }

            // Visible frame bounds in physical pixels, title bar included
            let mut rect = RECT::default();
            if DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut rect as *mut RECT as *mut std::ffi::c_void,
                std::mem::size_of::<RECT>() as u32,
            )
            .is_err()
            {
                continue;
            }
            rect
        };

        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            continue;
        }

        let z = window_count - index as i32;
        res.push((rect.left, rect.top, z, width as u32, height as u32));
    }

    Ok(res)
}

#[cfg(target_os = "macos")]
#[path = "sys_util/macos_windows.rs"]
mod macos_windows;

#[cfg(target_os = "macos")]
pub fn get_all_window_rect() -> Result<Vec<WindowRect>, Box<dyn std::error::Error>> {
    macos_windows::window_rectangles().map_err(Into::into)
}

pub fn get_cursor_position() -> Result<(i32, i32), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    {
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

    #[cfg(target_os = "windows")]
    {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

        let mut point = POINT { x: 0, y: 0 };
        unsafe {
            GetCursorPos(&mut point)?;
        }
        Ok((point.x, point.y))
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        Ok((0, 0))
    }
}

#[cfg(target_os = "macos")]
pub fn get_memory_usage() -> Result<MemoryUsage, Box<dyn std::error::Error>> {
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

#[cfg(target_os = "windows")]
pub fn get_memory_usage() -> Result<MemoryUsage, Box<dyn std::error::Error>> {
    let mut counters = ProcessStatus::PROCESS_MEMORY_COUNTERS_EX2 {
        cb: std::mem::size_of::<ProcessStatus::PROCESS_MEMORY_COUNTERS_EX2>() as u32,
        ..Default::default()
    };

    unsafe {
        ProcessStatus::GetProcessMemoryInfo(
            Threading::GetCurrentProcess(),
            // EX2 extends the C-layout base structure; pass the full buffer size.
            std::ptr::from_mut(&mut counters).cast(),
            std::mem::size_of::<ProcessStatus::PROCESS_MEMORY_COUNTERS_EX2>() as u32,
        )?;
    }

    Ok(MemoryUsage {
        // PrivateUsage measures commit, not resident private pages.
        resident_bytes: counters.PrivateWorkingSetSize as u64,
    })
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn get_memory_usage() -> Result<MemoryUsage, Box<dyn std::error::Error>> {
    Ok(MemoryUsage { resident_bytes: 0 })
}

#[cfg(target_os = "macos")]
pub fn get_permission_statuses() -> Vec<PermissionStatus> {
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

#[cfg(target_os = "windows")]
pub fn get_permission_statuses() -> Vec<PermissionStatus> {
    vec![
        PermissionStatus {
            key: "administrator".to_string(),
            name: "Administrator".to_string(),
            granted: Some(is_root()),
            detail: "Required for NTFS journal indexing and admin file launches".to_string(),
        },
        PermissionStatus {
            key: "screen_capture".to_string(),
            name: "Screen Capture".to_string(),
            granted: Some(true),
            detail: "Allowed by the desktop session".to_string(),
        },
        PermissionStatus {
            key: "file_search".to_string(),
            name: "File Search".to_string(),
            granted: Some(true),
            detail: "Uses readable NTFS volumes".to_string(),
        },
    ]
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn get_permission_statuses() -> Vec<PermissionStatus> {
    vec![PermissionStatus {
        key: "file_search".to_string(),
        name: "File Search".to_string(),
        granted: None,
        detail: "Permission checks are unavailable on this platform".to_string(),
    }]
}

#[cfg(target_os = "macos")]
fn check_macos_screen_capture_permission() -> Option<bool> {
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
    }

    Some(unsafe { CGPreflightScreenCaptureAccess() })
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    #[test]
    fn current_process_private_working_set_is_available() {
        let memory = super::get_memory_usage().expect("private working set query failed");
        assert!(memory.resident_bytes > 0);
    }

    #[test]
    fn get_all_window_rect_returns_valid_rects() {
        let rects = super::get_all_window_rect().expect("get_all_window_rect failed");
        assert!(!rects.is_empty(), "expected at least one visible window");
        for &(x, y, z, width, height) in &rects {
            assert!(
                width > 0 && height > 0,
                "invalid rect: {x},{y},{z},{width}x{height}"
            );
        }
    }
}
