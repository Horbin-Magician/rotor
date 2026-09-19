//! Owner drawing for a native Win32 popup menu that another library created.
//!
//! The desktop shell builds the HMENU and owns its lifetime; this module only
//! receives the raw handle plus theme colors. It subclasses the tray owner
//! window, watches for the popup window, configures its DWM frame and paints
//! items and frame with cached GDI resources. No menu library or UI runtime
//! type is referenced here.
use std::cell::{Cell, RefCell};
use windows::core::BOOL;
use windows::Win32::{
    Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM},
    Graphics::{Dwm::*, Gdi::*},
    System::Threading::GetCurrentThreadId,
    UI::{
        Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW},
        Controls::{DRAWITEMSTRUCT, MEASUREITEMSTRUCT, ODS_SELECTED, ODT_MENU},
        HiDpi::{GetDpiForMonitor, SystemParametersInfoForDpi, MDT_EFFECTIVE_DPI},
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};

const MN_GETHMENU: u32 = 0x01e1;
// tray-icon =0.21.3 uses this notification on its private owner window.
// Handle menu-opening clicks while preserving Windows' animation preferences.
const TRAY_ICON_CALLBACK: u32 = 6002;
const TRAY_MENU_FLAGS: TRACK_POPUP_MENU_FLAGS =
    TRACK_POPUP_MENU_FLAGS(TPM_BOTTOMALIGN.0 | TPM_LEFTALIGN.0);
const ITEM_COUNT: usize = 2;

thread_local! {
    // Only active between WM_INITMENUPOPUP and attachment of its popup.
    static PENDING_POPUP: Cell<*const State> = const { Cell::new(std::ptr::null()) };
}

/// Theme colors as `0xRRGGBB`, the same layout the UI palette uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuColors {
    pub background: u32,
    pub foreground: u32,
    pub hover: u32,
    pub border: u32,
}

/// Everything the painter needs from the application: both palettes and
/// whether the settings force a mode instead of following the system theme.
#[derive(Clone, Copy, Debug)]
pub struct MenuAppearance {
    pub light: MenuColors,
    pub dark: MenuColors,
    pub forced_dark: Option<bool>,
}

/// Owner drawing attached to an existing two-item popup menu.
pub struct OwnerDrawnMenu {
    state: Box<State>,
}

struct State {
    menu: HMENU,
    ids: [u32; ITEM_COUNT],
    forced_dark: Option<bool>,
    dark: Cell<bool>,
    dirty: Cell<bool>,
    popup: Cell<Option<HWND>>,
    popup_size: Cell<(i32, i32)>,
    composited_frame: Cell<bool>,
    frame_initializing: Cell<bool>,
    hook: Cell<Option<HHOOK>>,
    tracking: Cell<bool>,
    painter: RefCell<Painter>,
}

impl OwnerDrawnMenu {
    /// `menu` is the raw HMENU. Its two item labels are read from the menu so
    /// the caller decides language and text once.
    pub fn new(menu: isize, appearance: MenuAppearance) -> Result<Self, String> {
        let handle = HMENU(menu as _);
        let ids = unsafe { [GetMenuItemID(handle, 0), GetMenuItemID(handle, 1)] };
        let labels = [item_label(handle, 0)?, item_label(handle, 1)?];
        let forced_dark = appearance.forced_dark;
        let state = Box::new(State {
            menu: handle,
            ids,
            forced_dark,
            dark: Cell::new(forced_dark.unwrap_or_else(system_dark)),
            dirty: Cell::new(true),
            popup: Cell::new(None),
            popup_size: Cell::new((0, 0)),
            composited_frame: Cell::new(false),
            frame_initializing: Cell::new(false),
            hook: Cell::new(None),
            tracking: Cell::new(false),
            painter: RefCell::new(Painter::new(labels, appearance.light, appearance.dark)),
        });
        state.prepare()?;
        set_owner_draw(handle, true)?;
        Ok(Self { state })
    }

