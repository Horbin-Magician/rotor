use anyhow::Result;
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

#[derive(Debug, Clone, Copy)]
pub enum Event {
    Show,
    Masks,
    Quit,
}

/// Constructed and dropped on the GPUI main thread. OS callbacks wake a bounded
/// async receiver; repeated identical events may coalesce while it is busy.
pub struct Services {
    manager: GlobalHotKeyManager,
    hotkey: HotKey,
    _tray: TrayIcon,
}

impl Services {
    pub fn new(tx: async_channel::Sender<Event>, quitting: Arc<AtomicBool>) -> Result<Self> {
        let menu = Menu::new();
        let show = MenuItem::new("显示 P0 原型", true, None);
        let masks = MenuItem::new("每屏遮罩", true, None);
        let quit = MenuItem::new("退出原型", true, None);
        menu.append_items(&[&show, &masks, &quit])?;
        let show_id = show.id().clone();
        let masks_id = masks.id().clone();
        let quit_id = quit.id().clone();
        let menu_tx = tx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let event = if event.id == show_id {
                Event::Show
            } else if event.id == masks_id {
                Event::Masks
            } else if event.id == quit_id {
                // Even a full wake queue must not lose explicit exit intent.
                quitting.store(true, Ordering::Relaxed);
                Event::Quit
            } else {
                return;
            };
            if let Err(error) = menu_tx.try_send(event) {
                eprintln!("menu event delivery: {error}");
            }
        }));
        let icon = tray_icon::Icon::from_rgba([40, 170, 230, 255].repeat(32 * 32), 32, 32)?;
        let tray = TrayIconBuilder::new()
            .with_tooltip("Rotor GPUI P0 (isolated)")
            .with_menu(Box::new(menu))
            .with_icon(icon)
            .build()?;
        let manager = GlobalHotKeyManager::new()?;
        let hotkey = HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT),
            Code::KeyG,
        );
        let id = hotkey.id();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.id == id
                && event.state == HotKeyState::Pressed
                && let Err(error) = tx.try_send(Event::Show)
            {
                eprintln!("hotkey delivery: {error}");
            }
        }));
        manager.register(hotkey)?;
        Ok(Self {
            manager,
            hotkey,
            _tray: tray,
        })
    }
}

impl Drop for Services {
    fn drop(&mut self) {
        let _ = self.manager.unregister(self.hotkey);
        GlobalHotKeyEvent::set_event_handler(None::<fn(GlobalHotKeyEvent)>);
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
    }
}
