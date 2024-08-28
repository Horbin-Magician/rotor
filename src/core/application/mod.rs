pub mod app_config;
mod system_tray;

use std::sync::{mpsc, Arc, Mutex};
use std::collections::HashMap;
use crossbeam::channel::{unbounded, Receiver, Sender};
use global_hotkey::hotkey::HotKey;
use slint;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::module::{setting::Setting, screen_shotter::ScreenShotter, searcher::Searcher, Module, ModuleMessage};
use system_tray::SystemTray;

pub enum AppMessage {
    Quit,
    ShowSetting,
    ChangeHotkey(String, String),
}

pub struct Application {
    _system_tray: SystemTray,
    _modules: Vec<Box<dyn Module>>,
    _msg_sender: Sender<AppMessage>,
}

impl Application {
    pub fn new() -> Application {
        let (_msg_sender, msg_receiver) = unbounded();
        let hotkey_manager_rc: Arc<Mutex<GlobalHotKeyManager>> = Arc::new(Mutex::new(GlobalHotKeyManager::new().unwrap())); // initialize the hotkeys manager

        let _system_tray = SystemTray::new(_msg_sender.clone());

        let mut _modules: Vec<Box<dyn Module>> = Vec::new();
        _modules.push(Box::new(Searcher::new()));
        _modules.push(Box::new(ScreenShotter::new()));
        _modules.push(Box::new(Setting::new(_msg_sender.clone())));

        let mut module_profiles: HashMap<String, (Option<HotKey>, mpsc::Sender<ModuleMessage>)> = HashMap::new();
        for module in &mut _modules {
            let hotkey = module.get_hotkey();
            module_profiles.insert(module.flag().to_string(), (hotkey, module.run()));
        }

        std::thread::spawn(move || {
            app_loop(
                hotkey_manager_rc,
                msg_receiver,
                module_profiles,
            );
        });

        Application {
            _system_tray,
            _modules,
            _msg_sender,
        }
    }
}

fn app_loop (
    hotkey_manager_rc: Arc<Mutex<GlobalHotKeyManager>>,
    msg_receiver: Receiver<AppMessage>,
    mut module_profiles: HashMap<String, (Option<HotKey>, mpsc::Sender<ModuleMessage>)>,
) {
    let mut module_ports: HashMap<u32, mpsc::Sender<ModuleMessage>> = HashMap::new();
    let cloned_module_profiles = module_profiles.clone();
    for (_, (hotkey, runner)) in cloned_module_profiles {
        if let Some(hotkey) = hotkey {
            let hotkey_clone = hotkey.clone();
            let hotkey_manager_rc_clone = hotkey_manager_rc.clone();
            slint::invoke_from_event_loop(move || {
                let hotkey_manager = hotkey_manager_rc_clone.lock().unwrap();
                hotkey_manager.register(hotkey_clone).expect("Error in register hotkey"); // TODO deal with the error
            }).unwrap();
            module_ports.insert(hotkey.id(), runner.clone());
        }
    }

    loop {
        crossbeam::select! {
            recv(GlobalHotKeyEvent::receiver()) -> event => {
                let event = event.unwrap();
                if event.state == HotKeyState::Pressed {
                    module_ports[&event.id].send(ModuleMessage::Trigger).unwrap();
                }
            }
            recv(&msg_receiver) -> msg => {
                match msg.unwrap() {
                    AppMessage::Quit => {
                        slint::quit_event_loop().unwrap();
                        break;
                    },
                    AppMessage::ShowSetting => {
                        module_profiles.get("setting").unwrap().1.send(ModuleMessage::Trigger).unwrap();
                    },
                    AppMessage::ChangeHotkey(key, value) => {
                        if let Some((Some(hotkey), runner)) = module_profiles.remove(&key) {
                            let msg_sender = module_ports.remove(&hotkey.id()).unwrap();
                            
                            let hotkey_manager_rc_clone = hotkey_manager_rc.clone();
                            slint::invoke_from_event_loop(move || {
                                let hotkey_manager = hotkey_manager_rc_clone.lock().unwrap();
                                hotkey_manager.unregister(hotkey).expect("Error in unregister hotkey"); // TODO deal with the error
                            }).unwrap();

                            let hotkey = value.parse::<HotKey>().unwrap();
                            let hotkey_manager_rc_clone = hotkey_manager_rc.clone();
                            slint::invoke_from_event_loop(move || {
                                let hotkey_manager = hotkey_manager_rc_clone.lock().unwrap();
                                hotkey_manager.register(hotkey).expect("Error in register hotkey"); // TODO deal with the error
                            }).unwrap();
                            
                            module_profiles.insert(key, (Some(hotkey), runner));
                            module_ports.insert(hotkey.id(), msg_sender);
                        }
                    }
                }
            },
        }
    }
}