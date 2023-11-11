mod file_data;
mod volume;

use slint::{ComponentHandle, Model};
use std::{sync::{mpsc, mpsc::Sender}, rc::Rc};
use i_slint_backend_winit::WinitWindowAccessor;
use windows_sys::Win32::UI::WindowsAndMessaging;
use global_hotkey::hotkey::{HotKey, Modifiers, Code};

use crate::core::util::file_util;
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
    id: Option<u32>,
}

impl Module for Searcher{
    fn run(&self) -> Sender<ModuleMessage> {
        let (msg_sender, msg_reciever) = mpsc::channel();
        let search_win_clone = self.search_win.as_weak();
        std::thread::spawn(move || {
            loop {
                match msg_reciever.recv().unwrap() {
                    ModuleMessage::Trigger => {
                        search_win_clone.upgrade_in_event_loop(move |win| {
                            // BUG1: a trick to make on_lose_focus_trick work on the first time
                            win.show().unwrap();
                            win.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
                                winit_win.focus_window();
                            });
                        }).unwrap();
                    }
                }
            }
        });
        return msg_sender;
    }

    fn get_hotkey(&mut self) -> HotKey {
        let hotkey = HotKey::new(Some(Modifiers::SHIFT), Code::KeyF);
        self.id = Some(hotkey.id());
        return  hotkey;
    }

    fn get_id(&self) -> Option<u32> {
        return self.id;
    }
}

impl Searcher {
    pub fn new() -> Searcher {
        let search_win = SearchWindow::new().unwrap();

        let width: f32 = 500.;
        search_win.set_ui_width(width);

        let x_screen: f32;
        let y_screen: f32;
        unsafe{
            x_screen = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXSCREEN) as f32;
            y_screen = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CYSCREEN) as f32;
        }
        let x_pos = ((x_screen - width * search_win.window().scale_factor()) * 0.5) as i32;
        let y_pos = (y_screen * 0.3) as i32;
        search_win.window().set_position(slint::WindowPosition::Physical(slint::PhysicalPosition::new(x_pos, y_pos)));

        let search_result_model = Rc::new(slint::VecModel::from(vec![]));
        search_win.set_search_result(search_result_model.clone().into());
        search_win.set_active_id(0);

        let (searcher_msg_sender, searcher_msg_receiver) = mpsc::channel::<SearcherMessage>();
        let _file_data = FileData::new(search_win.as_weak());
        FileData::event_loop(searcher_msg_receiver, _file_data);
        searcher_msg_sender.send(SearcherMessage::Init).unwrap();

        { // add key event hander
            let search_win_clone = search_win.as_weak();
            let search_result_model_clone = search_result_model.clone();
            search_win.on_key_released(move |event| {
                let search_win_clone = search_win_clone.unwrap();
                if event.text == slint::SharedString::from(slint::platform::Key::Escape) {
                    search_win_clone.hide().unwrap();
                }else if event.text == slint::SharedString::from(slint::platform::Key::UpArrow) {
                    let mut active_id = search_win_clone.get_active_id();
                    if active_id > 0 { 
                        active_id -= 1;
                        search_win_clone.set_active_id(active_id);
                        let viewport_y = search_win_clone.get_viewport_y();
                        if (-viewport_y / 60.) as i32 > active_id { search_win_clone.set_viewport_y(viewport_y + 60.); }
                    }
                }else if event.text == slint::SharedString::from(slint::platform::Key::DownArrow) {
                    let mut active_id = search_win_clone.get_active_id();
                    if active_id < (search_result_model_clone.row_count() - 1) as i32 { 
                        active_id += 1;
                        search_win_clone.set_active_id(active_id);
                        let viewport_y = search_win_clone.get_viewport_y();
                        if (-viewport_y / 60. + 7.) as i32 <= active_id { search_win_clone.set_viewport_y(viewport_y - 60.); }
                    }
                }else if event.text == slint::SharedString::from(slint::platform::Key::Return) {
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
                println!("has_focus: {}; visible: {}", has_focus, search_win.window().is_visible());
                if !has_focus { 
                    if search_win.get_query() != "" {
                        search_win.set_query(slint::SharedString::from(""));
                        search_win.invoke_query_change(slint::SharedString::from(""));
                    }
                    search_win.hide().unwrap();
                    searcher_msg_sender_clone.send(SearcherMessage::Release).unwrap();
                } else if has_focus && search_win.window().is_visible() {
                    searcher_msg_sender_clone.send(SearcherMessage::Update).unwrap();
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
                    file_util::open_file((f.path[0..(f.path.to_string().len()-1)]).to_string());
                    search_win.hide().unwrap();
                }
            });
        }

        Searcher {
            search_win,
            id: None,
        }
    }
}

