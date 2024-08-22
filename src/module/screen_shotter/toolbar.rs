use std::sync::mpsc::Sender;
use i_slint_backend_winit::{winit::platform::windows::WindowExtWindows, WinitWindowAccessor};
use slint::ComponentHandle;

use crate::ui::ToolbarWindow;
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

        { // code for show
            let toolbar_window_clone = toolbar_window.as_weak();
            toolbar_window.on_show_pos(move |x, y, id| {
                let toolbar_window = toolbar_window_clone.unwrap();

                // fix the bug of error scale_factor TODO
                let unit_scale = (30. * toolbar_window.window().scale_factor()) as u32;
                let tool_len = toolbar_window.get_tool_len();
                let win_width = tool_len as u32 * unit_scale;
                let win_height = unit_scale;
                toolbar_window.window().set_size(slint::PhysicalSize::new( win_width, win_height));

                let x_pos = x - win_width as i32;
                let y_pos = y;
                toolbar_window.window().set_position(slint::WindowPosition::Physical(slint::PhysicalPosition::new(x_pos, y_pos + 2)));
                toolbar_window.set_pin_focused(true);
                toolbar_window.set_id(id);

                if !toolbar_window.window().is_visible() {
                    toolbar_window.show().unwrap();
                }
            });
        }

        { // code for click
            let message_sender_clone = message_sender.clone();
            toolbar_window.on_click(move |id, active_num| {
                if active_num == 0 {
                    message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::Hide())).unwrap();
                } else if active_num == 1 {
                    message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::Copy())).unwrap();
                } else if active_num == 2 {
                    message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::Close())).unwrap();
                } else if active_num == 3 {
                    message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::Save())).unwrap();
                } else if active_num == 4 {
                    message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::TriggerDraw())).unwrap();
                }
            });
        }

        { // code for hide
            let toolbar_window_clone = toolbar_window.as_weak();
            toolbar_window.on_try_hide(move |if_force| {
                let toolbar_window = toolbar_window_clone.unwrap();
                if !if_force {
                    let toolbar_focused = toolbar_window.get_toolbar_focused();
                    let pin_focused = toolbar_window.get_pin_focused();
                    if toolbar_focused || pin_focused { 
                        toolbar_window.set_pin_focused(false);
                        return;
                    }
                }
                toolbar_window.hide().unwrap();
            });
        }

        { // code for focuse change
            let toolbar_window_clone = toolbar_window.as_weak();
            toolbar_window.on_focus_trick(
                move |pin_focused, toolbar_focused| {
                    if pin_focused || toolbar_focused { return true; }
                    let toolbar_window = toolbar_window_clone.unwrap();
                    toolbar_window.set_id(-1);
                    toolbar_window.hide().unwrap();
                    true
                }
            );
        }

        { // code for win move
            let toolbar_window_clone = toolbar_window.as_weak();
            toolbar_window.on_win_move(move |x, y| {
                let toolbar_window = toolbar_window_clone.unwrap();
                let scale_factor = toolbar_window.window().scale_factor();

                let win_width = toolbar_window.get_win_width() as f32 * scale_factor;
                let x_pos = x - win_width as i32;
                let y_pos = y;
                toolbar_window.window().set_position(slint::WindowPosition::Physical(slint::PhysicalPosition::new(x_pos, y_pos + 2)));
                toolbar_window.show().unwrap();
            });
        }

        Toolbar {
            toolbar_window,
        }
    }

    pub fn get_window(&self) -> slint::Weak<ToolbarWindow> {
        self.toolbar_window.as_weak()
    }
}