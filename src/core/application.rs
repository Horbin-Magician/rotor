pub mod setting;
mod system_tray;

use std::sync::mpsc;
use std::collections::HashMap;
use crossbeam::channel::{unbounded, Receiver, Sender};

use slint::ComponentHandle;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use system_tray::SystemTray;
use setting::{Setting, SettingWindow};
use crate::module::{searcher::Searcher, Module, ModuleMessage};
use crate::module::screen_shotter::ScreenShotter;

pub enum AppMessage {
    Quit,
    ShowSetting,
}

pub struct Application {
    _system_tray: SystemTray,
    _setting: Setting,
    _modules: Vec<Box<dyn Module>>,
    _msg_sender: Sender<AppMessage>,
    _hotkey_manager: GlobalHotKeyManager,
}

impl Application {
    pub fn new() -> Application {
        let (_msg_sender, msg_receiver) = unbounded();
        let _hotkey_manager = GlobalHotKeyManager::new().unwrap(); // initialize the hotkeys manager

        let _system_tray = SystemTray::new(_msg_sender.clone());
        let _setting: Setting = Setting::new();

        let mut _modules: Vec<Box<dyn Module>> = Vec::new();
        _modules.push(Box::new(Searcher::new()));
        _modules.push(Box::new(ScreenShotter::new()));
        let mut module_ports: HashMap<u32, mpsc::Sender<ModuleMessage>> = HashMap::new();
        for module in &mut _modules {
            _hotkey_manager.register(module.get_hotkey()).expect("Failed to register hotkey."); // register it
            module_ports.insert(module.get_id().unwrap(), module.run());
        }

        let setting_win = _setting.setting_win.as_weak();
        std::thread::spawn(move || {
            app_loop(
                msg_receiver,
                setting_win,
                module_ports,
            );
        });

        Application {
            _system_tray,
            _setting,
            _modules,
            _msg_sender,
            _hotkey_manager,
        }
    }
}

fn app_loop (
    msg_receiver: Receiver<AppMessage>,
    setting_win: slint::Weak<SettingWindow>,
    module_ports: HashMap<u32, mpsc::Sender<ModuleMessage>>,
) {
    loop {
        crossbeam::select! {
            recv(GlobalHotKeyEvent::receiver()) -> event => {
                let event = event.unwrap();
                for module_port in &module_ports {
                    if event.state == HotKeyState::Released && event.id == *module_port.0 {
                        module_port.1.send(ModuleMessage::Trigger).unwrap();
                    }
                }
            }
            recv(&msg_receiver) -> msg => {
                match msg.unwrap() {
                    AppMessage::Quit => {
                        slint::quit_event_loop().unwrap();
                        break;
                    },
                    AppMessage::ShowSetting => {
                        setting_win.clone().upgrade_in_event_loop(move |win| {
                            win.show().unwrap();
                        }).unwrap();
                    },
                }
            },
        }
    }
}