slint::slint! {
    import { Button, VerticalBox, LineEdit, ListView , HorizontalBox, StyleMetrics} from "std-widgets.slint";

    struct SearchResult_slint {
        id: int,
        icon: image,
        filename: string,
        path: string,
    }

    export component SearchWindow inherits Window {
        in property <float> ui_width;
        in property <[SearchResult_slint]> search_result;
        in property <int> active_id;

        in-out property <string> query <=> input.text;
        in-out property <length> viewport-y <=> result-list.viewport-y;

        callback query_change(string);
        callback key_released(KeyEvent);
        callback item_click(PointerEvent, int);
        callback open_with_admin(int);
        callback open_file_dir(int);
        pure callback lose_focus_trick(bool) -> bool;

        title: "小云搜索";
        no-frame: true;
        forward-focus: input;
        default-font-size: 18px;
        default-font-family: "Microsoft YaHei UI";
        icon: @image-url("assets/logo.png");
        width: ui_width * 1px;
        height: 494px; // BUG2: Flexible change
        always-on-top: lose_focus_trick(input.has-focus || key-handler.has-focus);
        background: transparent;

        VerticalBox {
            Rectangle {
                border-radius: 5px;
                background: StyleMetrics.window-background;
                key-handler := FocusScope {
                    key-released(event) => {
                        debug(parent.height);
                        root.key_released(event);
                        accept
                    }

                    VerticalBox {
                        padding: 0;
                        spacing: 0;
                        input := LineEdit {
                            height: 60px;
                            placeholder-text: "请输入需要搜索的内容";
                            edited(str) => {
                                root.query_change(str);
                            }
                        }
                        result-list := ListView {
                            padding: 0;
                            height: (search_result.length > 7 ? 7 : search_result.length) * 60px + (search_result.length > 0 ? 14px : 0px);

                            for data in root.search_result: Rectangle {
                                height: 60px;
                                border-radius: 5px;

                                search_result_item_touch := TouchArea {
                                    mouse-cursor: pointer;
                                    pointer-event(event) => {
                                        root.item-click(event, data.id);
                                    }

                                    HorizontalLayout {
                                        item_content := HorizontalBox {
                                            width: 100%;
                                            padding-right: 0px;
                                            padding-left: 0px;
                                            Rectangle {
                                                width: 10px;
                                                active_bar := Rectangle {
                                                    x: 0px;
                                                    width: 2px;
                                                    border-radius: 1px;
                                                    height: 30px;
                                                    background: cyan;

                                                    animate x { 
                                                        duration: 200ms;
                                                        easing: linear;
                                                    }
                                                }
                                            }
                                            Rectangle {
                                                width: 30px;
                                                Image {
                                                    height: 32px;
                                                    source: data.icon;
                                                }
                                            }
                                            VerticalBox {
                                                padding: 0;
                                                Text {
                                                    height: 20px;
                                                    overflow: elide;
                                                    text: data.filename;
                                                    font-size: 16px;
                                                }
                                                Text {
                                                    height: 40px;
                                                    overflow: elide;
                                                    text: data.path;
                                                    color: grey;
                                                    font-size: 16px;
                                                }
                                            }
                                            animate width { 
                                                duration: 0.2s;
                                                easing: ease-in-out;
                                            }
                                        }

                                        item_menu := Rectangle {
                                            width: 100px;
                                            HorizontalLayout {
                                                TouchArea {
                                                    width: 50px;
                                                    mouse-cursor: pointer;
                                                    clicked => { root.open_with_admin(data.id); }
                                                    admin_btn_rc := Rectangle {
                                                        admin_btn_img := Image {
                                                            height: 20px;
                                                            width: 20px;
                                                            colorize: StyleMetrics.default-text-color;
                                                            source: @image-url("assets/icon/admin.svg");
                                                        }
                                                    }
                                                    states [ 
                                                        hover when self.has-hover: {
                                                            admin_btn_img.colorize: cyan;
                                                        }
                                                    ]
                                                }
                                                
                                                TouchArea {
                                                    width: 50px;
                                                    mouse-cursor: pointer;
                                                    clicked => { root.open_file_dir(data.id); }
                                                    file_btn_rc := Rectangle {
                                                        file_btn_img := Image {
                                                            height: 20px;
                                                            width: 20px;
                                                            colorize: StyleMetrics.default-text-color;
                                                            source: @image-url("assets/icon/file.svg");
                                                        }
                                                    }
                                                    states [ 
                                                        hover when self.has-hover: {
                                                            file_btn_img.colorize: cyan;
                                                        }
                                                    ]
                                                }
                                            }
                                        }
                                    }
                                }

                                states [
                                    active when root.active_id == data.id && !search_result_item_touch.has-hover: {
                                        background: StyleMetrics.textedit-background-disabled;
                                        active_bar.x: 0px;
                                        item_content.width: self.width;
                                    }
                                    inactive when root.active_id != data.id && !search_result_item_touch.has-hover : {
                                        background: transparent;
                                        active_bar.x: -2px;
                                        item_content.width: self.width;
                                    }
                                    active_hover when root.active_id == data.id && search_result_item_touch.has-hover: {
                                        background: StyleMetrics.textedit-background-disabled;
                                        active_bar.x: 0px;
                                        item_content.width: self.width - item_menu.width;
                                    }
                                    hover when search_result_item_touch.has-hover: {
                                        background: StyleMetrics.textedit-background-disabled;
                                        active_bar.x: -2px;
                                        item_content.width: self.width - item_menu.width;
                                    }
                                ]
                            }
                        }
                    }
                }
            }
        }
    }
}