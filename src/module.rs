pub mod searcher;
pub mod screen_shotter;
use std::sync::mpsc::Sender;

use global_hotkey::hotkey::HotKey;


pub enum ModuleMessage {
    Trigger,
}

pub trait Module {
    fn run(&self) -> Sender<ModuleMessage>;
    fn get_hotkey(&mut self) -> HotKey;
    fn get_id(&self) -> Option<u32>;
}