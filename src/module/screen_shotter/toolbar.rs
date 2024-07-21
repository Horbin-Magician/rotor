use std::sync::mpsc::Sender;

use i_slint_backend_winit::WinitWindowAccessor;

use super::{PinOperation, ShotterMessage};


pub struct Toolbar {
    pub toolbar_window: ToolbarWindow,
}

impl Toolbar {
    pub fn new(message_sender: Sender<ShotterMessage>) -> Toolbar {
        let toolbar_window = ToolbarWindow::new().unwrap();

        let toolbar_window_clone = toolbar_window.as_weak();
        toolbar_window.on_show_pos(move |x, y| {
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
        toolbar_window.on_move(move |x, y| {
            let toolbar_window = toolbar_window_clone.unwrap();
            let r = x * x + y * y;
            let mut angle = y.atan2(x) * 180.0 / 3.14 + 135.0;
            if angle < 0.0 { angle += 360.0; }
            if r < (50 * 50) as f32 { 
                toolbar_window.set_active_num(-1);
            } else {
                let active_num = (angle / 90.0) as i32;
                toolbar_window.set_active_num(active_num);
            }
        });

        let toolbar_window_clone = toolbar_window.as_weak();
        let message_sender_clone = message_sender.clone();
        toolbar_window.on_finish(move |id| {
            let toolbar_window = toolbar_window_clone.unwrap();
            let active_num = toolbar_window.get_active_num();

            if active_num == 0 {
                message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::Hide())).unwrap();
            } else if active_num == 1 {
                message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::Copy())).unwrap();
            } else if active_num == 2 {
                message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::Close())).unwrap();
            } else if active_num == 3 {
                message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::Save())).unwrap();
            }

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

    component Arc inherits Path {
        in-out property <bool> active: false;
        in-out property <int> out_radius: 100;
        in-out property <int> inner_radius: 50;
        in-out property <float> out_start_x;
        in-out property <float> out_start_y;
        in-out property <float> out_end_x;
        in-out property <float> out_end_y;
        in-out property <float> inner_start_x;
        in-out property <float> inner_start_y;
        in-out property <float> inner_end_x;
        in-out property <float> inner_end_y;

        width: 100%;
        height: 100%;
        viewbox-width: self.width / 1px;
        viewbox-height: self.height / 1px;
        fill: active ? rgb(0, 175, 255).with-alpha(0.5) : transparent;
        stroke: rgb(0, 175, 255);
        stroke-width: 2px;

        MoveTo { x: inner_start_x; y: inner_start_y; }
        LineTo { x: out_start_x;   y: out_start_y;   }

        ArcTo {
            x: out_end_x; y: out_end_y;
            radius-x: out_radius; radius-y: out_radius; sweep: true;
        }

        LineTo { x:  inner_end_x; y: inner_end_y; }

        ArcTo {
            x: inner_start_x; y: inner_start_y;
            radius-x: inner_radius; radius-y: inner_radius;
        }
    }

    component ArcBtn inherits Rectangle{
        in-out property <image> icon;
        in-out property <int> out_radius: 100;
        in-out property <int> inner_radius: 50;
        in-out property <int> begin_angle;
        in-out property <int> end_angle;
        in-out property <bool> active: false;

        Arc {
            active: active;
            out_radius: out_radius;
            inner_radius: inner_radius;
            out_start_x: 100 + sin(begin_angle * 1deg) * out_radius;
            out_start_y: 100 - cos(begin_angle * 1deg) * out_radius;
            out_end_x: 100 + sin(end_angle * 1deg) * out_radius;
            out_end_y: 100 - cos(end_angle * 1deg) * out_radius;
            inner_start_x: 100 + sin(begin_angle * 1deg) * inner_radius;
            inner_start_y: 100 - cos(begin_angle * 1deg) * inner_radius;
            inner_end_x: 100 + sin(end_angle * 1deg) * inner_radius;
            inner_end_y: 100 - cos(end_angle * 1deg) * inner_radius;
        }

        Image {
            colorize: active ? rgb(0, 175, 255) : Palette.foreground;
            source: icon;
            height: root.height / 8;
            width: root.width / 8;
            x: 100px + sin((begin_angle + end_angle) / 2 * 1deg) * (out_radius + inner_radius) / 2 * 1px - self.width / 2;
            y: 100px - cos((begin_angle + end_angle) / 2 * 1deg) * (out_radius + inner_radius) / 2 * 1px - self.height / 2;
        }
    }

    export component ToolbarWindow inherits Window {
        background: transparent;
        no-frame: true;
        title: "菜单栏";
        always-on-top: true;

        in-out property <int> win_height: 200;
        in-out property <int> win_width: 200;
        in-out property <int> active_num: -1;

        width: win_width * 1px;
        height: win_height * 1px;

        callback show_pos(int, int);
        callback move(float, float);
        callback finish(int);

        Rectangle {
            height: 100%;
            width: 100%;
            border-radius: self.height / 2;
            background: Palette.background.with-alpha(0.5);

            ArcBtn{
                icon: @image-url("./assets/icon/min.svg");
                begin_angle: -45;
                end_angle: 45;
                active: active_num == 0;
            }

            ArcBtn{
                icon: @image-url("./assets/icon/right.svg");
                begin_angle: 45;
                end_angle: 135;
                active: active_num == 1;
            }

            ArcBtn{
                icon: @image-url("./assets/icon/close.svg");
                begin_angle: 135;
                end_angle: 225;
                active: active_num == 2;
            }

            ArcBtn{
                icon: @image-url("./assets/icon/save.svg");
                begin_angle: 225;
                end_angle: 315;
                active: active_num == 3;
            }
        }
    }
}