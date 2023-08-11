use tray_icon::{TrayIconBuilder, menu::Menu, menu::MenuItem, menu::PredefinedMenuItem, Icon, TrayIconEvent, menu::MenuEvent, TrayIcon};

pub struct SystemTray {
    _tray_icon: TrayIcon,
}

impl SystemTray {
    pub fn new() -> SystemTray {
        let tray_menu = Menu::new();
        let menuitem_setting = MenuItem::new("设置", true, None);
        let menuitem_exit = PredefinedMenuItem::quit(Some("退出"));
        tray_menu.append(&menuitem_setting).unwrap();
        tray_menu.append(&menuitem_exit).unwrap();
        
        let _tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("system-tray - tray icon library!")
            .with_icon(Icon::from_path("assets/favicon.ico", Some((128, 128))).unwrap())
            .build()
            .unwrap();
    
        TrayIconEvent::set_event_handler(Some(
            |event: TrayIconEvent| {
                // TODO 左键打开设置
                println!("tray event: {:?}", event);
            }
        ));
    
        MenuEvent::set_event_handler(Some(
            |event: MenuEvent| {
                // TODO 菜单事件
                println!("menu event: {:?}", event);
            }
        ));

        SystemTray {
            _tray_icon
        }
    }
}