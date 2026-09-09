//! Adapt muda's native menu to owner drawing without creating another window.
use std::cell::{Cell, RefCell};
use tray_icon::{
    dpi::Position,
    menu::{ContextMenu, Menu},
};
use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::{Dwm::*, Gdi::HDC},
    System::Threading::GetCurrentThreadId,
    UI::{
        Controls::{DRAWITEMSTRUCT, MEASUREITEMSTRUCT, ODT_MENU},
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};
use windows::core::BOOL;

const MN_GETHMENU: u32 = 0x01e1;
// tray-icon =0.21.3 uses this notification on its private owner window.
// Handle only menu-opening clicks here so TrackPopupMenu receives NOANIMATION;
// changing ContextMenu::show_context_menu_for_hwnd alone does not affect it.
const TRAY_ICON_CALLBACK: u32 = 6002;
const TRAY_MENU_FLAGS: TRACK_POPUP_MENU_FLAGS =
    TRACK_POPUP_MENU_FLAGS(TPM_BOTTOMALIGN.0 | TPM_LEFTALIGN.0 | TPM_NOANIMATION.0);

thread_local! {
    // Only active between WM_INITMENUPOPUP and attachment of its popup.
    static PENDING_POPUP: Cell<*const State> = const { Cell::new(std::ptr::null()) };
}

pub struct NativeMenu {
    // Drop the HMENU before its background brush.
    menu: Menu,
    state: Box<State>,
}

struct State {
    menu: HMENU,
    ids: [u32; 2],
    forced_dark: Option<bool>,
    dark: Cell<bool>,
    dirty: Cell<bool>,
    popup: Cell<Option<HWND>>,
    popup_size: Cell<(i32, i32)>,
    composited_frame: Cell<bool>,
    frame_initializing: Cell<bool>,
    hook: Cell<Option<HHOOK>>,
    tracking: Cell<bool>,
    painter: RefCell<rotor_ui::NativeMenuPainter>,
}

impl NativeMenu {
    pub fn new(menu: Menu, config: &rotor_common::Config) -> Result<Self, String> {
        let handle = HMENU(menu.hpopupmenu() as _);
        let forced_dark = match config.get("theme").map(String::as_str) {
            Some("1") => Some(false),
            Some("2") => Some(true),
            _ => None,
        };
        let state = Box::new(State {
            menu: handle,
            ids: unsafe { [GetMenuItemID(handle, 0), GetMenuItemID(handle, 1)] },
            forced_dark,
            dark: Cell::new(forced_dark.unwrap_or_else(system_dark)),
            dirty: Cell::new(true),
            popup: Cell::new(None),
            popup_size: Cell::new((0, 0)),
            composited_frame: Cell::new(false),
            frame_initializing: Cell::new(false),
            hook: Cell::new(None),
            tracking: Cell::new(false),
            painter: RefCell::new(rotor_ui::NativeMenuPainter::new(
                rotor_common::i18n::language_for_config(config) == "zh-CN",
            )),
        });
        state.prepare()?;
        set_owner_draw(handle, true)?;
        Ok(Self { menu, state })
    }
}

fn system_dark() -> bool {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
        .and_then(|key| key.get_value::<u32, _>("AppsUseLightTheme"))
        .is_ok_and(|value| value == 0)
}

fn set_owner_draw(menu: HMENU, enabled: bool) -> Result<(), String> {
    for index in 0..2 {
        // Change only the type: preserve muda's IDs, text and item data.
        let info = MENUITEMINFOW {
            cbSize: size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_FTYPE,
            fType: if enabled { MFT_OWNERDRAW } else { MFT_STRING },
            ..Default::default()
        };
        unsafe { SetMenuItemInfoW(menu, index, true, &info) }.map_err(|e| e.to_string())?;
    }
    Ok(())
}

