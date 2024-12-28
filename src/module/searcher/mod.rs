mod file_data;
mod volume;

use slint::{ComponentHandle, Model};
use std::{error::Error, rc::Rc, sync::mpsc::{self, Sender}};
use i_slint_backend_winit::{winit::platform::windows::WindowExtWindows, WinitWindowAccessor};
use global_hotkey::hotkey::HotKey;
use xcap::Monitor;

use file_data::FileData;
use crate::{sys_util, util::log_util};
use crate::core::application::app_config::AppConfig;
use crate::util::file_util;
use crate::ui::SearchWindow;
use crate::module::{Module, ModuleMessage};

pub enum SearcherMessage {
    Init,
    Update,
    Find(String),
    Release,
}

pub struct Searcher {
    pub search_win: SearchWindow,
    searcher_msg_sender: mpsc::Sender<SearcherMessage>,
}

impl Module for Searcher{
    fn flag(&self) -> &str { "search" }

    fn run(&self) -> Sender<ModuleMessage> {
        let (msg_sender, msg_reciever) = mpsc::channel();
        let search_win_clone = self.search_win.as_weak();
        let searcher_msg_sender_clone = self.searcher_msg_sender.clone();
        std::thread::spawn(move || {
            loop {
                match msg_reciever.recv() {
                    Ok(ModuleMessage::Trigger) => {
                        let searcher_msg_sender_clone_clone = searcher_msg_sender_clone.clone();
                        search_win_clone.upgrade_in_event_loop(move |win| {
                            // Set window center
                            if let Ok(monitors) = Monitor::all() {
                                if let Some(primary_monitor) = monitors.iter().find(|m| m.is_primary()) {
                                    let physical_width = primary_monitor.width() as f32;
                                    let physical_height = primary_monitor.height() as f32;
                                    let x = primary_monitor.x();
                                    let y = primary_monitor.y();
                                    let width = win.get_ui_width();
                                    
                                    let scale_factor = sys_util::get_scale_factor(primary_monitor.id());
                                    let x_pos = x + ((physical_width - width * scale_factor) * 0.5) as i32;
                                    let y_pos = y + (physical_height * 0.3) as i32;
                                    win.window().set_position(slint::WindowPosition::Physical(slint::PhysicalPosition::new(x_pos, y_pos)));
                                }
                            }
                            let _ = searcher_msg_sender_clone_clone.send(SearcherMessage::Update);
                            let _ = win.show();
                            win.window().set_size(win.window().size()); // trick: fix the bug of error scale_factor
                            win.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
                                winit_win.focus_window();
                            });
                        }).unwrap_or_else(|e| log_util::log_error(format!("Failed to show search window: {:?}", e)));
                    },
                    Err(e) => {
                        log_util::log_error(format!("Failed to get message: {:?}", e));
                        break;
                    }
                }
            }
        });
        msg_sender
    }

    fn get_hotkey(&mut self) -> Option<HotKey> {
        AppConfig::global()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get_hotkey_from_str("search")
    }

    fn clean(&self) {
        // nothing need to clean until now
    }
}

