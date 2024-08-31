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
    _is_running: Arc<Mutex<bool>>,
    _system_tray: SystemTray,
    _modules: Vec<Box<dyn Module>>,
    _msg_sender: Sender<AppMessage>,
    _msg_receiver: Receiver<AppMessage>,
    _hotkey_manager_rc: Arc<Mutex<GlobalHotKeyManager>>,
    _module_profiles: HashMap<String, (Option<HotKey>, mpsc::Sender<ModuleMessage>)>,
}

impl Application {
    pub fn new() -> Application {
        let (_msg_sender, _msg_receiver) = unbounded();
        let _hotkey_manager_rc: Arc<Mutex<GlobalHotKeyManager>> = Arc::new(Mutex::new(GlobalHotKeyManager::new().unwrap())); // initialize the hotkeys manager

        let _system_tray = SystemTray::new(_msg_sender.clone());

        let mut _modules: Vec<Box<dyn Module>> = Vec::new();
        _modules.push(Box::new(Searcher::new()));
        _modules.push(Box::new(ScreenShotter::new()));
        _modules.push(Box::new(Setting::new(_msg_sender.clone())));

        let mut _module_profiles: HashMap<String, (Option<HotKey>, mpsc::Sender<ModuleMessage>)> = HashMap::new();
        for module in &mut _modules {
            let hotkey = module.get_hotkey();
            _module_profiles.insert(module.flag().to_string(), (hotkey, module.run()));
        }

        Application {
            _is_running: Arc::new(Mutex::new(true)),
            _system_tray,
            _modules,
            _msg_sender,
            _msg_receiver,
            _hotkey_manager_rc,
            _module_profiles,
        }
    }
    
    pub fn run(&mut self) {
        let mut module_ports: HashMap<u32, mpsc::Sender<ModuleMessage>> = HashMap::new();
        let cloned_module_profiles = self._module_profiles.clone();
        for (_, (hotkey, runner)) in cloned_module_profiles {
            if let Some(hotkey) = hotkey {
                let hotkey_clone = hotkey.clone();
                let hotkey_manager_rc_clone = self._hotkey_manager_rc.clone();
                slint::invoke_from_event_loop(move || {
                    let hotkey_manager = hotkey_manager_rc_clone.lock().unwrap();
                    hotkey_manager.register(hotkey_clone).expect("Error in register hotkey"); // TODO deal with the error
                }).unwrap();
                module_ports.insert(hotkey.id(), runner.clone());
            }
        }

        let is_running = self._is_running.clone();
        let msg_receiver = self._msg_receiver.clone();
        let mut module_profiles = self._module_profiles.clone();
        let hotkey_manager_rc = self._hotkey_manager_rc.clone();
        std::thread::spawn(move || {
            loop {
                crossbeam::select! {
                    recv(GlobalHotKeyEvent::receiver()) -> event => {
                        let event = event.unwrap();
                        if event.state == HotKeyState::Pressed {
                            module_ports[&event.id].send(ModuleMessage::Trigger).unwrap();
                        }
                    }
                    recv(msg_receiver) -> msg => {
                        match msg.unwrap() {
                            AppMessage::Quit => {
                                *is_running.lock().unwrap() = false;
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
        });
    }
    
    pub fn clean(&mut self) {
        for module in &self._modules {
            module.clean();
        }
    }

    pub fn is_running(&self) -> bool {
        self._is_running.lock().unwrap().clone()
    }
}