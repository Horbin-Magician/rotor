use i_slint_backend_winit::WinitWindowAccessor;


pub struct Toolbar {
    pub toolbar_window: ToolbarWindow,
}

impl Toolbar {
    pub fn new() -> Toolbar {
        let toolbar_window = ToolbarWindow::new().unwrap();

        let toolbar_window_clone = toolbar_window.as_weak();
        toolbar_window.on_show_pos(move |id, x, y| {
            println!("show_pos: x={}, y={}", x, y);
            let toolbar_window = toolbar_window_clone.unwrap();
            let height = toolbar_window.get_win_height() as f32;
            let width = toolbar_window.get_win_width() as f32;
            let scale = toolbar_window.window().scale_factor();
            let x_pos = x - (width * scale / 2.0) as i32;
            let y_pos = y - (height * scale / 2.0) as i32;
            toolbar_window.window().set_position(slint::WindowPosition::Physical(slint::PhysicalPosition::new(x_pos, y_pos)));
            toolbar_window.show().unwrap();

            toolbar_window.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
                winit_win.focus_window();
            });
        });

        let toolbar_window_clone = toolbar_window.as_weak();
        toolbar_window.on_move(move |id, x, y| {
            // TODO
            println!("move: x={}, y={}", x, y);
        });

        let toolbar_window_clone = toolbar_window.as_weak();
        toolbar_window.on_finish(move |id| {
            toolbar_window_clone.unwrap().hide().unwrap();
        });

        Toolbar {
            toolbar_window,
        }
    }

    pub fn get_window(&self) -> slint::Weak<ToolbarWindow> {
        self.toolbar_window.as_weak()
    }
}

slint::slint! {
    import { Button, Palette} from "std-widgets.slint";
    export component ToolbarWindow inherits Window {
        background: transparent;
        no-frame: true;
        always-on-top: true;

        in-out property <int> win_height: 200;
        in-out property <int> win_width: 200;

        // pure callback lose_focus_trick(bool) -> bool;

        width: win_width * 1px;
        height: win_height * 1px;

        callback show_pos(int, int, int);
        callback move(int, float, float);
        callback finish(int);

        Rectangle {
            height: 100%;
            width: 100%;
            border-radius: self.height / 2;
            background: Palette.background;

            TouchArea {
                pointer-event(event) => {
                    debug(event);
                    if (event.button == PointerEventButton.right) {
                        debug(event);
                    }
                }

                Button {
                    text: "Capture";
                    clicked => {
                        debug("Capture button clicked");
                    }
                }
            }
        }
    }
}