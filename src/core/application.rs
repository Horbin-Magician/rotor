mod system_tray;
mod setting;
pub mod powerboot;
pub mod admin_runner;

use std::{sync::{mpsc, mpsc::Sender}, collections::HashMap};

use slint::ComponentHandle;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};

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
    _msg_sender:mpsc::Sender<AppMessage>,
    _hotkey_manager: GlobalHotKeyManager,
}

impl Application {
    pub fn new() -> Application {
        let (_msg_sender, msg_reciever) = mpsc::channel();

        let _system_tray = SystemTray::new(_msg_sender.clone());
        let _setting: Setting = Setting::new();
        let _hotkey_manager = GlobalHotKeyManager::new().unwrap(); // initialize the hotkeys manager
        
        let mut _modules: Vec<Box<dyn Module>> = Vec::new();
        _modules.push(Box::new(Searcher::new()));
        _modules.push(Box::new(ScreenShotter::new()));
        let mut module_ports: HashMap<u32, Sender<ModuleMessage>> = HashMap::new();
        for module in &mut _modules {
            _hotkey_manager.register(module.get_hotkey()).unwrap(); // register it
            module_ports.insert(module.get_id().unwrap(), module.run());
        }

        let setting_win = _setting.setting_win.as_weak();
        std::thread::spawn(move || {
            app_loop(
                msg_reciever,
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
    msg_reciever: mpsc::Receiver<AppMessage>,
    setting_win: slint::Weak<SettingWindow>,
    module_ports: HashMap<u32, Sender<ModuleMessage>>,
) {
    loop {
        match msg_reciever.try_recv() {
            Ok(AppMessage::Quit) => {
                slint::quit_event_loop().unwrap();
                break;
            },
            Ok(AppMessage::ShowSetting) => {
                setting_win.clone().upgrade_in_event_loop(move |win| {
                    win.show().unwrap();
                }).unwrap();
            },
            Err(_) => {}
        }

        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            for module_port in &module_ports {
                if event.id == *module_port.0 {
                    module_port.1.send(ModuleMessage::Trigger).unwrap();
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}