mod system_tray;
mod setting;

use std::sync::mpsc;

use slint::ComponentHandle;

use crate::application::system_tray::SystemTray;
use crate::application::setting::{Setting, SettingWindow};
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
}

impl Application {
    pub fn new() -> Self {
        let (_msg_sender, msg_reciever) = mpsc::channel();

        let _system_tray = SystemTray::new(_msg_sender.clone());
        let _searcher: Searcher = Searcher::new();
        let _setting: Setting = Setting::new();

        let setting_win = _setting.setting_win.as_weak();
        let search_win = _searcher.search_win.as_weak();
        std::thread::spawn(move || {
            app_loop(msg_reciever, setting_win, search_win);
        });

        Self {
            _system_tray,
            _setting,
            _searcher,
            _msg_sender,
        }
    }

    pub fn _get_sender(&self) -> mpsc::Sender<AppMessage> {
        self._msg_sender.clone()
    }
}

fn app_loop (msg_reciever: mpsc::Receiver<AppMessage>, setting_win: slint::Weak<SettingWindow>, search_win: slint::Weak<SearchWindow>) {
    loop {
        match msg_reciever.try_recv() {
            Ok(AppMessage::Quit) => {
                slint::quit_event_loop().unwrap();
                break;
            },
            Ok(AppMessage::ShowSearch) => {
                search_win.clone().upgrade_in_event_loop(move |setting| {
                    setting.show().unwrap();
                }).unwrap();
            },
            Ok(AppMessage::ShowSetting) => {
                setting_win.clone().upgrade_in_event_loop(move |setting| {
                    setting.show().unwrap();
                }).unwrap();
            },
            Err(_) => {}
        }
    }
}