impl State {
    fn show_at_cursor(&self, owner: HWND) {
        if self.tracking.replace(true) {
            return;
        }
        let mut point = POINT::default();
        unsafe {
            if GetCursorPos(&mut point).is_ok() {
                let _ = SetForegroundWindow(owner);
                let _ = TrackPopupMenu(
                    self.menu,
                    TRAY_MENU_FLAGS,
                    point.x,
                    point.y,
                    None,
                    owner,
                    None,
                );
                // Required for notification-area menus to dismiss reliably on
                // later invocations. WM_COMMAND still goes through muda.
                let _ = PostMessageW(Some(owner), WM_NULL, WPARAM(0), LPARAM(0));
            }
        }
        self.tracking.set(false);
    }

    fn stop_popup_hook(&self) {
        if let Some(hook) = self.hook.take() {
            unsafe {
                let _ = UnhookWindowsHookEx(hook);
            }
        }
        PENDING_POPUP.with(|pending| {
            if std::ptr::eq(pending.get(), self) {
                pending.set(std::ptr::null());
            }
        });
    }

    fn watch_popup(&self) {
        self.stop_popup_hook();
        PENDING_POPUP.with(|pending| pending.set(self));
        // The hook is thread-local and removed as soon as the matching menu
        // window is found, before Windows paints or captures its opening frame.
        match unsafe {
            SetWindowsHookExW(WH_CALLWNDPROC, Some(popup_hook), None, GetCurrentThreadId())
        } {
            Ok(hook) => self.hook.set(Some(hook)),
            Err(error) => {
                self.stop_popup_hook();
                log::error!("Could not watch native menu creation: {error}");
            }
        }
    }

    fn attach_popup(&self, hwnd: HWND) {
        if self.popup.get() == Some(hwnd) {
            return;
        }
        self.detach_popup();
        let data = self as *const Self as usize;
        if !unsafe { SetWindowSubclass(hwnd, Some(popup_proc), data, data) }.as_bool() {
            log::error!("Could not attach native menu frame drawing");
            return;
        }
        self.popup.set(Some(hwnd));
        // DWM setters can reenter the popup procedure. Do not momentarily
        // apply the GDI fallback while the compositor is being configured.
        self.frame_initializing.set(true);
        let color = self.painter.borrow().border_color();
        let composited = configure_composited_frame(hwnd, color, self.dark.get());
        self.composited_frame.set(composited);
        if composited {
            clear_popup_region(hwnd);
        } else {
            // Older systems retain the existing GDI fallback.
            unsafe {
                let policy = DWMNCRP_DISABLED;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_NCRENDERING_POLICY,
                    (&policy as *const DWMNCRENDERINGPOLICY).cast(),
                    size_of_val(&policy) as u32,
                );
                let color = DWMWA_COLOR_NONE;
                let _ = DwmSetWindowAttribute(
                    hwnd,
                    DWMWA_BORDER_COLOR,
                    (&color as *const u32).cast(),
                    size_of_val(&color) as u32,
                );
            }
        }
        self.frame_initializing.set(false);
        self.update_popup_shape(hwnd);
    }

    fn update_popup_shape(&self, hwnd: HWND) {
        if self.composited_frame.get() || self.frame_initializing.get() {
            return;
        }
        let mut rect = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
            return;
        }
        let size = (rect.right - rect.left, rect.bottom - rect.top);
        if size.0 > 0 && size.1 > 0 && self.popup_size.replace(size) != size {
            // SetWindowRgn can synchronously send WM_WINDOWPOSCHANGED again.
            // Record the size first so that nested notification is a no-op.
            self.painter.borrow().round_popup(hwnd);
        }
    }

    fn detach_popup(&self) {
        self.stop_popup_hook();
        self.popup_size.set((0, 0));
        if let Some(hwnd) = self.popup.take() {
            unsafe {
                let _ = RemoveWindowSubclass(hwnd, Some(popup_proc), self as *const Self as usize);
            }
        }
        self.composited_frame.set(false);
    }

    fn prepare(&self) -> Result<(), String> {
        let brush = self
            .painter
            .borrow_mut()
            .prepare(self.dark.get(), self.dirty.get())?;
        let mut info = MENUINFO {
            cbSize: size_of::<MENUINFO>() as u32,
            fMask: MIM_STYLE,
            ..Default::default()
        };
        unsafe { GetMenuInfo(self.menu, &mut info) }.map_err(|e| e.to_string())?;
        let info = MENUINFO {
            fMask: MIM_BACKGROUND | MIM_STYLE,
            // Neither item has an icon or check mark; do not reserve its gutter.
            dwStyle: info.dwStyle | MNS_NOCHECK,
            hbrBack: brush,
            ..info
        };
        unsafe { SetMenuInfo(self.menu, &info) }.map_err(|e| e.to_string())?;
        self.dirty.set(false);
        Ok(())
    }
}

