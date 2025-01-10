use std::error::Error;
use std::sync::mpsc::Sender;
use i_slint_backend_winit::{winit::platform::windows::WindowExtWindows, WinitWindowAccessor};
use slint::ComponentHandle;

use crate::sys_util;
use crate::util::log_util;
use crate::{core::application::app_config::AppConfig, ui::ToolbarWindow};
use super::{PinOperation, ShotterMessage};

pub struct Toolbar {
    pub toolbar_window: ToolbarWindow,
}

impl Toolbar {
    pub fn new(message_sender: Sender<ShotterMessage>) -> Result<Toolbar, Box<dyn Error>> {
        let toolbar_window = ToolbarWindow::new()?;
        sys_util::forbid_window_animation(toolbar_window.window());
        toolbar_window.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
            winit_win.set_skip_taskbar(true);
        });

        let mut app_config = AppConfig::global().lock()?;
        toolbar_window.invoke_change_theme(app_config.get_theme() as i32);
        app_config.toolbar_win = Some(toolbar_window.as_weak());

        { // code for show
            let toolbar_window_clone = toolbar_window.as_weak();
            toolbar_window.on_show_pos(move |x, y, id| {
                if let Some(toolbar_window) = toolbar_window_clone.upgrade() {
                    // trick: fix the bug of error scale_factor
                    let tool_len = toolbar_window.get_tool_len() as f32;
                    let divi_len = toolbar_window.get_divi_len() as f32;
                    let win_width = tool_len * 30. + divi_len * 8.;
                    let win_height = 30.;
                    toolbar_window.window().set_size(slint::LogicalSize::new( win_width, win_height));

                    let x_pos = x - (win_width * toolbar_window.window().scale_factor()) as i32;
                    let y_pos = y;
                    toolbar_window.window().set_position(slint::WindowPosition::Physical(slint::PhysicalPosition::new(x_pos, y_pos + 2)));
                    toolbar_window.set_pin_focused(true);

                    let old_id = toolbar_window.get_id();
                    if !toolbar_window.window().is_visible() {
                        let _ = toolbar_window.show();
                    } else if old_id != id {
                        let _ = toolbar_window.hide(); // Prevents being blocked by other pin_wins
                        let _ = toolbar_window.show();
                        toolbar_window.set_id(id);
                    }
                }
            });
        }

        { // code for click
            let message_sender_clone = message_sender.clone();
            toolbar_window.on_click(move |id, active_num| {
                if active_num == 0 {
                    let _ = message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::Hide()));
                } else if active_num == 1 {
                    let _ = message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::Copy()));
                } else if active_num == 2 {
                    let _ = message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::Close()));
                } else if active_num == 3 {
                    let _ = message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::Save()));
                } else if active_num == 4 {
                    let _ = message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::TriggerDraw()));
                } else if active_num == 5 {
                    let _ = message_sender_clone.send(ShotterMessage::OperatePin(id as u32, PinOperation::ReturnDraw()));
                }
            });
        }

        { // code for hide
            let toolbar_window_clone = toolbar_window.as_weak();
            toolbar_window.on_try_hide(move |if_force| {
                if let Some(toolbar_window) = toolbar_window_clone.upgrade() {
                    if !if_force {
                        let toolbar_focused = toolbar_window.get_toolbar_focused();
                        let pin_focused = toolbar_window.get_pin_focused();
                        if toolbar_focused || pin_focused { 
                            toolbar_window.set_pin_focused(false);
                            return;
                        }
                    }
                    toolbar_window.hide()
                        .unwrap_or_else(|e| log_util::log_error(format!("toolbar_window hide error: {:?}", e)));
                }
            });
        }

        { // code for focuse change
            let toolbar_window_clone = toolbar_window.as_weak();
            toolbar_window.on_focus_trick(
                move |pin_focused, toolbar_focused| {
                    if pin_focused || toolbar_focused { return true; }
                    let toolbar_window_clone = toolbar_window_clone.clone();
                    std::thread::spawn(move || { // a trick to avoid pin_focused do not change in time
                        std::thread::sleep(std::time::Duration::from_millis(10));
                        toolbar_window_clone.upgrade_in_event_loop(
                            move |toolbar_window| {
                                if toolbar_window.get_pin_focused() == true { return; }
                                toolbar_window.set_id(-1);
                                toolbar_window.hide()
                                    .unwrap_or_else(|e| log_util::log_error(format!("toolbar_window hide error: {:?}", e)));
                            }
                        )
                    });

                    true
                }
            );
        }

        { // code for win move
            let toolbar_window_clone = toolbar_window.as_weak();
            toolbar_window.on_win_move(move |x, y| {
                if let Some(toolbar_window) = toolbar_window_clone.upgrade() {
                    let win_width = toolbar_window.window().size().width;
                    let x_pos = x - win_width as i32;
                    let y_pos = y;
                    toolbar_window.window().set_position(slint::WindowPosition::Physical(slint::PhysicalPosition::new(x_pos, y_pos + 2)));
                    toolbar_window.show()
                        .unwrap_or_else(|e| log_util::log_error(format!("toolbar_window show error: {:?}", e)));
                }
            });
        }

        Ok(Toolbar {
            toolbar_window,
        })
    }

    pub fn get_window(&self) -> slint::Weak<ToolbarWindow> {
        self.toolbar_window.as_weak()
    }
}