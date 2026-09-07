use async_channel::{Receiver, Sender};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::{Code, HotKey, Modifiers},
};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

const SHOW: u8 = 1;
const QUIT: u8 = 2;

#[derive(Clone, Copy)]
pub enum Command {
    ShowSettings,
    Quit,
}

/// Coalesced wakeups retain activation/exit intent even if the wake queue fills.
#[derive(Clone)]
pub struct CommandBus {
    wake: Sender<()>,
    pending: Arc<AtomicU8>,
}

impl CommandBus {
    pub fn new() -> (Self, Receiver<()>) {
        let (wake, receiver) = async_channel::bounded(1);
        (
            Self {
                wake,
                pending: Arc::new(AtomicU8::new(0)),
            },
            receiver,
        )
    }
    pub fn request(&self, command: Command) {
        self.pending.fetch_or(
            match command {
                Command::ShowSettings => SHOW,
                Command::Quit => QUIT,
            },
            Ordering::Release,
        );
        let _ = self.wake.try_send(());
    }
    pub fn take(&self) -> Option<Command> {
        let pending = self.pending.swap(0, Ordering::AcqRel);
        if pending & QUIT != 0 {
            Some(Command::Quit)
        } else if pending & SHOW != 0 {
            Some(Command::ShowSettings)
        } else {
            None
        }
    }
    pub fn close(&self) {
        self.wake.close();
    }
}

pub struct SystemServices {
    tray: TrayIcon,
    hotkeys: GlobalHotKeyManager,
    settings_hotkey: Option<HotKey>,
    pub warning: Option<String>,
}

impl SystemServices {
    pub fn new(
        commands: CommandBus,
        config: &rotor_common::Config,
        enable_hotkey: bool,
    ) -> Result<Self, String> {
        let icon =
            image::load_from_memory(include_bytes!("../../../src-tauri/assets/icons/32x32.png"))
                .map_err(|error| error.to_string())?
                .into_rgba8();
        let (width, height) = icon.dimensions();
        let icon = tray_icon::Icon::from_rgba(icon.into_raw(), width, height)
            .map_err(|error| error.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Rotor（开发版）")
            .build()
            .map_err(|error| error.to_string())?;
        let manager = GlobalHotKeyManager::new().map_err(|error| error.to_string())?;
        let hotkey = HotKey::new(
            Some(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SHIFT),
            Code::KeyG,
        );
        let hotkey_bus = commands.clone();
        let id = hotkey.id();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            if event.id == id && event.state == HotKeyState::Pressed {
                hotkey_bus.request(Command::ShowSettings);
            }
        }));
        let mut warning = None;
        let settings_hotkey = if enable_hotkey {
            match manager.register(hotkey) {
                Ok(()) => Some(hotkey),
                Err(error) => {
                    warning = Some(format!("Ctrl+Alt+Shift+G: {error}"));
                    None
                }
            }
        } else {
            None
        };
        let mut services = Self {
            tray,
            hotkeys: manager,
            settings_hotkey,
            warning,
        };
        services.update_menu(commands, config)?;
        Ok(services)
    }

    pub fn update_menu(
        &mut self,
        commands: CommandBus,
        config: &rotor_common::Config,
    ) -> Result<(), String> {
        let menu = Menu::new();
        let chinese = rotor_common::i18n::language_for_config(config) == "zh-CN";
        let settings = MenuItem::new(if chinese { "设置" } else { "Settings" }, true, None);
        let quit = MenuItem::new(if chinese { "退出" } else { "Quit" }, true, None);
        menu.append_items(&[&settings, &quit])
            .map_err(|error| error.to_string())?;
        let settings_id = settings.id().clone();
        let quit_id = quit.id().clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == settings_id {
                commands.request(Command::ShowSettings);
            } else if event.id == quit_id {
                commands.request(Command::Quit);
            }
        }));
        self.tray.set_menu(Some(Box::new(menu)));
        Ok(())
    }

    pub fn stop_events(&mut self) {
        if let Some(hotkey) = self.settings_hotkey.take() {
            let _ = self.hotkeys.unregister(hotkey);
        }
        GlobalHotKeyEvent::set_event_handler(None::<fn(GlobalHotKeyEvent)>);
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
    }
}

impl Drop for SystemServices {
    fn drop(&mut self) {
        self.stop_events();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_is_not_lost_when_activation_wakeups_are_coalesced() {
        let (bus, receiver) = CommandBus::new();
        for _ in 0..100 {
            bus.request(Command::ShowSettings);
        }
        bus.request(Command::Quit);
        assert_eq!(receiver.len(), 1);
        assert!(matches!(bus.take(), Some(Command::Quit)));
        assert!(bus.take().is_none());
    }
}
