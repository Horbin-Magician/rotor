use slint::ComponentHandle;
use windows_sys::Win32::UI::WindowsAndMessaging;
use std::rc::Rc;
use std::thread;
use std::sync::{Arc, Mutex, mpsc};

use file_data::FileData;
mod file_data;
pub mod volume;

pub struct Searcher {
    file_data: Arc<Mutex<FileData>>,
    search_win: SearchWindow,
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
        let x_pos = (x_screen - width) * 0.5;
        let y_pos = y_screen * 0.3;
        search_win.window().set_position(slint::WindowPosition::Logical(slint::LogicalPosition::new(x_pos, y_pos)));
        let search_result_model = Rc::new(slint::VecModel::from(vec![]));
        search_win.set_search_result(search_result_model.clone().into());

        let search_win_clone = search_win.as_weak();
        search_win.on_key_released(move |event| {
            if event.text == slint::SharedString::from(slint::platform::Key::Escape) {
                search_win_clone.unwrap().hide().unwrap();
            }else if event.text == slint::SharedString::from(slint::platform::Key::UpArrow) {
                println!("UP") // TODO UP
            }else if event.text == slint::SharedString::from(slint::platform::Key::DownArrow) {
                println!("Down") // TODO Down
            }else if event.text == slint::SharedString::from(slint::platform::Key::Return) {
                println!("Enter") // TODO Enter
            }
        });

        let search_win_clone = search_win.as_weak();

        search_win.on_lose_focus_trick(move |has_focus| {
            if has_focus == false { search_win_clone.unwrap().hide().unwrap(); }
            return true;
        });
        
        let (stop_find_sender, stop_finder_receiver) = mpsc::channel::<()>();
        let file_data = Arc::new(Mutex::new(FileData::new(search_win.as_weak(), stop_finder_receiver)));
        let file_data_clone = file_data.clone();
        thread::spawn(move || {
            file_data_clone.lock().unwrap().init_volumes();
        });

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

        let searcher = Searcher {
            file_data,
            search_win,
            search_result_model,
            stop_find_sender,
        };
        searcher
    }

    pub fn show(&self) {
        self.search_win.show().unwrap();
    }

    pub fn hide(&self) {
        self.search_win.hide().unwrap();
    }
}

slint::slint! {
    import { Button, VerticalBox, LineEdit, ListView , HorizontalBox, StyleMetrics} from "std-widgets.slint";

    struct SearchResult_slint {
        filename: string,
        path: string,
    }

    export component SearchWindow inherits Window {
    
        in property <float> ui_width;
        in property <float> ui_height;
        in property <[SearchResult_slint]> search_result;

        callback query_change(string);
        callback key_released(KeyEvent);
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
                            height: (search_result.length > 7 ? 7 : search_result.length) * 60px + (search_result.length > 0 ? 14px : 0px);
                            animate height { 
                                duration: 0.2s;
                                easing: ease-in-out;
                            }
                            for data in root.search_result: Rectangle {
                                height: 60px;
                                HorizontalBox {
                                    padding-right: 0px;
                                    Rectangle {
                                        width: 30px;
                                        Image {
                                            height: 30px;
                                            source: @image-url("assets/logo.png");
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
                                }
                            }
                        }
                    }
                }
            }
        }

    }
}