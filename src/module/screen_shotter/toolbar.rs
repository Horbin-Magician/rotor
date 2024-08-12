use std::sync::mpsc::Sender;

use i_slint_backend_winit::{winit::platform::windows::WindowExtWindows, WinitWindowAccessor};

use super::{PinOperation, ShotterMessage};


pub struct Toolbar {
    pub toolbar_window: ToolbarWindow,
}

impl Toolbar {
    pub fn new(message_sender: Sender<ShotterMessage>) -> Toolbar {
        let toolbar_window = ToolbarWindow::new().unwrap();
        toolbar_window.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
            winit_win.set_skip_taskbar(true);
        });

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

    struct Tool_slint {
        id: int,
        icon: image,
        name: string,
    }

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

        commands: @tr("M {} {} L {} {} A {} {} 0 0 0 {} {} L {} {} A {} {} 0 0 0 {} {}",
            inner_start_x, inner_start_y,
            out_start_x, out_start_y,
            out_radius, out_radius, out_end_x, out_end_y,
            inner_end_x, inner_end_y,
            inner_radius, inner_radius, inner_start_x, inner_start_y
        );
    }

    component ArcBtn inherits Rectangle{
        in-out property <image> icon;
        in-out property <int> out_radius: 80;
        in-out property <int> inner_radius: 50;
        in-out property <int> begin_angle;
        in-out property <int> end_angle;
        in-out property <bool> active: false;

        Arc {
            active: active;
            out_radius: out_radius;
            inner_radius: inner_radius;
            out_start_x: (out_radius) + sin(begin_angle * 1deg) * out_radius;
            out_start_y: (out_radius) - cos(begin_angle * 1deg) * out_radius;
            out_end_x: (out_radius) + sin(end_angle * 1deg) * out_radius;
            out_end_y: (out_radius) - cos(end_angle * 1deg) * out_radius;
            inner_start_x: (out_radius) + sin(begin_angle * 1deg) * inner_radius;
            inner_start_y: (out_radius) - cos(begin_angle * 1deg) * inner_radius;
            inner_end_x: (out_radius) + sin(end_angle * 1deg) * inner_radius;
            inner_end_y: (out_radius) - cos(end_angle * 1deg) * inner_radius;
        }

        Image {
            colorize: active ? rgb(0, 175, 255) : Palette.foreground;
            source: icon;
            height: root.height / 8;
            width: root.width / 8;
            x: ((out_radius) + sin((begin_angle + end_angle) / 2 * 1deg) * (out_radius + inner_radius) / 2) * 1px - self.width / 2;
            y: ((out_radius) - cos((begin_angle + end_angle) / 2 * 1deg) * (out_radius + inner_radius) / 2) * 1px - self.height / 2;
        }
    }

    export component ToolbarWindow inherits Window {
        background: transparent;
        no-frame: true;
        title: "菜单栏";
        always-on-top: true;

        in-out property <int> win_height: 160;
        in-out property <int> win_width: 160;
        in-out property <int> active_num: -1;

        in-out property <[Tool_slint]> tools: [
            { id: 0, icon: @image-url("./assets/icon/min.svg"), name: "最小化" },
            { id: 1, icon: @image-url("./assets/icon/right.svg"), name: "复制" },
            { id: 2, icon: @image-url("./assets/icon/close.svg"), name: "关闭" },
            { id: 3, icon: @image-url("./assets/icon/save.svg"), name: "保存" },
        ];

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

            for tool in root.tools: ArcBtn{
                icon: tool.icon;
                begin_angle: 360/tools.length * (tool.id) - 360/tools.length/2;
                end_angle: 360/tools.length * (tool.id) + 360/tools.length/2;
                active: active_num == tool.id;
            }

            tool-tips := Text {
                font-size: 14px;
                text: (active_num == -1) ? "" : tools[active_num].name;
                height: root.height;
                width: root.width;
                vertical-alignment: center;
                horizontal-alignment: center;
            }
        }
    }
}