use slint::ComponentHandle;
use windows_sys::Win32::UI::WindowsAndMessaging;
use std::rc::Rc;

pub struct Searcher {
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
        search_win.set_ui_width(width);
        search_win.set_ui_height(height);
        let x_pos = (x_screen - width) * 0.5;
        let y_pos = (y_screen - height) * 0.382 - 30.;
        search_win.window().set_position(slint::WindowPosition::Logical(slint::LogicalPosition::new(x_pos, y_pos)));

        let search_result_model = std::rc::Rc::new(slint::VecModel::from(vec![]));
        search_win.set_search_result(search_result_model.clone().into());

        let search_result_model_clone = search_result_model.clone();
        search_win.on_query_change(move |query| {
            // TODO 搜索事件
            if query == "" {
                search_result_model_clone.set_vec(vec![]);
            } else {
                search_result_model_clone.push(SearchResult { filename: query.clone().into(), path: query.clone().into(),});
            }
        });

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

        Searcher {
            search_win,
            search_result_model,
        }
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
        default-font-size: 20px;
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
                                        width: 60px;
                                        height: 60px;
                                        Image {
                                            padding: 15px;
                                            width: 30px;
                                            height: 30px;
                                            source: @image-url("assets/logo.png");
                                        }
                                    }
                                    VerticalBox {
                                        padding: 0;
                                        Text {
                                            height: 20px;
                                            text: data.filename;
                                            font-size: 18px;
                                        }
                                        Text {
                                            height: 40px;
                                            text: data.path;
                                            color: grey;
                                            font-size: 18px;
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