    /// Subclass the menu's owner window. On failure the menu falls back to the
    /// system appearance instead of leaving unpainted owner-drawn items.
    ///
    /// # Safety
    /// `hwnd` must be a live window on the current thread, and
    /// [`Self::detach_owner`] must run before that window or this value is
    /// destroyed.
    pub unsafe fn attach_owner(&self, hwnd: isize) {
        let id = self.state.as_ref() as *const State as usize;
        if !unsafe { SetWindowSubclass(HWND(hwnd as _), Some(menu_proc), id, id) }.as_bool() {
            log::error!("Could not attach native tray menu drawing; using system appearance");
            let _ = set_owner_draw(self.state.menu, false);
            let info = MENUINFO {
                cbSize: size_of::<MENUINFO>() as u32,
                fMask: MIM_BACKGROUND,
                hbrBack: unsafe { GetSysColorBrush(COLOR_MENU) },
                ..Default::default()
            };
            let _ = unsafe { SetMenuInfo(self.state.menu, &info) };
        }
    }

    /// # Safety
    /// `hwnd` must be the window passed to [`Self::attach_owner`].
    pub unsafe fn detach_owner(&self, hwnd: isize) {
        self.state.detach_popup();
        let id = self.state.as_ref() as *const State as usize;
        unsafe {
            let _ = RemoveWindowSubclass(HWND(hwnd as _), Some(menu_proc), id);
        }
    }
}

fn item_label(menu: HMENU, index: u32) -> Result<String, String> {
    let mut text = [0u16; 64];
    let mut info = MENUITEMINFOW {
        cbSize: size_of::<MENUITEMINFOW>() as u32,
        fMask: MIIM_STRING,
        dwTypeData: windows::core::PWSTR(text.as_mut_ptr()),
        cch: text.len() as u32,
        ..Default::default()
    };
    unsafe { GetMenuItemInfoW(menu, index, true, &mut info) }
        .map_err(|error| format!("Native menu item {index}: {error}"))?;
    String::from_utf16(&text[..info.cch as usize])
        .map_err(|error| format!("Native menu item {index}: {error}"))
}

/// Reads the per-user Windows setting the shell follows when no theme is forced.
pub fn system_dark() -> bool {
    winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize")
        .and_then(|key| key.get_value::<u32, _>("AppsUseLightTheme"))
        .is_ok_and(|value| value == 0)
}

fn set_owner_draw(menu: HMENU, enabled: bool) -> Result<(), String> {
    for index in 0..ITEM_COUNT as u32 {
        // Change only the type: preserve the library's IDs, text and item data.
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
                // later invocations. WM_COMMAND still goes through the owner.
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
        let region = CreateRectRgn(0, 0, 0, 0);
        if !region.is_invalid() {
            if GetWindowRgn(hwnd, region) != RGN_ERROR {
                SetWindowRgn(hwnd, None, false);
            }
            let _ = DeleteObject(region.into());
        }
    }
}

