mod system_tray;
mod setting;
pub mod powerboot;
pub mod admin_runner;

use std::{sync::mpsc, rc::Rc};

use slint::ComponentHandle;
use i_slint_backend_winit::WinitWindowAccessor;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, hotkey::{HotKey, Modifiers, Code}};

use system_tray::SystemTray;
use setting::{Setting, SettingWindow};
use crate::module::searcher::{Searcher, SearchWindow};
use crate::module::screen_shotter::{ScreenShotter, MaskWindow};

pub enum AppMessage {
    Quit,
    ShowSearch,
    ShowSetting,
}

pub struct Application {
    _system_tray: SystemTray,
    _setting: Setting,
    _searcher: Searcher,
    _shotter: Rc<ScreenShotter>,
    _msg_sender:mpsc::Sender<AppMessage>,
    _hotkey_manager: GlobalHotKeyManager,
}

impl Application {
    pub fn new() -> Application {
        let (_msg_sender, msg_reciever) = mpsc::channel();

        let _system_tray = SystemTray::new(_msg_sender.clone());
        let _setting: Setting = Setting::new();
        let _searcher: Searcher = Searcher::new();
        let _shotter: Rc<ScreenShotter> = Rc::new(ScreenShotter::new());

        let _hotkey_manager = GlobalHotKeyManager::new().unwrap(); // initialize the hotkeys manager
        let find_hotkey = HotKey::new(Some(Modifiers::SHIFT), Code::KeyF); // construct the hotkey
        let shot_hotkey = HotKey::new(Some(Modifiers::SHIFT), Code::KeyC); // construct the hotkey
        _hotkey_manager.register(find_hotkey).unwrap(); // register it
        _hotkey_manager.register(shot_hotkey).unwrap(); // register it

        let setting_win = _setting.setting_win.as_weak();
        let search_win = _searcher.search_win.as_weak();
        let mask_win = _shotter.mask_win.as_weak();
        let find_hotkey_id = find_hotkey.id();
        let shot_hotkey_id = shot_hotkey.id();
        let msg_sender_clone = _msg_sender.clone();
        std::thread::spawn(move || {
            app_loop(
                msg_sender_clone,
                msg_reciever,
                setting_win,
                search_win,
                mask_win,
                find_hotkey_id,
                shot_hotkey_id,
            );
        });

        Application {
            _system_tray,
            _setting,
            _searcher,
            _shotter,
            _msg_sender,
            _hotkey_manager,
        }
    }

    // pub fn get_sender(&self) -> mpsc::Sender<AppMessage> {
    //     self._msg_sender.clone()
    // }
}

fn app_loop (
    msg_sender_clone: mpsc::Sender<AppMessage>,
    msg_reciever: mpsc::Receiver<AppMessage>,
    setting_win: slint::Weak<SettingWindow>,
    search_win: slint::Weak<SearchWindow>,
    mask_win: slint::Weak<MaskWindow>,
    find_hotkey_id: u32,
    shot_hotkey_id: u32,
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
            } else if event.id == shot_hotkey_id {
                mask_win.clone().upgrade_in_event_loop(move |win| {
                    win.invoke_shot();
                }).unwrap();
            }
        }
        
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}