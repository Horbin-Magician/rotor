mod system_tray;
mod setting;

use std::sync::mpsc;

use slint::ComponentHandle;
use i_slint_backend_winit::WinitWindowAccessor;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, hotkey::{HotKey, Modifiers, Code}};

use system_tray::SystemTray;
use setting::{Setting, SettingWindow};
use crate::module::searcher::{Searcher, SearchWindow};

pub enum AppMessage {
    Quit,
    ShowSearch,
    ShowSetting,
}

pub struct Application {
    _system_tray: SystemTray,
    _setting: Setting,
    _searcher: Searcher,
    _msg_sender:mpsc::Sender<AppMessage>,
    _hotkey_manager: GlobalHotKeyManager,
}

impl Application {
    pub fn new() -> Self {
        let (_msg_sender, msg_reciever) = mpsc::channel();

        let _system_tray = SystemTray::new(_msg_sender.clone());
        let _searcher: Searcher = Searcher::new();
        let _setting: Setting = Setting::new();

        let _hotkey_manager = GlobalHotKeyManager::new().unwrap(); // initialize the hotkeys manager
        let find_hotkey = HotKey::new(Some(Modifiers::SHIFT), Code::KeyF); // construct the hotkey
        _hotkey_manager.register(find_hotkey).unwrap(); // register it

        let setting_win = _setting.setting_win.as_weak();
        let search_win = _searcher.search_win.as_weak();
        let find_hotkey_id = find_hotkey.id();
        let msg_sender_clone = _msg_sender.clone();
        std::thread::spawn(move || {
            app_loop(
                msg_sender_clone,
                msg_reciever,
                setting_win,
                search_win,
                find_hotkey_id
            );
        });

        Self {
            _system_tray,
            _setting,
            _searcher,
            _msg_sender,
            _hotkey_manager,
        }
    }

    pub fn _get_sender(&self) -> mpsc::Sender<AppMessage> {
        self._msg_sender.clone()
    }
}

fn app_loop (
    msg_sender_clone: mpsc::Sender<AppMessage>,
    msg_reciever: mpsc::Receiver<AppMessage>,
    setting_win: slint::Weak<SettingWindow>,
    search_win: slint::Weak<SearchWindow>,
    find_hotkey_id: u32,
) {
    loop {
        match msg_reciever.try_recv() {
            Ok(AppMessage::Quit) => {
                slint::quit_event_loop().unwrap();
                break;
            },
            Ok(AppMessage::ShowSearch) => {
                search_win.clone().upgrade_in_event_loop(move |win| {
                    win.show().unwrap();
                    win.window().with_winit_window(|winit_win: &winit::window::Window| {
                        winit_win.focus_window();
                    });
                }).unwrap();
                
            },
            Ok(AppMessage::ShowSetting) => {
                setting_win.clone().upgrade_in_event_loop(move |win| {
                    win.show().unwrap();
                }).unwrap();
            },
            Err(_) => {}
        }

        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == find_hotkey_id {
                msg_sender_clone.send(AppMessage::ShowSearch).unwrap();
            }
        }
    }
}