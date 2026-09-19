//! Adapt the tray library's menu to the platform's owner drawing.
//!
//! Lifecycle (attach/detach with the tray owner window) stays here; every
//! Win32 call lives in `rotor_platform::tray_menu`.
use rotor_platform::tray_menu::{MenuAppearance, MenuColors, OwnerDrawnMenu};
use tray_icon::{
    dpi::Position,
    menu::{ContextMenu, Menu},
};

pub struct NativeMenu {
    // Detach and drop the owner drawing before the HMENU it decorates.
    native: OwnerDrawnMenu,
    menu: Menu,
}

impl NativeMenu {
    pub fn new(menu: Menu, config: &rotor_common::Config) -> Result<Self, String> {
        let forced_dark = rotor_common::Settings::theme(config).forced_dark();
        let native = OwnerDrawnMenu::new(
            menu.hpopupmenu(),
            MenuAppearance {
                light: menu_colors(false),
                dark: menu_colors(true),
                forced_dark,
            },
        )?;
        Ok(Self { native, menu })
    }
}

fn menu_colors(dark: bool) -> MenuColors {
    let palette = rotor_ui::surface_palette(dark);
    MenuColors {
        background: palette.background,
        foreground: palette.foreground,
        hover: palette.hover,
        border: palette.border,
    }
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
            // tray-icon detaches this subclass before dropping or replacing us.
            self.native.attach_owner(hwnd);
        }
    }

    unsafe fn detach_menu_subclass_from_hwnd(&self, hwnd: isize) {
        unsafe {
            self.native.detach_owner(hwnd);
            self.menu.detach_menu_subclass_from_hwnd(hwnd);
        }
    }
}
