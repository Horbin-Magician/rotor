use std::error::Error;
use chrono;

pub struct PinWin {
    _id: u32,
}

impl PinWin {
    pub fn new(
        offset_x: i32,
        offset_y: i32,
        id: u32,
    ) -> Result<PinWin, Box<dyn Error>> {
        
        // let file_name = chrono::Local::now().format("Rotor_%Y-%m-%d-%H-%M-%S.png").to_string();

        // { // code for key press
        //     let pin_window_clone = pin_window.as_weak();
        //     pin_window.on_key_pressed(move |shortcut| {
        //         //TODO: handle F1-F12
        //         let mut text = shortcut.text.to_string();
        //         if text == "\u{1b}" { text = "Esc".into(); } // escape
        //         else if text == " " { text = "Space".into(); } // space
        //         else if text == "\n" { text = "Enter".into(); } // enter
        //         else if text.as_str() > "\u{1f}" && text.as_str() < "\u{7f}" { text = text.to_uppercase(); } // char
        //         else { return; } // exclude other control string

        //         let mut shortcut_str = String::new();
        //         if shortcut.modifiers.control { shortcut_str += "Ctrl+"; }
        //         if shortcut.modifiers.shift { shortcut_str += "Shift+"; }
        //         if shortcut.modifiers.meta { shortcut_str += "Win+"; }
        //         if shortcut.modifiers.alt { shortcut_str += "Alt+"; }
        //         else { shortcut_str += &text; }
                
        //         if let Some(pin_window) = pin_window_clone.upgrade() {
        //             if let Ok(app_config) = AppConfig::global().try_lock() {
        //                 let default = "unkown".to_string();
        //                 let shortcut_pinwin_save = app_config.get_shortcut("pinwin_save").unwrap_or(&default);
        //                 let shortcut_pinwin_close = app_config.get_shortcut("pinwin_close").unwrap_or(&default);
        //                 let shortcut_pinwin_copy = app_config.get_shortcut("pinwin_copy").unwrap_or(&default);
        //                 let shortcut_pinwin_hide = app_config.get_shortcut("pinwin_hide").unwrap_or(&default);
        
        //                 if shortcut_str.eq(shortcut_pinwin_save){
        //                     pin_window.invoke_save();
        //                 } else if shortcut_str == *shortcut_pinwin_close {
        //                     pin_window.invoke_close();
        //                 } else if shortcut_str == *shortcut_pinwin_copy {
        //                     pin_window.invoke_copy();
        //                 } else if shortcut_str == *shortcut_pinwin_hide {
        //                     pin_window.invoke_hide();
        //                 }
        //             }
        //         }
        //     });
        // }
        
        Ok(PinWin {
            _id: id,
        })
    }
}