fn clear_popup_region(hwnd: HWND) {
    // A new menu normally has no region. Do not issue SetWindowRgn in that
    // case: it can trigger a second position/paint cycle during appearance.
    unsafe {
        use windows::Win32::Graphics::Gdi::*;
        let region = CreateRectRgn(0, 0, 0, 0);
        if !region.is_invalid() {
            if GetWindowRgn(hwnd, region) != RGN_ERROR {
                SetWindowRgn(hwnd, None, false);
            }
            let _ = DeleteObject(region.into());
        }
    }
}

fn configure_composited_frame(
    hwnd: HWND,
    color: windows::Win32::Foundation::COLORREF,
    dark: bool,
) -> bool {
    unsafe {
        if !DwmIsCompositionEnabled().is_ok_and(|enabled| enabled.as_bool()) {
            return false;
        }
        // Forcing a normal nonclient frame onto the menu can introduce a
        // caption-style top highlight. Keep the popup's own frame policy.
        let policy = DWMNCRP_USEWINDOWSTYLE;
        let preference = DWMWCP_ROUNDSMALL;
        let dark = BOOL::from(dark);
        let disabled = BOOL(1);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            (&disabled as *const BOOL).cast(),
            size_of_val(&disabled) as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark as *const BOOL).cast(),
            size_of_val(&dark) as u32,
        );
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            (&policy as *const DWMNCRENDERINGPOLICY).cast(),
            size_of_val(&policy) as u32,
        )
        .is_ok()
            && DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR,
                (&color as *const windows::Win32::Foundation::COLORREF).cast(),
                size_of_val(&color) as u32,
            )
            .is_ok()
            && DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&preference as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
                size_of_val(&preference) as u32,
            )
            .is_ok()
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.detach_popup();
    }
}

