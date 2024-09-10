mod file_data;
mod volume;

use slint::{ComponentHandle, Model};
use std::{sync::{mpsc, mpsc::Sender}, rc::Rc};
use i_slint_backend_winit::{winit::platform::windows::WindowExtWindows, WinitWindowAccessor};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use global_hotkey::hotkey::HotKey;

use xcap::Monitor;
use crate::core::application::app_config::AppConfig;
use crate::util::file_util;
use crate::ui::SearchWindow;
use crate::module::{Module, ModuleMessage};
use file_data::FileData;

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
                match msg_reciever.recv().unwrap() {
                    ModuleMessage::Trigger => {
                        let searcher_msg_sender_clone_clone = searcher_msg_sender_clone.clone();
                        search_win_clone.upgrade_in_event_loop(move |win| {
                            // Set window center
                            let monitors = Monitor::all().unwrap();
                            let primary_monitor = monitors.iter().find(|m| m.is_primary()).unwrap();
                            let physical_width = primary_monitor.width() as f32;
                            let physical_height = primary_monitor.height() as f32;
                            let x = primary_monitor.x();
                            let y = primary_monitor.y();
                            let width = win.get_ui_width();
                            
                            let scale_factor = unsafe{ 
                                let mut dpi_x: u32 = 0;
                                let mut dpi_y: u32 = 0;
                                GetDpiForMonitor(primary_monitor.id() as isize, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
                                dpi_x as f32 / 96.0
                            };
                            let x_pos = x + ((physical_width - width * scale_factor) * 0.5) as i32;
                            let y_pos = y + (physical_height * 0.3) as i32;
                            win.window().set_position(slint::WindowPosition::Physical(slint::PhysicalPosition::new(x_pos, y_pos)));
                            
                            searcher_msg_sender_clone_clone.send(SearcherMessage::Update).unwrap();
                            win.show().unwrap();
                            win.window().set_size(win.window().size()); // trick: fix the bug of error scale_factor
                            win.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
                                winit_win.focus_window();
                            });
                        }).unwrap();
                    }
                }
            }
        });
        msg_sender
    }

    fn get_hotkey(&mut self) -> Option<HotKey> {
        let app_config = AppConfig::global().lock().unwrap();
        app_config.get_hotkey_from_str("search")
    }

    fn clean(&self) {
        // nothing need to clean until now
    }
}

impl Searcher {
    pub fn new() -> Searcher {
        let search_win = SearchWindow::new().unwrap();
        {
            let mut app_config = AppConfig::global().lock().unwrap();
            search_win.invoke_change_theme(app_config.get_theme() as i32);
            app_config.search_win = Some(search_win.as_weak());
        }

        search_win.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
            winit_win.set_skip_taskbar(true);
        });

        let search_result_model = Rc::new(slint::VecModel::from(vec![]));
        search_win.set_search_result(search_result_model.clone().into());
        search_win.set_active_id(0);

        let (searcher_msg_sender, searcher_msg_receiver) = mpsc::channel::<SearcherMessage>();
        let _file_data = FileData::new(search_win.as_weak());
        FileData::event_loop(searcher_msg_receiver, _file_data);
        searcher_msg_sender.send(SearcherMessage::Init).unwrap();

        { // add key event hander
            let search_win_clone = search_win.as_weak();
            let searcher_msg_sender_clone = searcher_msg_sender.clone();
            let search_result_model_clone = search_result_model.clone();
            search_win.on_key_pressed(move |event| {
                let search_win_clone = search_win_clone.unwrap();
                if event.text == slint::SharedString::from(slint::platform::Key::Escape) {
                    // ESC
                    search_win_clone.hide().unwrap();
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
                        searcher_msg_sender_clone.send(SearcherMessage::Find(search_win_clone.get_query().to_string())).unwrap();
                    }
                }else if event.text == slint::SharedString::from(slint::platform::Key::Return) {
                    // Enter
                    let active_id = search_win_clone.get_active_id();
                    let data = search_result_model_clone.row_data(active_id as usize);
                    if let Some(f) = data {
                        file_util::open_file((f.path + &f.filename).to_string());
                        search_win_clone.hide().unwrap();
                    }
                }
            });
        }

        { // add focus change hander
            let search_win_clone = search_win.as_weak();
            let searcher_msg_sender_clone = searcher_msg_sender.clone();
            search_win.on_lose_focus_trick(move |has_focus| {
                let search_win = search_win_clone.unwrap();
                if !has_focus { 
                    if search_win.get_query() != "" {
                        search_win.set_query(slint::SharedString::from(""));
                        search_win.invoke_query_change(slint::SharedString::from(""));
                    }
                    search_win.hide().unwrap();
                    searcher_msg_sender_clone.send(SearcherMessage::Release).unwrap();
                }
                true
            });
        }
        
        { // add query change hander
            let searcher_msg_sender_clone = searcher_msg_sender.clone();
            let search_result_model_clone = search_result_model.clone();
            search_win.on_query_change(move |query| {
                if query.is_empty() { search_result_model_clone.set_vec(vec![]); }
                searcher_msg_sender_clone.send(SearcherMessage::Find(query.to_string())).unwrap();
            });
        }

        { // add item click hander
            let search_win_clone = search_win.as_weak();
            let search_result_model_clone = search_result_model.clone();
            search_win.on_item_click(move |event, id| {
                if event.kind == slint::private_unstable_api::re_exports::PointerEventKind::Up {
                    let search_win = search_win_clone.unwrap();
                    if event.button == slint::platform::PointerEventButton::Left {
                        let data = search_result_model_clone.row_data(id as usize);
                        if let Some(f) = data {
                            file_util::open_file((f.path + &f.filename).to_string());
                            search_win.hide().unwrap();
                        }
                    }
                }
            });
        }

        { // on open with admin
            let search_win_clone = search_win.as_weak();
            let search_result_model_clone = search_result_model.clone();
            search_win.on_open_with_admin(move |id| {
                let search_win = search_win_clone.unwrap();
                let data = search_result_model_clone.row_data(id as usize);
                if let Some(f) = data {
                    file_util::open_file_admin((f.path + &f.filename).to_string());
                    search_win.hide().unwrap();
                }
            });
        }

        { // on open file dir
            let search_win_clone = search_win.as_weak();
            let search_result_model_clone = search_result_model.clone();
            search_win.on_open_file_dir(move |id| {
                let search_win = search_win_clone.unwrap();
                let data = search_result_model_clone.row_data(id as usize);
                if let Some(f) = data {
                    file_util::open_file((f.path[..(f.path.len()-1)]).to_string());
                    search_win.hide().unwrap();
                }
            });
        }

        Searcher {
            search_win,
            searcher_msg_sender,
        }
    }
}