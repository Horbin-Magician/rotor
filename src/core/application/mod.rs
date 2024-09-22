pub mod app_config;
mod system_tray;

use std::error::Error;
use std::sync::{mpsc, Arc, Mutex};
use std::collections::HashMap;
use crossbeam::channel::{unbounded, Receiver, RecvError, Sender};
use global_hotkey::hotkey::HotKey;
use slint;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

use crate::util::log_util;
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
    pub fn new() -> Result<Application, Box<dyn std::error::Error>> {
        let (_msg_sender, _msg_receiver) = unbounded();
        let _hotkey_manager_rc: Arc<Mutex<GlobalHotKeyManager>> = Arc::new(Mutex::new(GlobalHotKeyManager::new()?)); // initialize the hotkeys manager

        let _system_tray = SystemTray::new(_msg_sender.clone())?;

        let mut _modules: Vec<Box<dyn Module>> = Vec::new();
        if let Ok(searcher) = Searcher::new() {
            _modules.push(Box::new(searcher));
        }
        if let Ok(screen_shotter) = ScreenShotter::new() {
            _modules.push(Box::new(screen_shotter));
        }
        if let Ok(setting) = Setting::new(_msg_sender.clone()) {
            _modules.push(Box::new(setting));
        }

        let mut _module_profiles: HashMap<String, (Option<HotKey>, mpsc::Sender<ModuleMessage>)> = HashMap::new();
        for module in &mut _modules {
            let hotkey = module.get_hotkey();
            _module_profiles.insert(module.flag().to_string(), (hotkey, module.run()));
        }

        Ok(Application {
            _is_running: Arc::new(Mutex::new(true)),
            _system_tray,
            _modules,
            _msg_sender,
            _msg_receiver,
            _hotkey_manager_rc,
            _module_profiles,
        })
    }
    
    pub fn run(&mut self) {
        let mut module_ports: HashMap<u32, mpsc::Sender<ModuleMessage>> = HashMap::new();
        let cloned_module_profiles = self._module_profiles.clone();
        for (_, (hotkey, runner)) in cloned_module_profiles {
            if let Some(hotkey) = hotkey {
                let hotkey_clone = hotkey.clone();
                let hotkey_manager_rc_clone = self._hotkey_manager_rc.clone();
                slint::invoke_from_event_loop(move || {
                    if let Ok(hotkey_manager) = hotkey_manager_rc_clone.lock() {
                        hotkey_manager.register(hotkey_clone)
                            .unwrap_or_else(|e| log_util::log_error(format!("Error in register hotkey: {:?}", e)));
                    }
                }).unwrap_or_else(|e| log_util::log_error(format!("Error in invoke_from_event_loop: {:?}", e)));
                module_ports.insert(hotkey.id(), runner.clone());
            }
        }

        fn handle_global_hotkey_event(
            event: Result<GlobalHotKeyEvent, RecvError>,
            module_ports: &HashMap<u32, mpsc::Sender<ModuleMessage>>
        ) {
            match event {
                Ok(event) => {
                    if event.state == HotKeyState::Pressed {
                        if let Some(sender) = module_ports.get(&event.id) {
                            let _ = sender.send(ModuleMessage::Trigger);
                        } else {
                            log_util::log_error("No sender found for the event ID".to_string());
                        }
                    }
                },
                Err(e) => {log_util::log_error(format!("Failed to receive GlobalHotKeyEvent: {:?}", e));}
            }
        }

        fn handle_quit(is_running: Arc<Mutex<bool>>) {
            if let Ok(mut is_running) = is_running.lock() {
                *is_running = false;
                if let Err(e) = slint::quit_event_loop() {
                    log_util::log_error(format!("Failed to quit event loop: {:?}", e));
                }
            } else {
                log_util::log_error("Failed to lock is_running mutex".to_string());
            }
        }

        fn handle_show_setting(module_profiles: &HashMap<String, (Option<HotKey>, mpsc::Sender<ModuleMessage>)>) {
            if let Some((_, sender)) = module_profiles.get("setting") {
                let _ = sender.send(ModuleMessage::Trigger);
            } else {
                log_util::log_error("No sender found for setting".to_string());
            }
        }

        fn handle_change_hotkey(
            key: String,
            value: String,
            module_profiles: &mut HashMap<String, (Option<HotKey>, mpsc::Sender<ModuleMessage>)>,
            module_ports: &mut HashMap<u32, mpsc::Sender<ModuleMessage>>,
            hotkey_manager_rc: &Arc<Mutex<GlobalHotKeyManager>>,
        ) -> Result<(), Box<dyn Error>> {
            if let Some((Some(hotkey), runner)) = module_profiles.remove(&key) {
                let msg_sender = module_ports.remove(&hotkey.id())
                    .ok_or("No sender found for the hotkey")?;
                
                let hotkey_manager_rc_clone = hotkey_manager_rc.clone();
                slint::invoke_from_event_loop(move || {
                    if let Ok(hotkey_manager) = hotkey_manager_rc_clone.lock() {
                        hotkey_manager.unregister(hotkey)
                            .unwrap_or_else(|e| log_util::log_error(format!("Error in unregister hotkey: {:?}", e)));
                    } else {
                        log_util::log_error("Failed to lock hotkey manager".to_string());
                    }
                }).unwrap_or_else(|e| log_util::log_error(format!("Error in invoke_from_event_loop: {:?}", e)));

                let hotkey = value.parse::<HotKey>()?;
                let hotkey_manager_rc_clone = hotkey_manager_rc.clone();
                slint::invoke_from_event_loop(move || {
                    if let Ok(hotkey_manager) = hotkey_manager_rc_clone.lock() {
                        hotkey_manager.register(hotkey)
                            .unwrap_or_else(|e| log_util::log_error(format!("Error in register hotkey: {:?}", e)));
                    } else {
                        log_util::log_error("Failed to lock hotkey manager".to_string());
                    }
                }).unwrap_or_else(|e| log_util::log_error(format!("Error in invoke_from_event_loop: {:?}", e)));
                
                module_profiles.insert(key, (Some(hotkey), runner));
                module_ports.insert(hotkey.id(), msg_sender);
            }
            Ok(())
        }

        let is_running = self._is_running.clone();
        let msg_receiver = self._msg_receiver.clone();
        let mut module_profiles = self._module_profiles.clone();
        let hotkey_manager_rc = self._hotkey_manager_rc.clone();
        std::thread::spawn(move || {
            loop {
                crossbeam::select! {
                    recv(GlobalHotKeyEvent::receiver()) -> event => {
                        handle_global_hotkey_event(event, &module_ports);
                    }
                    recv(msg_receiver) -> msg => {
                        match msg {
                            Ok(msg) => {
                                match msg {
                                    AppMessage::Quit => {
                                        handle_quit(is_running.clone());
                                        break;
                                    },
                                    AppMessage::ShowSetting => {
                                        handle_show_setting(&module_profiles)
                                    },
                                    AppMessage::ChangeHotkey(key, value) => {
                                        handle_change_hotkey(
                                            key,
                                            value,
                                            &mut module_profiles,
                                            &mut module_ports,
                                            &hotkey_manager_rc
                                        ).unwrap_or_else(|e| log_util::log_error(format!("Failed to change hotkey: {:?}", e)));
                                    }
                                }
                            },
                            Err(e) => {log_util::log_error(format!("Failed to receive message: {:?}", e));}
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
        if let Ok(is_running) = self._is_running.lock() {
            return *is_running;
        } else { false }
    }
}