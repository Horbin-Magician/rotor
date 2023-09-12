use std::sync::mpsc;

use tray_icon::{
    menu::{Menu, MenuEvent},
    ClickType, TrayIconBuilder, menu::MenuItem, Icon, TrayIconEvent, TrayIcon
};

use super::AppMessage;

pub struct SystemTray {
    _tray_icon: TrayIcon,
}

impl SystemTray {
    pub fn new( msg_sender:mpsc::Sender<AppMessage> ) -> SystemTray {
        let tray_menu = Menu::new();
        let menuitem_setting = MenuItem::new("设置", true, None);
        let menuitem_quit = MenuItem::new("退出", true, None);
        tray_menu.append(&menuitem_setting).unwrap();
        tray_menu.append(&menuitem_quit).unwrap();
        
        let _tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("system-tray - tray icon library!")
            .with_icon(Icon::from_path("assets/logo.ico", Some((128, 128))).unwrap())
            .build()
            .unwrap();

        let _setting_id = menuitem_setting.id().clone();
        let _quit_id = menuitem_quit.id().clone();
        std::thread::spawn(move || {
            loop {
                if let Ok(event) = TrayIconEvent::receiver().try_recv() {
                    if event.click_type == ClickType::Left {
                        msg_sender.send(AppMessage::ShowSetting).unwrap();
                    }
                }

                if let Ok(event) = MenuEvent::receiver().try_recv() {
                    if event.id == _setting_id {
                        msg_sender.send(AppMessage::ShowSetting).unwrap();
                    } else if event.id == _quit_id {
                        msg_sender.send(AppMessage::Quit).unwrap();
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        });

        SystemTray { _tray_icon }
    }
}