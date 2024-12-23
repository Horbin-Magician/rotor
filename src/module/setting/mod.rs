use std::error::Error;
use std::sync::mpsc;
use crossbeam;
use global_hotkey::hotkey::HotKey;
use i_slint_backend_winit::WinitWindowAccessor;
use slint::ComponentHandle;
use windows::Win32::UI::WindowsAndMessaging;

use crate::core::application::{AppMessage, app_config::AppConfig};
use crate::util::net_util::Updater;
use crate::util::{file_util, log_util};
use crate::ui::SettingWindow;
use crate::module::{Module, ModuleMessage};


pub struct Setting {
    pub setting_win: SettingWindow,
}

impl Module for Setting{
    fn flag(&self) -> &str { "setting" }

    fn run(&self) -> mpsc::Sender<ModuleMessage> {
        let (msg_sender, msg_reciever) = mpsc::channel();
        let search_win_clone = self.setting_win.as_weak();
        std::thread::spawn(move || {
            loop {
                match msg_reciever.recv() {
                    Ok(ModuleMessage::Trigger) => {
                        search_win_clone.upgrade_in_event_loop(move |win| {
                            let _ = win.show();
                        }).unwrap_or_else(
                            |e| log_util::log_error(format!("Setting module failed to show window: {:?}", e))
                        );
                    },
                    Err(e) => {
                        log_util::log_error(format!("Setting module failed to receive message: {:?}", e));
                    }
                }
            }
        });
        msg_sender
    }

    fn get_hotkey(&mut self) -> Option<HotKey> {
        None
    }

    fn clean(&self) {
        // nothing need to clean until now
    }
}

