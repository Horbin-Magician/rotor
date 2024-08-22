pub mod app_config;

use crossbeam::channel::Sender;
use i_slint_backend_winit::WinitWindowAccessor;
use slint::ComponentHandle;
use windows_sys::Win32::UI::WindowsAndMessaging;

use app_config::AppConfig;
use crate::core::util::net_util;
use crate::ui::SettingWindow;
use super::AppMessage;

pub struct Setting {
    pub setting_win: SettingWindow,
}

impl Setting {
    pub fn new( msg_sender: Sender<AppMessage> ) -> Setting {
        let setting_win = SettingWindow::new().unwrap();

        {
            let width: f32 = 500.;
            let height: f32 = 400.;
            let x_screen: f32;
            let y_screen: f32;
            unsafe{
                x_screen = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXSCREEN) as f32;
                y_screen = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CYSCREEN) as f32;
            }
            let x_pos = ((x_screen - width * setting_win.window().scale_factor()) * 0.5) as i32;
            let y_pos = ((y_screen - height * setting_win.window().scale_factor()) * 0.4) as i32;
            setting_win.window().set_position(slint::WindowPosition::Physical(slint::PhysicalPosition::new(x_pos, y_pos)));
        }

        {   
            let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
            setting_win.set_version(version.into());
            
            let app_config = AppConfig::global().lock().unwrap();
            setting_win.set_power_boot(app_config.get_power_boot());
            setting_win.set_theme(app_config.get_theme() as i32);
            // TODO Batch setting
            setting_win.set_shortcut_search(app_config.get_shortcut("search").unwrap_or(&"unkown".to_string()).into());
            setting_win.set_shortcut_screenshot(app_config.get_shortcut("screenshot").unwrap_or(&"unkown".to_string()).into());
            setting_win.set_shortcut_pinwin_save(app_config.get_shortcut("pinwin_save").unwrap_or(&"unkown".to_string()).into());
            setting_win.set_shortcut_pinwin_close(app_config.get_shortcut("pinwin_close").unwrap_or(&"unkown".to_string()).into());
            setting_win.set_shortcut_pinwin_copy(app_config.get_shortcut("pinwin_copy").unwrap_or(&"unkown".to_string()).into());
            setting_win.set_shortcut_pinwin_hide(app_config.get_shortcut("pinwin_hide").unwrap_or(&"unkown".to_string()).into());
        }

        { // code for setting change
            { // power boot
                setting_win.on_power_boot_changed(move |power_boot| {
                    let mut app_config = AppConfig::global().lock().unwrap();
                    app_config.set_power_boot(power_boot);
                });
            }

            {// theme
                let setting_win_clone = setting_win.as_weak();
                setting_win.on_theme_changed(move |theme| {
                    let setting_win = setting_win_clone.unwrap();
                    setting_win.invoke_change_theme(theme);
                    let mut app_config = AppConfig::global().lock().unwrap();
                    app_config.set_theme(theme as u8);
                });
            }

            {// shortcut
                let setting_win_clone = setting_win.as_weak();
                let msg_sender = msg_sender.clone();
                setting_win.on_shortcut_changed(move |id, shortcut| {
                    //TODO: handle F1-F12
                    let mut text = shortcut.text.to_string();
                    if text == "\u{1b}" { text = "Esc".into(); } // escape
                    else if text == " " { text = "Space".into(); } // space
                    else if text == "\n" { text = "Enter".into(); } // enter
                    else if text.as_str() > "\u{1f}" && text.as_str() < "\u{7f}" { text = text.to_uppercase(); } // char
                    else { return; } // exclude other control string
                    
                    let mut shortcut_str = String::new();
                    if shortcut.modifiers.control { shortcut_str += "Ctrl+"; }
                    if shortcut.modifiers.shift { shortcut_str += "Shift+"; }
                    if shortcut.modifiers.meta { shortcut_str += "Win+"; }
                    if shortcut.modifiers.alt { shortcut_str += "Alt+"; }
                    else { shortcut_str += &text; }

                    msg_sender.send(AppMessage::ChangeHotkey(id.to_string(), shortcut_str.clone())).unwrap();

                    // TODO Batch setting
                    let setting_win = setting_win_clone.unwrap();
                    if id == "search" { setting_win.set_shortcut_search(shortcut_str.clone().into());
                    } else if id == "screenshot" { setting_win.set_shortcut_screenshot(shortcut_str.clone().into());}
                    else if id == "pinwin_save" { setting_win.set_shortcut_pinwin_save(shortcut_str.clone().into());}
                    else if id == "pinwin_close" { setting_win.set_shortcut_pinwin_close(shortcut_str.clone().into());}
                    else if id == "pinwin_copy" { setting_win.set_shortcut_pinwin_copy(shortcut_str.clone().into());}
                    else if id == "pinwin_hide" { setting_win.set_shortcut_pinwin_hide(shortcut_str.clone().into());}

                    let mut app_config = AppConfig::global().lock().unwrap();
                    app_config.set_shortcut(id.to_string(), shortcut_str);
                });
            }
        }

        { // minimize, close, win move
            let setting_win_clone = setting_win.as_weak();
            setting_win.on_minimize(move || {
                setting_win_clone.unwrap().window().with_winit_window(|winit_win| {
                    winit_win.set_minimized(true);
                });
            });

            let setting_win_clone = setting_win.as_weak();
            setting_win.on_close(move || {
                setting_win_clone.unwrap().hide().unwrap();
            });

            let setting_win_clone = setting_win.as_weak();
            setting_win.on_win_move(move || {
                setting_win_clone.unwrap().window().with_winit_window(|winit_win| {
                    let _ = winit_win.drag_window();
                });
            });
        }

        { // update
            let setting_win_clone = setting_win.as_weak();
            setting_win.on_check_update(move || {
                let setting_window = setting_win_clone.unwrap();
                setting_window.set_block(true);

                let setting_win_clone = setting_win_clone.clone();
                std::thread::spawn(move || {
                    let latest_version = net_util::Updater::global().lock().unwrap().get_latest_version().unwrap_or("unknown".to_string());
                    let current_version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");

                    setting_win_clone.upgrade_in_event_loop(move |setting_window| {
                        setting_window.set_current_version(current_version.into());
                        setting_window.set_latest_version(latest_version.into());
                        setting_window.set_update_state(1);
                        setting_window.set_block(false);
                    }).unwrap();
                });
            });

            let setting_win_clone = setting_win.as_weak();
            setting_win.on_update(move || {
                let setting_window = setting_win_clone.unwrap();
                setting_window.set_block(true);
                setting_window.set_update_state(0);

                let setting_win_clone = setting_win_clone.clone();
                std::thread::spawn(move || {
                    net_util::Updater::global().lock().unwrap().update_software().unwrap_or_else(
                        |_| {
                            setting_win_clone.upgrade_in_event_loop(move |setting_window| {
                                setting_window.set_block(false);
                                setting_window.set_update_state(2);
                            }).unwrap();
                        }
                    );
                });
            });
        }

        Setting {
            setting_win
        }
    }
}