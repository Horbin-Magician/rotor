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
    pub fn new( msg_sender: Sender<AppMessage> ) -> SystemTray {
        let tray_menu = Menu::new();
        let menuitem_setting = MenuItem::new("设置", true, None); // TODO wait to translate
        let menuitem_quit = MenuItem::new("退出", true, None); // TODO wait to translate
        tray_menu.append(&menuitem_setting).unwrap();
        tray_menu.append(&menuitem_quit).unwrap();
        
        let _tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("小云管家") // TODO wait to translate
            .with_icon(Icon::from_path("assets/logo.ico", Some((128, 128))).unwrap())
            .build()
            .unwrap();

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
                                        msg_sender.send(AppMessage::ShowSetting).unwrap();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    recv(MenuEvent::receiver()) -> event => {
                        if let Ok(event) = event {
                            if event.id == _setting_id {
                                msg_sender.send(AppMessage::ShowSetting).unwrap();
                            } else if event.id == _quit_id {
                                msg_sender.send(AppMessage::Quit).unwrap();
                            }
                        }
                    }
                }
            }
        });

        SystemTray {
            _tray_icon
        }
    }
}