impl Setting {
    pub fn new( msg_sender: crossbeam::channel::Sender<AppMessage> ) -> Result<Setting, Box<dyn Error>> {
        let setting_win = SettingWindow::new()?;

        let mut app_config = AppConfig::global().lock()?;
        setting_win.invoke_change_theme(app_config.get_theme() as i32);
        app_config.setting_win = Some(setting_win.as_weak());

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

        let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
        setting_win.set_version(version.into());

        // TODO Batch setting
        setting_win.set_power_boot(app_config.get_power_boot());
        setting_win.set_language(app_config.get_language() as i32);
        setting_win.set_theme(app_config.get_theme() as i32);
        setting_win.set_shortcut_search(app_config.get_shortcut("search").unwrap_or(&"unkown".to_string()).into());
        setting_win.set_shortcut_screenshot(app_config.get_shortcut("screenshot").unwrap_or(&"unkown".to_string()).into());
        setting_win.set_shortcut_pinwin_save(app_config.get_shortcut("pinwin_save").unwrap_or(&"unkown".to_string()).into());
        setting_win.set_shortcut_pinwin_close(app_config.get_shortcut("pinwin_close").unwrap_or(&"unkown".to_string()).into());
        setting_win.set_shortcut_pinwin_copy(app_config.get_shortcut("pinwin_copy").unwrap_or(&"unkown".to_string()).into());
        setting_win.set_shortcut_pinwin_hide(app_config.get_shortcut("pinwin_hide").unwrap_or(&"unkown".to_string()).into());
        
        setting_win.set_zoom_delta(app_config.get_zoom_delta().to_string().into());

        { // code for setting change
            { // power boot
                setting_win.on_power_boot_changed(move |power_boot| {
                    AppConfig::global()
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .set_power_boot(power_boot)
                        .unwrap_or_else(|e| log_util::log_error(format!("Failed to set power boot: {:?}", e)));
                });
            }

            {// language
                setting_win.on_language_changed(move |language| {
                    AppConfig::global()
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .set_language(language as u8);
                });
            }

            {// theme
                setting_win.on_theme_changed(move |theme| {
                    AppConfig::global()
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .set_theme(theme as u8);
                });
            }

            { // screenshot
                let setting_win_clone = setting_win.as_weak();
                setting_win.on_zoom_delta_changed(move |zoom_delta| {
                    let zoom_delta_int = zoom_delta.parse::<u8>().unwrap_or(2);
                    if let Some(setting_win) = setting_win_clone.upgrade() {
                        setting_win.set_zoom_delta(zoom_delta_int.to_string().into());
                    }
                    AppConfig::global()
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .set_zoom_delta(zoom_delta_int)
                        .unwrap_or_else(|e| log_util::log_error(format!("Failed to set zoom delta: {:?}", e)));
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

                    let _ = msg_sender.send(AppMessage::ChangeHotkey(id.to_string(), shortcut_str.clone()));

                    // TODO Batch setting
                    if let Some(setting_win) = setting_win_clone.upgrade() {
                        if id == "search" { setting_win.set_shortcut_search(shortcut_str.clone().into());
                        } else if id == "screenshot" { setting_win.set_shortcut_screenshot(shortcut_str.clone().into());}
                        else if id == "pinwin_save" { setting_win.set_shortcut_pinwin_save(shortcut_str.clone().into());}
                        else if id == "pinwin_close" { setting_win.set_shortcut_pinwin_close(shortcut_str.clone().into());}
                        else if id == "pinwin_copy" { setting_win.set_shortcut_pinwin_copy(shortcut_str.clone().into());}
                        else if id == "pinwin_hide" { setting_win.set_shortcut_pinwin_hide(shortcut_str.clone().into());}
                    }

                    AppConfig::global()
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .set_shortcut(id.to_string(), shortcut_str);
                });
            }
        }

        { // minimize, close, win move
            let setting_win_clone = setting_win.as_weak();
            setting_win.on_minimize(move || {
                if let Some(setting_win) = setting_win_clone.upgrade() {
                    setting_win.window().with_winit_window(|winit_win| {
                        winit_win.set_minimized(true);
                    });
                }
            });

            let setting_win_clone = setting_win.as_weak();
            setting_win.on_close(move || {
                if let Some(setting_win) = setting_win_clone.upgrade() {
                    let _ = setting_win.hide();
                }
            });

            let setting_win_clone = setting_win.as_weak();
            setting_win.on_win_move(move || {
                if let Some(setting_win) = setting_win_clone.upgrade() {
                    setting_win.window().with_winit_window(|winit_win| {
                        winit_win.drag_window().unwrap_or_else(
                            |e| log_util::log_error(format!("Failed to drag window: {:?}", e))
                        );
                    });
                }
            });
        }

        { // update
            let setting_win_clone = setting_win.as_weak();
            setting_win.on_check_update(move || {
                if let Some(setting_win) = setting_win_clone.upgrade() {
                    setting_win.set_block(true);
                }

                let setting_win_clone = setting_win_clone.clone();
                std::thread::spawn(move || {
                    let mut updater = Updater::global().lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let latest_version = updater.get_latest_version().unwrap_or("unknown".to_string());
                    let update_info = updater.get_update_info();

                    let current_version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");

                    setting_win_clone.upgrade_in_event_loop(move |setting_window| {
                        setting_window.set_current_version(current_version.into());
                        setting_window.set_latest_version(latest_version.into());
                        if let Some(update_info) = update_info {
                            setting_window.set_update_info(update_info.into());
                        }
                        setting_window.set_update_state(1);
                        setting_window.set_block(false);
                    }).unwrap_or_else(|e| log_util::log_error(format!("Failed to check update: {:?}", e)));
                });
            });

            let setting_win_clone = setting_win.as_weak();
            let msg_sender = msg_sender.clone();
            setting_win.on_update(move || {
                if let Some(setting_win) = setting_win_clone.upgrade() {
                    setting_win.set_block(true);
                    setting_win.set_update_state(0);
                }

                let msg_sender = msg_sender.clone();
                let setting_win_clone = setting_win_clone.clone();
                std::thread::spawn(move || {
                    let updater = Updater::global().lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());

                    match updater.update_software() {
                        Ok(_) => { let _ = msg_sender.send(AppMessage::Quit); },
                        Err(e) => {
                            log_util::log_error(format!("Failed to update software: {:?}", e));
                            setting_win_clone.upgrade_in_event_loop(move |setting_window| {
                                setting_window.set_block(false);
                                setting_window.set_update_state(2);
                            }).unwrap_or_else(|e| log_util::log_error(format!("Set setting_window back from updating: {:?}", e)));
                        }
                    }
                });
            });
        }

        { // logo
            setting_win.on_click_logo(move || {
                file_util::open_file("https://github.com/Horbin-Magician/rotor".to_string())
                    .unwrap_or_else(|e| log_util::log_error(format!("Failed to open link: {:?}", e)));
            });
        }

        Ok(Setting {
            setting_win
        })
    }
}