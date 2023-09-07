mod file_data;
pub mod volume;

use slint::{ComponentHandle, Model};
use windows_sys::Win32::UI::WindowsAndMessaging;
use std::rc::Rc;
use std::thread;
use std::sync::{Arc, Mutex, mpsc};

use crate::core::util::file_util;
use file_data::FileData;

pub struct Searcher {
    pub search_win: SearchWindow,
    file_data: Arc<Mutex<FileData>>,
    search_result_model: Rc<slint::VecModel<SearchResult_slint>>,
    stop_find_sender: mpsc::Sender<()>,
}

impl Searcher {
    pub fn new() -> Searcher {
        let x_screen: f32;
        let y_screen: f32;
        unsafe{
            x_screen = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXSCREEN) as f32;
            y_screen = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CYSCREEN) as f32;
        }

        let search_win = SearchWindow::new().unwrap();
        let width: f32 = 500.;
        search_win.set_ui_width(width);

        // to fix the bug that the on_lose_focus_trick do not be triggered when it is first displayed
        search_win.show().unwrap();
        search_win.hide().unwrap();

        let x_pos = (x_screen - width) * 0.5;
        let y_pos = y_screen * 0.3;
        search_win.window().set_position(slint::WindowPosition::Logical(slint::LogicalPosition::new(x_pos, y_pos)));
        let search_result_model = Rc::new(slint::VecModel::from(vec![]));
        search_win.set_search_result(search_result_model.clone().into());
        search_win.set_active_id(0);

        let (stop_find_sender, stop_finder_receiver) = mpsc::channel::<()>();
        let file_data = Arc::new(Mutex::new(FileData::new(search_win.as_weak(), stop_finder_receiver)));
        
        let file_data_clone = file_data.clone();
        thread::spawn(move || {
            file_data_clone.lock().unwrap().init_volumes();
        });

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

        { // add lose focus hander
            let search_win_clone = search_win.as_weak();
            let file_data_clone = file_data.clone();
            let stop_find_sender_clone = stop_find_sender.clone();
            search_win.on_lose_focus_trick(move |has_focus| {
                match file_data_clone.try_lock() {
                    Ok(_) => {},
                    Err(_) => { 
                        stop_find_sender_clone.send(()).unwrap(); 
                    },
                }
                let file_data = file_data_clone.clone();
                let search_win = search_win_clone.unwrap();
                if has_focus == false { 
                    if search_win.get_query() != "" {
                        search_win.set_query(slint::SharedString::from(""));
                        search_win.invoke_query_change(slint::SharedString::from(""));
                    }
                    search_win.hide().unwrap();
                    thread::spawn(move || {
                        file_data.lock().unwrap().release_index();
                    });
                } else if has_focus && search_win.window().is_visible() {
                    thread::spawn(move || {
                        file_data.lock().unwrap().update_index();
                    });
                }
                return true;
            });
        }
        
        { // add query change hander
            let file_data_clone = file_data.clone();
            let stop_find_sender_clone = stop_find_sender.clone();
            let search_result_model_clone = search_result_model.clone();
            search_win.on_query_change(move |query| {
                let file_data_clone_clone = file_data_clone.clone();
                match file_data_clone_clone.try_lock() {
                    Ok(_) => {},
                    Err(_) => { stop_find_sender_clone.send(()).unwrap(); },
                }
                if query == "" { search_result_model_clone.set_vec(vec![]); }
                thread::spawn(move || {
                    file_data_clone_clone.lock().unwrap().find(query.to_string());
                });
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

        let searcher = Searcher {
            file_data,
            search_win,
            search_result_model,
            stop_find_sender,
        };
        searcher
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
        in property <float> ui_height;
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

        no-frame: true;
        forward-focus: input;
        default-font-size: 18px;
        default-font-family: "Microsoft YaHei UI";
        icon: @image-url("assets/logo.png");
        width: ui_width * 1px;
        height: 510px;
        always-on-top: lose_focus_trick(input.has-focus || key-handler.has-focus);
        background: transparent;

        VerticalBox {
            Rectangle {
                border-radius: 5px;
                background: StyleMetrics.window-background;
                key-handler := FocusScope {
                    key-released(event) => {
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
                            animate height { 
                                duration: 0.2s;
                                easing: ease-in-out;
                            }

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
                                                        duration: 0.2s;
                                                        easing: ease-in-out;
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