fn configure_composited_frame(hwnd: HWND, color: COLORREF, dark: bool) -> bool {
    unsafe {
        if !DwmIsCompositionEnabled().is_ok_and(|enabled| enabled.as_bool()) {
            return false;
        }
        // Forcing a normal nonclient frame onto the menu can introduce a
        // caption-style top highlight. Keep the popup's own frame policy.
        let policy = DWMNCRP_USEWINDOWSTYLE;
        let preference = DWMWCP_ROUNDSMALL;
        let dark = BOOL::from(dark);
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
                (&color as *const COLORREF).cast(),
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
        // Some popup creation paths paint before the first position change.
        // Attach before either erase or nonclient paint reaches the menu proc.
        if matches!(
            event.message,
            WM_WINDOWPOSCHANGING | WM_ERASEBKGND | WM_NCPAINT
        ) {
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

unsafe extern "system" fn menu_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    // State is boxed and owned by OwnerDrawnMenu for the entire attachment.
    let state = unsafe { &*(data as *const State) };
    match message {
        TRAY_ICON_CALLBACK if matches!(lparam.0 as u32, WM_RBUTTONDOWN | WM_LBUTTONDOWN) => {
            // The application uses the tray library's default left-click menu too.
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
            if item.CtlType == ODT_MENU && item.hwndItem.0 == state.menu.0 {
                if let Some(index) = state.ids.iter().position(|id| *id == item.itemID) {
                    state.painter.borrow().draw(index, item);
                    return LRESULT(1);
                }
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
    if message == WM_ERASEBKGND {
        // The native class brush can expose a light surface before owner-drawn
        // items arrive. Cover the whole client area with our cached background.
        return LRESULT(isize::from(
            state
                .painter
                .borrow()
                .erase_popup_background(hwnd, HDC(wparam.0 as _)),
        ));
    }
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

/// Cached GDI resources for the two owner-drawn items and the popup frame.
struct Painter {
    labels: [Vec<u16>; ITEM_COUNT],
    light: MenuColors,
    dark: MenuColors,
    resources: Option<Resources>,
}

struct Resources {
    dpi: u32,
    dark: bool,
    font: HFONT,
    background: HBRUSH,
    selected: HBRUSH,
    border: HBRUSH,
    border_color: COLORREF,
    foreground: COLORREF,
    selected_foreground: COLORREF,
    width: u32,
    height: u32,
    padding: i32,
}

impl Drop for Resources {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.font.into());
            let _ = DeleteObject(self.background.into());
            let _ = DeleteObject(self.selected.into());
            let _ = DeleteObject(self.border.into());
        }
    }
}

impl Painter {
    fn new(labels: [String; ITEM_COUNT], light: MenuColors, dark: MenuColors) -> Self {
        Self {
            labels: labels.map(|label| label.encode_utf16().collect()),
            light,
            dark,
            resources: None,
        }
    }

    fn colors(&self, dark: bool) -> &MenuColors {
        if dark {
            &self.dark
        } else {
            &self.light
        }
    }

    /// Refresh only after a DPI, theme or system metrics change. Painting
    /// never creates fonts or brushes. The returned brush is owned here.
    fn prepare(&mut self, dark: bool, force: bool) -> Result<HBRUSH, String> {
        let mut cursor = POINT::default();
        // A locked/noninteractive desktop can deny cursor access. Warm the
        // cache at the last DPI (96 initially), then refresh when it is usable.
        let mut dpi = self.resources.as_ref().map_or(96, |r| r.dpi);
        if unsafe { GetCursorPos(&mut cursor) }.is_ok() {
            let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
            let (mut x, mut y) = (0, 0);
            if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x, &mut y) }.is_ok()
                && x > 0
            {
                dpi = x;
            }
        }
        if let Some(resources) = &self.resources {
            if !force && resources.dpi == dpi && resources.dark == dark {
                return Ok(resources.background);
            }
        }
        let mut metrics = NONCLIENTMETRICSW {
            cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
            ..Default::default()
        };
        unsafe {
            SystemParametersInfoForDpi(
                SPI_GETNONCLIENTMETRICS.0,
                metrics.cbSize,
                Some((&mut metrics as *mut NONCLIENTMETRICSW).cast()),
                0,
                dpi,
            )
        }
        .map_err(|error| format!("Native menu font metrics: {error}"))?;
        let mut contrast = HIGHCONTRASTW {
            cbSize: size_of::<HIGHCONTRASTW>() as u32,
            ..Default::default()
        };
        unsafe {
            SystemParametersInfoW(
                SPI_GETHIGHCONTRAST,
                contrast.cbSize,
                Some((&mut contrast as *mut HIGHCONTRASTW).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .map_err(|error| format!("Native menu contrast settings: {error}"))?;
        let palette = *self.colors(dark);
        let high_contrast = contrast.dwFlags.contains(HCF_HIGHCONTRASTON);
        let (background, foreground, selected, selected_foreground) = if high_contrast {
            unsafe {
                (
                    GetSysColor(COLOR_MENU),
                    GetSysColor(COLOR_MENUTEXT),
                    GetSysColor(COLOR_HIGHLIGHT),
                    GetSysColor(COLOR_HIGHLIGHTTEXT),
                )
            }
        } else {
            (
                rgb(palette.background),
                rgb(palette.foreground),
                rgb(palette.hover),
                rgb(palette.foreground),
            )
        };
        let border_color = COLORREF(if high_contrast {
            unsafe { GetSysColor(COLOR_WINDOWFRAME) }
        } else {
            rgb(palette.border)
        });
        let mut resources = Resources {
            dpi,
            dark,
            font: unsafe { CreateFontIndirectW(&metrics.lfMenuFont) },
            background: unsafe { CreateSolidBrush(COLORREF(background)) },
            selected: unsafe { CreateSolidBrush(COLORREF(selected)) },
            border: unsafe { CreateSolidBrush(border_color) },
            border_color,
            foreground: COLORREF(foreground),
            selected_foreground: COLORREF(selected_foreground),
            width: scaled(80, dpi) as u32,
            height: scaled(24, dpi) as u32,
            padding: scaled(10, dpi),
        };
        if resources.font.is_invalid()
            || resources.background.is_invalid()
            || resources.selected.is_invalid()
            || resources.border.is_invalid()
        {
            return Err("Could not allocate native menu drawing resources".into());
        }
        let dc = unsafe { GetDC(None) };
        if dc.is_invalid() {
            return Err("Could not measure native menu font".into());
        }
        let saved = unsafe { SelectObject(dc, resources.font.into()) };
        let mut measured = true;
        for label in &self.labels {
            let mut size = SIZE::default();
            measured &= unsafe { GetTextExtentPoint32W(dc, label, &mut size) }.as_bool();
            resources.width = resources
                .width
                .max((size.cx + resources.padding * 2) as u32);
            resources.height = resources.height.max((size.cy + scaled(6, dpi)) as u32);
        }
        unsafe {
            SelectObject(dc, saved);
            ReleaseDC(None, dc);
        }
        if !measured {
            return Err("Could not measure native menu labels".into());
        }
        let brush = resources.background;
        self.resources = Some(resources);
        Ok(brush)
    }

    fn dimensions(&self) -> (u32, u32) {
        self.resources
            .as_ref()
            .map(|r| (r.width, r.height))
            .unwrap_or((80, 28))
    }

    fn border_color(&self) -> COLORREF {
        self.resources
            .as_ref()
            .map_or(COLORREF(rgb(self.light.border)), |r| r.border_color)
    }

    /// Erase with the same brush as the items, including the first paint.
    /// Use the supplied DC so Windows can also render into an offscreen buffer.
    fn erase_popup_background(&self, hwnd: HWND, dc: HDC) -> bool {
        let Some(resources) = &self.resources else {
            return false;
        };
        if dc.is_invalid() {
            return false;
        }
        let mut rect = RECT::default();
        unsafe {
            GetClientRect(hwnd, &mut rect).is_ok() && FillRect(dc, &rect, resources.background) != 0
        }
    }

    /// Shape the native popup in window coordinates. Windows owns a region
    /// after a successful SetWindowRgn; failed transfers remain ours.
    fn round_popup(&self, hwnd: HWND) {
        let Some(resources) = &self.resources else {
            return;
        };
        unsafe {
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return;
            }
            let diameter = scaled(12, resources.dpi);
            let region = CreateRoundRectRgn(
                0,
                0,
                rect.right - rect.left + 1,
                rect.bottom - rect.top + 1,
                diameter,
                diameter,
            );
            if !region.is_invalid() && SetWindowRgn(hwnd, Some(region), true) == 0 {
                let _ = DeleteObject(region.into());
            }
        }
    }

    /// Replace the native nonclient edge using the cached theme brushes.
    fn draw_popup_frame(&self, hwnd: HWND, composited: bool) {
        unsafe {
            let dc = GetWindowDC(Some(hwnd));
            if dc.is_invalid() {
                return;
            }
            self.draw_popup_frame_to(hwnd, dc, composited);
            ReleaseDC(Some(hwnd), dc);
        }
    }

    /// WM_PRINT supplies its own DC, which can be an offscreen animation frame.
    fn draw_popup_frame_to(&self, hwnd: HWND, dc: HDC, composited: bool) {
        let Some(resources) = &self.resources else {
            return;
        };
        unsafe {
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_err() {
                return;
            }
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            resources.draw_frame(dc, width, height, composited);
        }
    }

    /// The structure and its DC must belong to the current WM_DRAWITEM.
    fn draw(&self, index: usize, item: &DRAWITEMSTRUCT) {
        let Some(resources) = &self.resources else {
            return;
        };
        let Some(label) = self.labels.get(index) else {
            return;
        };
        let selected = item.itemState.0 & ODS_SELECTED.0 != 0;
        unsafe {
            let saved = SaveDC(item.hDC);
            if saved == 0 {
                return;
            }
            FillRect(
                item.hDC,
                &item.rcItem,
                if selected {
                    resources.selected
                } else {
                    resources.background
                },
            );
            SelectObject(item.hDC, resources.font.into());
            SetBkMode(item.hDC, TRANSPARENT);
            SetTextColor(
                item.hDC,
                if selected {
                    resources.selected_foreground
                } else {
                    resources.foreground
                },
            );
            let mut size = SIZE::default();
            let _ = GetTextExtentPoint32W(item.hDC, label, &mut size);
            let _ = TextOutW(
                item.hDC,
                item.rcItem.left + (item.rcItem.right - item.rcItem.left - size.cx) / 2,
                item.rcItem.top + (item.rcItem.bottom - item.rcItem.top - size.cy) / 2,
                label,
            );
            let _ = RestoreDC(item.hDC, saved);
        }
    }
}

impl Resources {
    fn draw_frame(&self, dc: HDC, width: i32, height: i32, composited: bool) {
        unsafe {
            let diameter = scaled(12, self.dpi);
            // DWM needs an opaque rectangular surface: it supplies the smooth
            // outer mask and colored border during composition. A GDI outline
            // here would leave another aliased curve inside that smooth mask.
            let region = if composited {
                CreateRectRgn(0, 0, width, height)
            } else {
                CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter)
            };
            if !region.is_invalid() {
                // Native menu painting can repaint its rectangular edge during
                // WM_PAINT as well as WM_NCPAINT. Restore our entire frame last.
                let inset = scaled(3, self.dpi);
                let _ = FrameRgn(dc, region, self.background, inset, inset);
                if !composited {
                    let _ = FrameRgn(dc, region, self.border, 1, 1);
                }
                let _ = DeleteObject(region.into());
            }
        }
    }
}

fn rgb(value: u32) -> u32 {
    ((value & 0xff) << 16) | (value & 0xff00) | ((value >> 16) & 0xff)
}

fn scaled(value: i32, dpi: u32) -> i32 {
    ((i64::from(value) * i64::from(dpi) + 48) / 96) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::w;

    const LIGHT: MenuColors = MenuColors {
        background: 0xffffff,
        foreground: 0x202428,
        hover: 0xedf4f7,
        border: 0xe1e4e8,
    };
    const DARK: MenuColors = MenuColors {
        background: 0x111111,
        foreground: 0xf1f1f1,
        hover: 0x282828,
        border: 0x333333,
    };
    const IDS: [u32; 2] = [1001, 1002];

    fn appearance(forced_dark: Option<bool>) -> MenuAppearance {
        MenuAppearance {
            light: LIGHT,
            dark: DARK,
            forced_dark,
        }
    }

    /// A raw two-item popup menu with fixed IDs, independent of any menu library.
    fn synthetic_menu() -> HMENU {
        unsafe {
            let menu = CreatePopupMenu().unwrap();
            AppendMenuW(menu, MF_STRING, IDS[0] as usize, w!("设置")).unwrap();
            AppendMenuW(menu, MF_STRING, IDS[1] as usize, w!("退出")).unwrap();
            menu
        }
    }

    fn hidden_owner() -> HWND {
        unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!(""),
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
            .unwrap()
        }
    }

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
        let handle = synthetic_menu();
        let menu = OwnerDrawnMenu::new(handle.0 as isize, appearance(Some(true))).unwrap();
        unsafe {
            let class = w!("RotorTrayFrameSyntheticTest");
            let atom = RegisterClassW(&WNDCLASSW {
                lpszClassName: class,
                lpfnWndProc: Some(synthetic_popup),
                ..Default::default()
            });
            assert_ne!(atom, 0);
            let owner = hidden_owner();
            menu.attach_owner(owner.0 as isize);
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
                w!(""),
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
            // Re-arm before an erase with no intervening position change.
            // This must attach early and never delegate to the class brush.
            for dark in [false, true] {
                menu.state.detach_popup();
                menu.state.dark.set(dark);
                menu.state.prepare().unwrap();
                menu.state.watch_popup();
                let mut info = MENUINFO {
                    cbSize: size_of::<MENUINFO>() as u32,
                    fMask: MIM_BACKGROUND,
                    ..Default::default()
                };
                GetMenuInfo(menu.state.menu, &mut info).unwrap();
                let mut brush = LOGBRUSH::default();
                assert_ne!(
                    GetObjectW(
                        info.hbrBack.into(),
                        size_of::<LOGBRUSH>() as i32,
                        Some((&mut brush as *mut LOGBRUSH).cast()),
                    ),
                    0
                );
                for _ in 0..2 {
                    let _ = PatBlt(dc, 0, 0, 160, 80, WHITENESS);
                    assert_eq!(
                        SendMessageW(popup, WM_ERASEBKGND, Some(WPARAM(dc.0 as usize)), None),
                        LRESULT(1)
                    );
                    assert_eq!(menu.state.popup.get(), Some(popup));
                    assert!(menu.state.hook.get().is_none());
                    let mut client = RECT::default();
                    GetClientRect(popup, &mut client).unwrap();
                    for (x, y) in [(0, 0), (80, 40), (client.right - 1, client.bottom - 1)] {
                        assert_eq!(GetPixel(dc, x, y), brush.lbColor);
                    }
                }
            }
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
            menu.detach_owner(owner.0 as isize);
            DestroyWindow(owner).unwrap();
            UnregisterClassW(class, None).unwrap();
            drop(menu);
            DestroyMenu(handle).unwrap();
        }
    }

    #[test]
    fn owner_drawing_preserves_native_labels_ids_and_item_count() {
        let handle = synthetic_menu();
        let menu = OwnerDrawnMenu::new(handle.0 as isize, appearance(Some(true))).unwrap();
        assert_eq!(unsafe { GetMenuItemCount(Some(handle)) }, 2);
        assert_eq!(menu.state.ids, IDS);
        assert_eq!(
            menu.state.painter.borrow().labels,
            ["设置", "退出"].map(|label| label.encode_utf16().collect::<Vec<u16>>())
        );
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
            assert_eq!(info.wID, IDS[index]);
            assert_eq!(info.fType, MFT_OWNERDRAW);
            assert_eq!(
                String::from_utf16(&text[..info.cch as usize]).unwrap(),
                *label
            );
        }
        // A hidden synthetic owner exercises the real subclass without
        // opening a menu or any application profile.
        unsafe {
            let owner = hidden_owner();
            menu.attach_owner(owner.0 as isize);
            SendMessageW(
                owner,
                WM_INITMENUPOPUP,
                Some(WPARAM(handle.0 as usize)),
                Some(LPARAM(0)),
            );
            let mut measure = MEASUREITEMSTRUCT {
                CtlType: ODT_MENU,
                itemID: IDS[0],
                ..Default::default()
            };
            let measured = SendMessageW(
                owner,
                WM_MEASUREITEM,
                Some(WPARAM(0)),
                Some(LPARAM((&mut measure as *mut MEASUREITEMSTRUCT) as isize)),
            );
            menu.detach_owner(owner.0 as isize);
            DestroyWindow(owner).unwrap();
            assert_eq!(measured, LRESULT(1));
            assert!(measure.itemWidth > 0 && measure.itemHeight > 0);
        }
        set_owner_draw(handle, false).unwrap();
        assert_eq!(unsafe { GetMenuItemCount(Some(handle)) }, 2);
        drop(menu);
        unsafe { DestroyMenu(handle) }.unwrap();
    }

    #[test]
    fn rounded_frame_replaces_bright_edges_without_touching_content() {
        let mut painter = Painter::new(["设置".into(), "退出".into()], LIGHT, DARK);
        for dark in [false, true] {
            painter.prepare(dark, true).unwrap();
            let resources = painter.resources.as_mut().unwrap();
            for dpi in [96, 144, 192] {
                resources.dpi = dpi;
                let width = scaled(80, dpi);
                let height = scaled(56, dpi);
                unsafe {
                    let screen = GetDC(None);
                    let dc = CreateCompatibleDC(Some(screen));
                    let bitmap = CreateCompatibleBitmap(screen, width, height);
                    ReleaseDC(None, screen);
                    assert!(!dc.is_invalid() && !bitmap.is_invalid());
                    let previous = SelectObject(dc, bitmap.into());
                    // Simulate the native menu repainting a bright rectangle.
                    let _ = PatBlt(dc, 0, 0, width, height, WHITENESS);
                    resources.draw_frame(dc, width, height, false);
                    let mut border = LOGBRUSH::default();
                    assert_ne!(
                        GetObjectW(
                            resources.border.into(),
                            size_of::<LOGBRUSH>() as i32,
                            Some((&mut border as *mut LOGBRUSH).cast())
                        ),
                        0
                    );
                    for (x, y) in [
                        (width / 2, 0),
                        (0, height / 2),
                        (width - 1, height / 2),
                        (width / 2, height - 1),
                    ] {
                        assert_eq!(
                            GetPixel(dc, x, y),
                            border.lbColor,
                            "edge at {x},{y}, DPI {dpi}"
                        );
                    }
                    let diameter = scaled(12, dpi);
                    let region =
                        CreateRoundRectRgn(0, 0, width + 1, height + 1, diameter, diameter);
                    // Check the curved outline too, where rectangular native
                    // borders previously ended abruptly in the supplied image.
                    for y in 0..diameter / 2 {
                        let x = (0..width / 2)
                            .find(|&x| PtInRegion(region, x, y).as_bool())
                            .unwrap();
                        assert_eq!(GetPixel(dc, x, y), border.lbColor);
                    }
                    assert_eq!(GetPixel(dc, 0, 0), COLORREF(rgb(0xffffff)));
                    assert_eq!(GetPixel(dc, width / 2, height / 2), COLORREF(rgb(0xffffff)));
                    let _ = DeleteObject(region.into());
                    // With DWM rounding, all corners must remain opaque menu
                    // background; no GDI staircase or second outline is drawn.
                    resources.draw_frame(dc, width, height, true);
                    let mut background = LOGBRUSH::default();
                    assert_ne!(
                        GetObjectW(
                            resources.background.into(),
                            size_of::<LOGBRUSH>() as i32,
                            Some((&mut background as *mut LOGBRUSH).cast())
                        ),
                        0
                    );
                    for (x, y) in [
                        (0, 0),
                        (width - 1, 0),
                        (0, height - 1),
                        (width - 1, height - 1),
                        (width / 2, 0),
                    ] {
                        assert_eq!(GetPixel(dc, x, y), background.lbColor);
                    }
                    assert_eq!(GetPixel(dc, width / 2, height / 2), COLORREF(rgb(0xffffff)));
                    SelectObject(dc, previous);
                    let _ = DeleteObject(bitmap.into());
                    let _ = DeleteDC(dc);
                }
            }
        }
    }

    #[test]
    fn drawing_uses_cached_brush_and_restores_the_callers_dc() {
        let mut painter = Painter::new(["设置".into(), "退出".into()], LIGHT, DARK);
        for dark in [false, true] {
            let brush = painter.prepare(dark, true).unwrap();
            assert_eq!(painter.prepare(dark, false).unwrap(), brush);
            let (width, height) = painter.dimensions();
            assert!(width > 0 && height > 0);
            unsafe {
                let screen = GetDC(None);
                assert!(!screen.is_invalid());
                let dc = CreateCompatibleDC(Some(screen));
                let bitmap = CreateCompatibleBitmap(screen, width as i32, height as i32);
                ReleaseDC(None, screen);
                assert!(!dc.is_invalid() && !bitmap.is_invalid());
                let previous = SelectObject(dc, bitmap.into());
                let original_color = COLORREF(rgb(0xabcdef));
                SetTextColor(dc, original_color);
                let mut item = DRAWITEMSTRUCT {
                    CtlType: ODT_MENU,
                    hDC: dc,
                    rcItem: RECT {
                        left: 0,
                        top: 0,
                        right: width as i32,
                        bottom: height as i32,
                    },
                    ..Default::default()
                };
                for selected in [false, true] {
                    item.itemState = if selected {
                        ODS_SELECTED
                    } else {
                        Default::default()
                    };
                    painter.draw(0, &item);
                    let resources = painter.resources.as_ref().unwrap();
                    let expected = if selected {
                        resources.selected
                    } else {
                        resources.background
                    };
                    let mut brush_info = LOGBRUSH::default();
                    assert_ne!(
                        GetObjectW(
                            expected.into(),
                            size_of::<LOGBRUSH>() as i32,
                            Some((&mut brush_info as *mut LOGBRUSH).cast())
                        ),
                        0
                    );
                    assert_eq!(GetPixel(dc, 1, 1), brush_info.lbColor);
                    assert_eq!(GetTextColor(dc), original_color);
                }
                SelectObject(dc, previous);
                let _ = DeleteObject(bitmap.into());
                let _ = DeleteDC(dc);
            }
        }
    }

    #[test]
    fn native_colors_and_dpi_units_match_windows_formats() {
        assert_eq!(rgb(0x1c2027), 0x27201c);
        assert_eq!(scaled(32, 96), 32);
        assert_eq!(scaled(32, 144), 48);
        assert_eq!(scaled(32, 192), 64);
    }
}
