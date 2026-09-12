use crate::objc_bridge::{ObjcId as id, string_utf8};
use cocoa::appkit::{NSAppearanceNameVibrantDark, NSAppearanceNameVibrantLight};
use gpui::WindowAppearance;
use objc::{msg_send, sel, sel_impl};
use std::ffi::CStr;

pub(crate) unsafe fn window_appearance_from_native(appearance: id) -> WindowAppearance {
    let name: id = msg_send![appearance, name];
    unsafe {
        if name == NSAppearanceNameVibrantLight {
            WindowAppearance::VibrantLight
        } else if name == NSAppearanceNameVibrantDark {
            WindowAppearance::VibrantDark
        } else if name == NSAppearanceNameAqua {
            WindowAppearance::Light
        } else if name == NSAppearanceNameDarkAqua {
            WindowAppearance::Dark
        } else {
            println!(
                "unknown appearance: {:?}",
                CStr::from_ptr(string_utf8(name))
            );
            WindowAppearance::Light
        }
    }
}

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    pub static NSAppearanceNameAqua: id;
    pub static NSAppearanceNameDarkAqua: id;
}
