use std::sync::mpsc;

use slint::ComponentHandle;

use crate::util::system_tray::SystemTray;
use crate::module::searcher::SearchWindow;
use crate::setting::SettingWindow;

pub enum AppMessage {
    Quit,
    ShowSearch,
    ShowSetting,
}

pub struct Application {
    _system_tray: SystemTray,
    _msg_sender:mpsc::Sender<AppMessage>,
}

impl Application {
    pub fn new(setting: slint::Weak<SettingWindow>, searcher: slint::Weak<SearchWindow>) -> Self {
        let (_msg_sender, msg_reciever) = mpsc::channel();
        let _system_tray = SystemTray::new(_msg_sender.clone());

        std::thread::spawn(move || {
            app_loop(msg_reciever, setting, searcher);
        });

        Self {
            _system_tray,
            _msg_sender,
        }
    }

    pub fn _get_sender(&self) -> mpsc::Sender<AppMessage> {
        self._msg_sender.clone()
    }
}

fn app_loop (msg_reciever: mpsc::Receiver<AppMessage>, setting: slint::Weak<SettingWindow>, searcher: slint::Weak<SearchWindow>) {
    loop {
        match msg_reciever.try_recv() {
            Ok(AppMessage::Quit) => {
                slint::quit_event_loop().unwrap();
                break;
            },
            Ok(AppMessage::ShowSearch) => {
                searcher.clone().upgrade_in_event_loop(move |setting| {
                    setting.show().unwrap();
                }).unwrap();
            },
            Ok(AppMessage::ShowSetting) => {
                setting.clone().upgrade_in_event_loop(move |setting| {
                    setting.show().unwrap();
                }).unwrap();
            },
            Err(_) => {}
        }
    }
}