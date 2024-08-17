use crossbeam::channel::Sender;

use tray_icon::{
    menu::{Menu, MenuEvent},
    ClickType, TrayIconBuilder, menu::MenuItem, Icon, TrayIconEvent, TrayIcon
};

use super::AppMessage;

pub struct SystemTray {
    _tray_icon: TrayIcon,
}

impl SystemTray {
    pub fn new( msg_sender: Sender<AppMessage> ) -> SystemTray {
        let tray_menu = Menu::new();
        let menuitem_setting = MenuItem::new("设置", true, None);
        let menuitem_quit = MenuItem::new("退出", true, None);
        tray_menu.append(&menuitem_setting).unwrap();
        tray_menu.append(&menuitem_quit).unwrap();
        
        let _tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("小云管家")
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
                            if event.click_type == ClickType::Left {
                                msg_sender.send(AppMessage::ShowSetting).unwrap();
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