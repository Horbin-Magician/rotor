use slint::ComponentHandle;
use windows_sys::Win32::UI::WindowsAndMessaging;
// use i_slint_backend_winit::WinitWindowAccessor;
// use raw_window_handle::HasRawWindowHandle;
use std::{rc::Rc, cell::RefCell, borrow::BorrowMut};

use file_data::FileData;
mod file_data;
pub mod volume;

pub struct Searcher {
    file_data_rc: Rc<RefCell<FileData>>,
    search_win: SearchWindow,
    search_result_model: Rc<slint::VecModel<SearchResult>>,
}

impl Searcher {
    pub fn new() -> Searcher {
        let x_screen: f32;
        let y_screen: f32;
        unsafe{
            x_screen = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXSCREEN) as f32;
            y_screen = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CYSCREEN) as f32;
        }
    
        let width: f32 = 500.;
        let height: f32 = 60.;


        let search_win = SearchWindow::new().unwrap();
        // search_win.window().with_winit_window(|winit_window: &winit::window::Window| {
        //     let raw_window_handle = winit_window.raw_window_handle();
        //     if let raw_window_handle::RawWindowHandle::Win32(win32_window_handle) = raw_window_handle {
        //         let hwnd = win32_window_handle.hwnd as isize;
        //         unsafe{
        //             WindowsAndMessaging::SetWindowLongPtrA(hwnd, WindowsAndMessaging::GWL_EXSTYLE, WindowsAndMessaging::WS_EX_TOOLWINDOW as isize);
        //             let result = WindowsAndMessaging::GetWindowLongPtrA(hwnd, WindowsAndMessaging::GWL_EXSTYLE);
        //         }
        //     }
        // });

        search_win.set_ui_width(width);
        search_win.set_ui_height(height);
        let x_pos = (x_screen - width) * 0.5;
        let y_pos = (y_screen - height) * 0.382 - 30.;
        search_win.window().set_position(slint::WindowPosition::Logical(slint::LogicalPosition::new(x_pos, y_pos)));

        let search_result_model = std::rc::Rc::new(slint::VecModel::from(vec![]));
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
            if has_focus == false {
                search_win_clone.unwrap().hide().unwrap();
            }
            return true;
        });
        
        let mut file_data = FileData::new();
        file_data.init_volumes();
        let file_data_rc: Rc<RefCell<FileData>> = Rc::new(RefCell::new(file_data));

        let search_result_model_clone = search_result_model.clone();
        let file_data_rc_clone = file_data_rc.clone();
        
        search_win.on_query_change(move |query| {
            search_result_model_clone.set_vec(vec![]); // clear history
            if query != "" {
                let mut file_data_mut = file_data_rc_clone.as_ref().borrow_mut();
                let result = file_data_mut.find(query.to_string());
                if result.query == query.to_string() {
                    for item in &result.items {
                        search_result_model_clone.push(
                            SearchResult { 
                                filename: slint::SharedString::from(item.file_name.clone()),
                                path: slint::SharedString::from(item.path.clone()),
                            }
                        );
                    }
                }
            }
        });

        let searcher = Searcher {
            file_data_rc,
            search_win,
            search_result_model,
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

    struct SearchResult {
        filename: string,
        path: string,
    }

    export component SearchWindow inherits Window {
    
        in property <float> ui_width;
        in property <float> ui_height;
        in property <[SearchResult]> search_result;

        callback query_change(string);
        callback key_released(KeyEvent);
        pure callback lose_focus_trick(bool) -> bool;

        no-frame: true;
        forward-focus: input;
        default-font-size: 18px;
        default-font-family: "Microsoft YaHei UI";
        icon: @image-url("assets/logo.png");
        width: ui_width * 1px;
        always-on-top: lose_focus_trick(input.has-focus || key-handler.has-focus);
        background: transparent;

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
                        height: (search_result.length > 7 ? 7 : search_result.length) * 66px + (search_result.length > 0 ? 14px : 0px);
                        for data in root.search_result : VerticalBox {
                            padding: 0;
                            padding-top: 6px;
                            padding-left: 10px;
                            Rectangle {
                                padding: 0;
                                height: 60px;
                                HorizontalBox { 
                                    padding: 0;
                                    Rectangle {
                                        padding-left: 10px;
                                        width: 40px;
                                        height: 100%;
                                        Image {
                                            width: 30px;
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