unsafe extern "system" fn popup_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && lparam.0 != 0 {
        let event = unsafe { &*(lparam.0 as *const CWPSTRUCT) };
        if event.message == WM_WINDOWPOSCHANGING {
            PENDING_POPUP.with(|pending| {
                let Some(state) = (unsafe { pending.get().as_ref() }) else {
                    return;
                };
                // Match the exact HMENU. System popup windows need not expose
                // the tray owner through GW_OWNER on every Windows version.
                if unsafe { SendMessageW(event.hwnd, MN_GETHMENU, None, None) }.0
                    == state.menu.0 as isize
                {
                    state.attach_popup(event.hwnd);
                }
            });
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

impl ContextMenu for NativeMenu {
    fn hpopupmenu(&self) -> isize {
        self.menu.hpopupmenu()
    }

    unsafe fn show_context_menu_for_hwnd(&self, hwnd: isize, position: Option<Position>) -> bool {
        unsafe { self.menu.show_context_menu_for_hwnd(hwnd, position) }
    }

    unsafe fn attach_menu_subclass_for_hwnd(&self, hwnd: isize) {
        unsafe {
            self.menu.attach_menu_subclass_for_hwnd(hwnd);
        }
        let id = self.state.as_ref() as *const State as usize;
        // tray-icon detaches this subclass before dropping or replacing us.
        if !unsafe { SetWindowSubclass(HWND(hwnd as _), Some(menu_proc), id, id) }.as_bool() {
            log::error!("Could not attach native tray menu drawing; using system appearance");
            let _ = set_owner_draw(self.state.menu, false);
            let info = MENUINFO {
                cbSize: size_of::<MENUINFO>() as u32,
                fMask: MIM_BACKGROUND,
                hbrBack: unsafe {
                    windows::Win32::Graphics::Gdi::GetSysColorBrush(
                        windows::Win32::Graphics::Gdi::COLOR_MENU,
                    )
                },
                ..Default::default()
            };
            let _ = unsafe { SetMenuInfo(self.state.menu, &info) };
        }
    }

    unsafe fn detach_menu_subclass_from_hwnd(&self, hwnd: isize) {
        self.state.detach_popup();
        let id = self.state.as_ref() as *const State as usize;
        unsafe {
            let _ = RemoveWindowSubclass(HWND(hwnd as _), Some(menu_proc), id);
            self.menu.detach_menu_subclass_from_hwnd(hwnd);
        }
    }
}

unsafe extern "system" fn menu_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    // State is boxed and owned by NativeMenu for the entire attachment.
    let state = unsafe { &*(data as *const State) };
    match message {
        TRAY_ICON_CALLBACK if matches!(lparam.0 as u32, WM_RBUTTONDOWN | WM_LBUTTONDOWN) => {
            // The application uses tray-icon's default left-click menu too.
            state.show_at_cursor(hwnd);
            return LRESULT(0);
        }
        WM_ENTERIDLE if wparam.0 == MSGF_MENU as usize && lparam.0 != 0 => {
            let popup = HWND(lparam.0 as _);
            // MN_GETHMENU identifies this popup without touching other menus.
            if state.popup.get() != Some(popup)
                && unsafe { SendMessageW(popup, MN_GETHMENU, None, None) }.0
                    == state.menu.0 as isize
            {
                state.attach_popup(popup);
                state
                    .painter
                    .borrow()
                    .draw_popup_frame(popup, state.composited_frame.get());
            }
        }
        WM_EXITMENULOOP => state.detach_popup(),
        WM_INITMENUPOPUP if wparam.0 == state.menu.0 as usize => {
            if let Err(error) = state.prepare() {
                log::error!("Native menu: {error}");
            }
            // Invalidate Windows' cached item measurements after a DPI change.
            if let Err(error) = set_owner_draw(state.menu, true) {
                log::error!("Native menu metrics: {error}");
            }
            state.watch_popup();
        }
        WM_SETTINGCHANGE | WM_THEMECHANGED | WM_SYSCOLORCHANGE => {
            state
                .dark
                .set(state.forced_dark.unwrap_or_else(system_dark));
            state.dirty.set(true);
        }
        WM_MEASUREITEM if lparam.0 != 0 => {
            let item = unsafe { &mut *(lparam.0 as *mut MEASUREITEMSTRUCT) };
            if item.CtlType == ODT_MENU && state.ids.contains(&item.itemID) {
                (item.itemWidth, item.itemHeight) = state.painter.borrow().dimensions();
                return LRESULT(1);
            }
        }
        WM_DRAWITEM if lparam.0 != 0 => {
            let item = unsafe { &*(lparam.0 as *const DRAWITEMSTRUCT) };
            if item.CtlType == ODT_MENU
                && item.hwndItem.0 == state.menu.0
                && let Some(index) = state.ids.iter().position(|id| *id == item.itemID)
            {
                state.painter.borrow().draw(index, item);
                return LRESULT(1);
            }
        }
        WM_NCDESTROY => unsafe {
            state.detach_popup();
            let _ = RemoveWindowSubclass(hwnd, Some(menu_proc), id);
        },
        _ => {}
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

unsafe extern "system" fn popup_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    let state = unsafe { &*(data as *const State) };
    if message == WM_NCACTIVATE {
        // Activation can paint the native top highlight without WM_NCPAINT.
        // Preserve activation/dismissal handling but suppress that repaint.
        return unsafe { DefSubclassProc(hwnd, message, wparam, LPARAM(-1)) };
    }
    if message == WM_NCPAINT {
        if state.frame_initializing.get() {
            return LRESULT(0);
        }
        state
            .painter
            .borrow()
            .draw_popup_frame(hwnd, state.composited_frame.get());
        return LRESULT(0);
    }
    if message == WM_NCDESTROY {
        state.popup.set(None);
        unsafe {
            let _ = RemoveWindowSubclass(hwnd, Some(popup_proc), id);
        }
    }
    let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
    if message == WM_WINDOWPOSCHANGED {
        state.update_popup_shape(hwnd);
    }
    if message == WM_PRINT
        && lparam.0 as i32 & PRF_NONCLIENT != 0
        && wparam.0 != 0
        && (lparam.0 as i32 & PRF_CHECKVISIBLE == 0 || unsafe { IsWindowVisible(hwnd) }.as_bool())
    {
        // Menu animations can use an offscreen DC. Painting GetWindowDC here
        // would leave that captured frame with the system's original border.
        state.painter.borrow().draw_popup_frame_to(
            hwnd,
            HDC(wparam.0 as _),
            state.composited_frame.get(),
        );
    }
    if message == WM_PAINT {
        state
            .painter
            .borrow()
            .draw_popup_frame(hwnd, state.composited_frame.get());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tray_icon::menu::MenuItem;
    use windows::Win32::Graphics::Gdi::*;

    thread_local! {
        static POSITION_CHANGES: Cell<u32> = const { Cell::new(0) };
        static ACTIVATION_REPAINT: Cell<isize> = const { Cell::new(0) };
    }

    unsafe extern "system" fn synthetic_popup(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            if message == WM_WINDOWPOSCHANGED {
                POSITION_CHANGES.with(|count| count.set(count.get() + 1));
            }
            if message == WM_NCACTIVATE {
                ACTIVATION_REPAINT.with(|value| value.set(lparam.0));
            }
            if message == WM_NCCREATE {
                let create = &*(lparam.0 as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
            }
            if message == MN_GETHMENU {
                return LRESULT(GetWindowLongPtrW(hwnd, GWLP_USERDATA));
            }
            if message == WM_PRINT {
                if lparam.0 as i32 & PRF_CHECKVISIBLE == 0 {
                    let mut rect = RECT::default();
                    GetWindowRect(hwnd, &mut rect).unwrap();
                    let _ = PatBlt(
                        HDC(wparam.0 as _),
                        0,
                        0,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        WHITENESS,
                    );
                }
                return LRESULT(0);
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
    }

    #[test]
    fn popup_attaches_before_idle_and_prints_into_the_requested_dc() {
        let menu = Menu::new();
        menu.append(&MenuItem::new("设置", true, None)).unwrap();
        menu.append(&MenuItem::new("退出", true, None)).unwrap();
        let mut config = rotor_common::Config::default();
        config.insert("theme".into(), "2".into());
        let menu = NativeMenu::new(menu, &config).unwrap();
        unsafe {
            let class = windows::core::w!("RotorTrayFrameSyntheticTest");
            let atom = RegisterClassW(&WNDCLASSW {
                lpszClassName: class,
                lpfnWndProc: Some(synthetic_popup),
                ..Default::default()
            });
            assert_ne!(atom, 0);
            let owner = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                windows::core::w!("STATIC"),
                windows::core::w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            menu.attach_menu_subclass_for_hwnd(owner.0 as isize);
            SendMessageW(
                owner,
                WM_INITMENUPOPUP,
                Some(WPARAM(menu.state.menu.0 as usize)),
                None,
            );
            assert!(menu.state.hook.get().is_some());
            let popup = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                windows::core::w!(""),
                WS_POPUP | WS_BORDER,
                0,
                0,
                160,
                80,
                Some(owner),
                None,
                None,
                Some(menu.state.menu.0),
            )
            .unwrap();
            // Exercise the real thread hook while keeping both windows hidden.
            SetWindowPos(popup, None, 0, 0, 160, 80, SWP_NOACTIVATE | SWP_NOZORDER).unwrap();
            assert_eq!(menu.state.popup.get(), Some(popup));
            assert!(menu.state.hook.get().is_none());
            assert!(!IsWindowVisible(popup).as_bool());
            let region = CreateRectRgn(0, 0, 0, 0);
            let composited = menu.state.composited_frame.get();
            if composited {
                assert_eq!(GetWindowRgn(popup, region), RGN_ERROR);
                let changes = POSITION_CHANGES.with(Cell::get);
                clear_popup_region(popup);
                clear_popup_region(popup);
                assert_eq!(POSITION_CHANGES.with(Cell::get), changes);
                let mut preference = DWM_WINDOW_CORNER_PREFERENCE::default();
                DwmGetWindowAttribute(
                    popup,
                    DWMWA_WINDOW_CORNER_PREFERENCE,
                    (&mut preference as *mut DWM_WINDOW_CORNER_PREFERENCE).cast(),
                    size_of_val(&preference) as u32,
                )
                .unwrap();
                assert_eq!(preference, DWMWCP_ROUNDSMALL);
                // BORDER_COLOR supports setting, but not querying on all builds.
                // configure_composited_frame requires that setter to succeed.
            } else {
                assert_ne!(GetWindowRgn(popup, region), RGN_ERROR);
                assert!(!PtInRegion(region, 0, 0).as_bool());
                assert!(PtInRegion(region, 80, 0).as_bool());
            }
            let _ = DeleteObject(region.into());
            for active in [0, 1, 0] {
                SendMessageW(popup, WM_NCACTIVATE, Some(WPARAM(active)), Some(LPARAM(0)));
                assert_eq!(ACTIVATION_REPAINT.with(Cell::get), -1);
            }
            let mut policy = DWMNCRENDERINGPOLICY::default();
            if DwmGetWindowAttribute(
                popup,
                DWMWA_NCRENDERING_POLICY,
                (&mut policy as *mut DWMNCRENDERINGPOLICY).cast(),
                size_of_val(&policy) as u32,
            )
            .is_ok()
            {
                assert_eq!(
                    policy,
                    if composited {
                        DWMNCRP_USEWINDOWSTYLE
                    } else {
                        DWMNCRP_DISABLED
                    }
                );
            }

            let screen = GetDC(None);
            let dc = CreateCompatibleDC(Some(screen));
            let bitmap = CreateCompatibleBitmap(screen, 160, 80);
            ReleaseDC(None, screen);
            assert!(!dc.is_invalid() && !bitmap.is_invalid());
            let previous = SelectObject(dc, bitmap.into());
            menu.state
                .painter
                .borrow()
                .draw_popup_frame_to(popup, dc, composited);
            let expected = GetPixel(dc, 80, 0);
            assert_ne!(expected.0, 0xffffff);
            for _ in 0..2 {
                // The underlying procedure repaints a bright frame on each
                // request; the subclass must repair this very DC each time.
                SendMessageW(
                    popup,
                    WM_PRINT,
                    Some(WPARAM(dc.0 as usize)),
                    Some(LPARAM((PRF_NONCLIENT | PRF_CLIENT) as isize)),
                );
                assert_eq!(GetPixel(dc, 80, 0), expected);
                assert_eq!(GetPixel(dc, 159, 40), expected);
                assert_eq!(GetPixel(dc, 80, 79), expected);
                assert_eq!(GetPixel(dc, 0, 40), expected);
                assert_eq!(GetPixel(dc, 80, 40).0, 0xffffff);
            }
            let _ = PatBlt(dc, 0, 0, 160, 80, BLACKNESS);
            SendMessageW(
                popup,
                WM_PRINT,
                Some(WPARAM(dc.0 as usize)),
                Some(LPARAM((PRF_NONCLIENT | PRF_CHECKVISIBLE) as isize)),
            );
            assert_eq!(GetPixel(dc, 80, 0).0, 0);
            SelectObject(dc, previous);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(dc);
            DestroyWindow(popup).unwrap();
            assert!(menu.state.popup.get().is_none());
            // An unopened/cancelled popup must release the hook too.
            SendMessageW(
                owner,
                WM_INITMENUPOPUP,
                Some(WPARAM(menu.state.menu.0 as usize)),
                None,
            );
            SendMessageW(owner, WM_EXITMENULOOP, None, None);
            assert!(menu.state.hook.get().is_none());
            menu.detach_menu_subclass_from_hwnd(owner.0 as isize);
            DestroyWindow(owner).unwrap();
            UnregisterClassW(class, None).unwrap();
        }
    }

    #[test]
    fn owner_drawing_preserves_native_labels_ids_and_item_count() {
        let menu = Menu::new();
        menu.append(&MenuItem::new("设置", true, None)).unwrap();
        menu.append(&MenuItem::new("退出", true, None)).unwrap();
        let handle = HMENU(menu.hpopupmenu() as _);
        let ids = unsafe { [GetMenuItemID(handle, 0), GetMenuItemID(handle, 1)] };
        let mut config = rotor_common::Config::default();
        config.insert("theme".into(), "2".into());
        let menu = NativeMenu::new(menu, &config).unwrap();
        assert_eq!(unsafe { GetMenuItemCount(Some(handle)) }, 2);
        assert_eq!(menu.state.ids, ids);
        for (index, label) in ["设置", "退出"].iter().enumerate() {
            let mut text = [0u16; 32];
            let mut info = MENUITEMINFOW {
                cbSize: size_of::<MENUITEMINFOW>() as u32,
                fMask: MIIM_FTYPE | MIIM_STRING | MIIM_ID,
                dwTypeData: windows::core::PWSTR(text.as_mut_ptr()),
                cch: text.len() as u32,
                ..Default::default()
            };
            unsafe { GetMenuItemInfoW(handle, index as u32, true, &mut info) }.unwrap();
            assert_eq!(info.wID, ids[index]);
            assert_eq!(info.fType, MFT_OWNERDRAW);
            assert_eq!(
                String::from_utf16(&text[..info.cch as usize]).unwrap(),
                *label
            );
        }
        // A hidden synthetic owner exercises the real subclass without
        // opening a menu or any application profile.
        unsafe {
            let owner = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                windows::core::w!("STATIC"),
                windows::core::w!(""),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            )
            .unwrap();
            menu.attach_menu_subclass_for_hwnd(owner.0 as isize);
            SendMessageW(
                owner,
                WM_INITMENUPOPUP,
                Some(WPARAM(handle.0 as usize)),
                Some(LPARAM(0)),
            );
            let mut measure = MEASUREITEMSTRUCT {
                CtlType: ODT_MENU,
                itemID: ids[0],
                ..Default::default()
            };
            let measured = SendMessageW(
                owner,
                WM_MEASUREITEM,
                Some(WPARAM(0)),
                Some(LPARAM((&mut measure as *mut MEASUREITEMSTRUCT) as isize)),
            );
            menu.detach_menu_subclass_from_hwnd(owner.0 as isize);
            DestroyWindow(owner).unwrap();
            assert_eq!(measured, LRESULT(1));
            assert!(measure.itemWidth > 0 && measure.itemHeight > 0);
        }
        set_owner_draw(handle, false).unwrap();
        assert_eq!(unsafe { GetMenuItemCount(Some(handle)) }, 2);
    }
}
