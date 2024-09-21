use std::error::Error;
use crossbeam::channel::Sender;
use tray_icon::{
    menu::{Menu, MenuEvent},
    TrayIconBuilder, menu::MenuItem, Icon, TrayIconEvent, TrayIcon
};

use super::AppMessage;


pub struct SystemTray {
    _tray_icon: TrayIcon,
}

impl SystemTray {
    pub fn new( msg_sender: Sender<AppMessage> ) -> Result<SystemTray, Box<dyn Error>> {
        let tray_menu = Menu::new();
        let menuitem_setting = MenuItem::new("设置", true, None); // TODO wait to translate
        let menuitem_quit = MenuItem::new("退出", true, None); // TODO wait to translate
        tray_menu.append(&menuitem_setting)?;
        tray_menu.append(&menuitem_quit)?;
        
        let _tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("小云管家") // TODO wait to translate
            .with_icon(Icon::from_path("assets/logo.ico", Some((128, 128)))?)
            .build()?;

        let _setting_id = menuitem_setting.id().clone();
        let _quit_id = menuitem_quit.id().clone();
        std::thread::spawn(move || {
            loop {
                crossbeam::select! {
                    recv(TrayIconEvent::receiver()) -> event => {
                        if let Ok(event) = event {
                            match event {
                                TrayIconEvent::Click { id:_, position:_, rect:_, button, button_state } => {
                                    if button == tray_icon::MouseButton::Left && button_state == tray_icon::MouseButtonState::Up {
                                        let _ = msg_sender.send(AppMessage::ShowSetting);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    recv(MenuEvent::receiver()) -> event => {
                        if let Ok(event) = event {
                            if event.id == _setting_id {
                                let _ = msg_sender.send(AppMessage::ShowSetting);
                            } else if event.id == _quit_id {
                                let _ = msg_sender.send(AppMessage::Quit);
                            }
                        }
                    }
                }
            }
        });

        Ok(SystemTray {_tray_icon})
    }
}