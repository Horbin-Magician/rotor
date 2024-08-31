pub mod setting;
pub mod searcher;
pub mod screen_shotter;

use std::sync::mpsc::Sender;
use global_hotkey::hotkey::HotKey;


pub enum ModuleMessage {
    Trigger,
}

pub trait Module {
    fn flag(&self) -> &str;
    fn run(&self) -> Sender<ModuleMessage>;
    fn get_hotkey(&mut self) -> Option<HotKey>;
    fn clean(&self);
}