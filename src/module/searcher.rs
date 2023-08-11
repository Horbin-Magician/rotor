use slint::ComponentHandle;
use windows_sys::Win32::UI::WindowsAndMessaging;
use std::rc::Rc;

pub struct Searcher {
    search_win: Rc<SearchWindow>,
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

        let search_win = Rc::new(SearchWindow::new().unwrap());
        search_win.set_ui_width(width);
        search_win.set_ui_height(height);
        println!("{x_screen}, {y_screen}");
        let x_pos = (x_screen - width) * 0.5;
        let y_pos = (y_screen - height) * 0.382;
        search_win.window().set_position(slint::WindowPosition::Logical(slint::LogicalPosition::new(x_pos, y_pos)));

        search_win.on_query_change( |query| {
            println!("search: {}", query);
        });

        let search_win_clone = search_win.clone();
        
        search_win.on_key_press(move |event| {
            if event.modifiers.control {
                // TODO
            }
            if event.text == slint::SharedString::from(slint::platform::Key::Escape) {
                search_win_clone.hide().unwrap();
            }
        });

        let search_win_clone = search_win.clone();

        search_win.on_lose_focus_trick(move |has_focus| {
            if has_focus == false {
                search_win_clone.hide().unwrap();
            }
            return slint::Color::from_rgb_u8(0, 0, 0);
        });

        Searcher {
            search_win,
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
    import { Button, VerticalBox, LineEdit } from "std-widgets.slint";

    export component SearchWindow inherits Window {
    
        in property <float> ui_width;
        in property <float> ui_height;

        callback query_change(string);
        callback key_press(KeyEvent);
        pure callback lose_focus_trick(bool) -> color;

        always-on-top: true;
        no-frame: true;
        forward-focus: input;
        default-font-size: 20px;
        default-font-family: "Microsoft YaHei UI";
        icon: @image-url("assets/logo.png");
        width: ui_width * 1px;
        height: ui_height * 1px;
        background: lose_focus_trick(input.has-focus);

        key-handler := FocusScope {
            key-pressed(event) => {
                root.key_press(event);
                accept
            }

            VerticalBox {
                padding: 0;
                input := LineEdit {
                    placeholder-text: "请输入需要搜索的内容";
                    edited(str) => {
                        root.query_change(str);
                    }
                    height: 100%;
                    width: 100%;
                }
            }
        }
    }
}