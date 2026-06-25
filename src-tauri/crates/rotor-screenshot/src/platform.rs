#[cfg(target_os = "windows")]
use rotor_platform::sys_util;
#[cfg(target_os = "windows")]
use tauri::Manager;
use tauri::WebviewWindow;

#[cfg(target_os = "macos")]
fn update_macos_overlay_window(window: &WebviewWindow, order_front: bool) -> tauri::Result<()> {
    use cocoa::appkit::{NSWindow, NSWindowCollectionBehavior};
    use cocoa::base::id;
    use cocoa::foundation::NSInteger;

    const NS_SCREEN_SAVER_WINDOW_LEVEL: NSInteger = 1000;

    let ns_window = window.ns_window()? as usize;
    window.run_on_main_thread(move || {
        let ns_window = ns_window as id;
        if ns_window.is_null() {
            return;
        }

        unsafe {
            ns_window.setLevel_(NS_SCREEN_SAVER_WINDOW_LEVEL);
            ns_window.setCollectionBehavior_(
                ns_window.collectionBehavior()
                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces
                    | NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary,
            );

            if order_front && ns_window.isVisible() {
                ns_window.orderFrontRegardless();
            }
        }
    })
}

#[cfg(target_os = "macos")]
pub(crate) fn prepare_overlay_window(window: &WebviewWindow) -> tauri::Result<()> {
    update_macos_overlay_window(window, false)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn prepare_overlay_window(_window: &WebviewWindow) -> tauri::Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
pub(crate) fn raise_overlay_window(window: &WebviewWindow) -> tauri::Result<()> {
    update_macos_overlay_window(window, true)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn raise_overlay_window(_window: &WebviewWindow) -> tauri::Result<()> {
    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn disable_window_animation(window: &WebviewWindow) {
    window
        .hwnd()
        .map(|hwnd| {
            sys_util::forbid_window_animation(hwnd);
        })
        .ok();
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn disable_window_animation(_window: &WebviewWindow) {}