impl Searcher {
    pub fn new() -> Result<Searcher, Box<dyn Error>> {
        let search_win = SearchWindow::new()?;
        
        let mut app_config = AppConfig::global().lock()?;
        search_win.invoke_change_theme(app_config.get_theme() as i32);
        app_config.search_win = Some(search_win.as_weak());

        search_win.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
            winit_win.set_skip_taskbar(true);
        });

        let search_result_model = Rc::new(slint::VecModel::from(vec![]));
        search_win.set_search_result(search_result_model.clone().into());
        search_win.set_active_id(0);

        let (searcher_msg_sender, searcher_msg_receiver) = mpsc::channel::<SearcherMessage>();
        let _file_data = FileData::new(search_win.as_weak());
        FileData::event_loop(searcher_msg_receiver, _file_data);
        let _ = searcher_msg_sender.send(SearcherMessage::Init);

        { // add result
            let search_win_clone = search_win.as_weak();
            let searcher_msg_sender_clone = searcher_msg_sender.clone();
            search_win.on_add_result(move || {
                if let Some(search_win_clone) = search_win_clone.upgrade() {
                    let _ = searcher_msg_sender_clone.send(SearcherMessage::Find(search_win_clone.get_query().to_string()));
                }
            });
        }

        { // add key event hander
            let search_win_clone = search_win.as_weak();
            let searcher_msg_sender_clone = searcher_msg_sender.clone();
            let search_result_model_clone = search_result_model.clone();
            search_win.on_key_pressed(move |event| {
                match search_win_clone.upgrade() {
                    Some(search_win_clone) => {
                        if event.text == slint::SharedString::from(slint::platform::Key::Escape) {
                            // ESC
                            let _ = search_win_clone.hide();
                        }else if event.text == slint::SharedString::from(slint::platform::Key::UpArrow) {
                            // UpArrow
                            let mut active_id = search_win_clone.get_active_id();
                            if active_id > 0 { 
                                active_id -= 1;
                                search_win_clone.set_active_id(active_id);
                                let viewport_y = search_win_clone.get_viewport_y();
                                if (-viewport_y / 60.) as i32 > active_id { search_win_clone.set_viewport_y(viewport_y + 60.); }
                            }
                        }else if event.text == slint::SharedString::from(slint::platform::Key::DownArrow) {
                            // DownArrow
                            let mut active_id = search_win_clone.get_active_id();
                            if active_id < (search_result_model_clone.row_count() - 1) as i32 { // If no more item
                                active_id += 1;
                                search_win_clone.set_active_id(active_id);
                                let viewport_y = search_win_clone.get_viewport_y();
                                if (-viewport_y / 60. + 7.) as i32 <= active_id { search_win_clone.set_viewport_y(viewport_y - 60.); }
                            }
                            // If to the bottom, try to find more
                            if active_id == (search_result_model_clone.row_count() - 1) as i32 {
                                let _ = searcher_msg_sender_clone.send(SearcherMessage::Find(search_win_clone.get_query().to_string()));
                            }
                        }else if event.text == slint::SharedString::from(slint::platform::Key::Return) {
                            // Enter
                            let active_id = search_win_clone.get_active_id();
                            let data = search_result_model_clone.row_data(active_id as usize);
                            if let Some(f) = data {
                                file_util::open_file((f.path + &f.filename).to_string())
                                    .unwrap_or_else(|e| log_util::log_error(format!("open_file error: {:?}", e)));
                                let _ = search_win_clone.hide();
                            }
                        }
                    },
                    None => { log_util::log_error("Failed to upgrade search_win in key event hander".to_string()); }
                }
            });
        }

        { // add focus change hander
            let search_win_clone = search_win.as_weak();
            let searcher_msg_sender_clone = searcher_msg_sender.clone();
            search_win.on_focus_change(move |has_focus| {
                if let Some(search_win) = search_win_clone.upgrade() {
                    if !has_focus { 
                        if search_win.get_query() != "" {
                            search_win.set_query(slint::SharedString::from(""));
                            search_win.invoke_query_change(slint::SharedString::from(""));
                        }
                        let _ = search_win.hide();
                        let _ = searcher_msg_sender_clone.send(SearcherMessage::Release);
                    }
                }
            });
        }
        
        { // add query change hander
            let searcher_msg_sender_clone = searcher_msg_sender.clone();
            let search_result_model_clone = search_result_model.clone();
            search_win.on_query_change(move |query| {
                if query.is_empty() { search_result_model_clone.set_vec(vec![]); }
                let _ = searcher_msg_sender_clone.send(SearcherMessage::Find(query.to_string()));
            });
        }

        { // add item click hander
            let search_result_model_clone = search_result_model.clone();
            search_win.on_item_click(move |event, id| {
                if event.kind == slint::private_unstable_api::re_exports::PointerEventKind::Up {
                    if event.button == slint::platform::PointerEventButton::Left {
                        let data = search_result_model_clone.row_data(id as usize);
                        if let Some(f) = data {
                            file_util::open_file((f.path + &f.filename).to_string())
                                .unwrap_or_else(|e| log_util::log_error(format!("open_file error: {:?}", e)));
                        }
                    }
                }
            });
        }

        { // on open with admin
            let search_result_model_clone = search_result_model.clone();
            search_win.on_open_with_admin(move |id| {
                let data = search_result_model_clone.row_data(id as usize);
                if let Some(f) = data {
                    file_util::open_file_admin((f.path + &f.filename).to_string());
                }
            });
        }

        { // on open file dir
            let search_result_model_clone = search_result_model.clone();
            search_win.on_open_file_dir(move |id| {
                let data = search_result_model_clone.row_data(id as usize);
                if let Some(f) = data {
                    file_util::open_file((f.path[..(f.path.len()-1)]).to_string())
                        .unwrap_or_else(|e| log_util::log_error(format!("open_file error: {:?}", e)));
                }
            });
        }

        Ok(Searcher {
            search_win,
            searcher_msg_sender,
        })
    }
}