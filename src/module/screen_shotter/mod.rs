mod pin_win;
mod toolbar;

use arboard::Clipboard;
use image::{self, GenericImageView, Rgba};
use std::{collections::HashMap, error::Error, sync::{mpsc::{self, Sender}, Arc, Mutex}};
use slint::{ComponentHandle, Rgba8Pixel, SharedPixelBuffer, Weak};
use i_slint_backend_winit::{winit::platform::windows::WindowExtWindows, WinitWindowAccessor};
use global_hotkey::hotkey::HotKey;
use xcap::Monitor;
use windows::Win32::{UI::WindowsAndMessaging::GetCursorPos, Foundation::POINT};

use crate::util::{img_util, log_util, sys_util};
use crate::core::application::app_config::AppConfig;
use crate::ui::{MaskWindow, PinWindow, ToolbarWindow};
use super::{Module, ModuleMessage};
use pin_win::PinWin;
use toolbar::Toolbar;

pub enum PinOperation {
    Close(),
    Hide(),
    Save(),
    Copy(),
    TriggerDraw(),
}

pub enum ShotterMessage {
    Move(u32),
    Close(u32),
    ShowToolbar(i32, i32, u32, Weak<PinWindow>),
    HideToolbar(bool),
    OperatePin(u32, PinOperation),
}

pub struct ScreenShotter {
    _mask_win: MaskWindow,
    _toolbar: Toolbar,
    _max_pin_win_id: Arc<Mutex<u32>>,
    _pin_windows: Arc<Mutex<HashMap<u32, slint::Weak<PinWindow>>>>,
    _pin_wins: Arc<Mutex<HashMap<u32, PinWin>>>,
    _bac_rects: Arc<Mutex<Vec<(u32, u32, u32, u32)>>>,
}

impl Module for ScreenShotter {
    fn flag(&self) -> &str { "screenshot" }

    fn run(&self) -> Sender<ModuleMessage> {
        let (msg_sender, msg_reciever) = mpsc::channel();
        let mask_win_clone = self._mask_win.as_weak();
        std::thread::spawn(move || {
            loop {
                match msg_reciever.recv() {
                    Ok(ModuleMessage::Trigger) => {
                        mask_win_clone.upgrade_in_event_loop(move |win| {
                            win.invoke_shot();
                        }).unwrap_or_else(|e| { log_util::log_error(format!("Error in ScreenShotter Trigger: {:?}", e)); });
                    },
                    Err(e) => {
                        log_util::log_error(format!("Error in ScreenShotter msg_reciever: {:?}", e));
                    },
                }
            }
        });
        msg_sender
    }

    fn get_hotkey(&mut self) -> Option<HotKey> {
        AppConfig::global()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get_hotkey_from_str("screenshot")
    }

    fn clean(&self) {
        *self._max_pin_win_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            = 0;
        self._pin_windows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        self._pin_wins
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }
}

impl ScreenShotter{
    pub fn new() -> Result<ScreenShotter, Box<dyn Error>> {
        let mask_win = MaskWindow::new()?; // init MaskWindow
        sys_util::forbid_window_animation(mask_win.window());
        mask_win.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
            winit_win.set_skip_taskbar(true);
        });

        let max_pin_win_id: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
        let pin_wins: Arc<Mutex<HashMap<u32, PinWin>>> =  Arc::new(Mutex::new(HashMap::new()));
        let pin_windows: Arc<Mutex<HashMap<u32, slint::Weak<PinWindow>>>> =  Arc::new(Mutex::new(HashMap::new()));
        let (message_sender, message_reciever) = mpsc::channel::<ShotterMessage>();

        let toolbar = Toolbar::new(message_sender.clone())?;

        let bac_buffer_rc = Arc::new(Mutex::new(
            SharedPixelBuffer::<Rgba8Pixel>::new(1, 1)
        ));

        let bac_rects = Arc::new(Mutex::new(Vec::new()));

        { // code for shot
            let bac_buffer_rc_clone = Arc::clone(&bac_buffer_rc);
            let mask_win_clone = mask_win.as_weak();
            let bac_rects_clone = bac_rects.clone();
            mask_win.on_shot(move || {
                // get screens and info
                let mut point = POINT{x: 0, y: 0};
                unsafe {
                    GetCursorPos(&mut point)
                        .unwrap_or_else(|e| { log_util::log_error(format!("Error in GetCursorPos: {:?}", e)); })
                }
                
                if let Ok(monitor) = Monitor::from_point(point.x, point.y) {
                    if let Some(mask_win) = mask_win_clone.upgrade() {
                        let monitor_img: image::ImageBuffer<Rgba<u8>, Vec<u8>> = monitor.capture_image().unwrap_or_default();
                        let scale_factor = sys_util::get_scale_factor(monitor.id());
                        
                        // if let Ok(mut bac_rect_guard) = bac_rects_clone.lock() {
                        //     *bac_rect_guard = img_util::detect_rect(&monitor_img);
                        // }

                        // refresh img
                        let mut bac_buffer = bac_buffer_rc_clone.lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        *bac_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                            &monitor_img,
                            monitor_img.width(),
                            monitor_img.height(),
                        );
                        mask_win.set_bac_image(slint::Image::from_rgba8((*bac_buffer).clone()));
    
                        // refresh window
                        let pre_scale_factor = mask_win.get_scale_factor();
                        mask_win.window().set_position(slint::PhysicalPosition::new(monitor.x(), monitor.y()));
                        mask_win.set_offset_x(monitor.x());
                        mask_win.set_offset_y(monitor.y());
                        mask_win.set_scale_factor(scale_factor);
    
                        // +1 to fix the bug and set_fullscreen does not work well TODO: fix this bug
                        let mut scale = 1.0;
                        if pre_scale_factor != 0.0 && pre_scale_factor > scale_factor { scale = pre_scale_factor / scale_factor; } // to fix scale problem
                        let window_width = ((monitor.width() as f32) * scale) as u32;
                        let window_height = ((monitor.height() as f32) * scale) as u32 + 1;
                        mask_win.window().set_size(slint::PhysicalSize::new( window_width, window_height));
    
                        let _ = mask_win.show();
                        mask_win.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
                            winit_win.focus_window();
                        });
                    }
                }
            });
        }

        { // refresh rgb code str
            let mask_win_clone = mask_win.as_weak();
            let bac_buffer_rc_clone = Arc::clone(&bac_buffer_rc);
            let bac_rects_clone = bac_rects.clone();
            mask_win.on_mouse_move(move |mouse_x, mouse_y, color_type_dec, mouse_left_press| {
                // TODO get windows rect
                if mouse_left_press == false { 
                    let scale_factor = mask_win_clone.upgrade().unwrap().window().scale_factor();
                    let mouse_x_phs = (mouse_x * scale_factor) as u32;
                    let mouse_y_phs = (mouse_y * scale_factor) as u32;
                    let bac_rects = bac_rects_clone.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    let mut if_set = false;
                    for (x, y, width , height) in bac_rects.iter() {
                        if *x <= mouse_x_phs && mouse_x_phs <= *x + width && *y <= mouse_y_phs && mouse_y_phs <= *y + height {
                            if let Some(mask_win) = mask_win_clone.upgrade() {
                                mask_win.set_select_rect(
                                    crate::ui::Rect{
                                        x: *x as f32,
                                        y: *y as f32,
                                        width: *width as f32,
                                        height: *height as f32
                                    }
                                );
                                if_set = true;
                                break;
                            }
                        }
                    }
                    if if_set == false {
                        if let Some(mask_win) = mask_win_clone.upgrade() {
                            mask_win.set_select_rect(
                                crate::ui::Rect{ x: -1.0, y: -1.0, width: 0.0, height: 0.0 }
                            );
                        }
                    }
                }

                if let Some(mask_win) = mask_win_clone.upgrade() {
                    let scale_factor = mask_win.window().scale_factor();

                    let bac_buffer = bac_buffer_rc_clone.lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let width = bac_buffer.width();
                    let height = bac_buffer.height();
                    let img: image::DynamicImage = image::DynamicImage::ImageRgba8(
                        image::RgbaImage::from_vec(width, height, bac_buffer.as_bytes().to_vec())
                            .unwrap_or_default()
                    );
                    
                    let point_x = ((mouse_x * scale_factor) as u32).clamp(0, width-1);
                    let point_y = ((mouse_y * scale_factor) as u32).clamp(0, height-1);
                    let pixel: Rgba<u8> = img.get_pixel(point_x, point_y);
                    let (r, g, b) = (pixel[0], pixel[1], pixel[2]);
                    if color_type_dec { mask_win.set_color_str(format!("RGB:({},{},{})", r, g, b).into());
                    } else { mask_win.set_color_str(format!("#{:02X}{:02X}{:02X}", r, g, b).into()); }
                }
            });
        }

        { // code for key release
            let mask_win_clone = mask_win.as_weak();
            mask_win.on_key_released(move |event| {
                if let Some(mask_win) = mask_win_clone.upgrade() {
                    if event.text == slint::SharedString::from(slint::platform::Key::Escape) {
                        mask_win.set_mouse_left_press(false);
                        let _ = mask_win.hide();
                    } else if event.text == "z" || event.text == "Z"  { // switch Dec or Hex
                        let color_type_dec = mask_win.get_color_type_Dec();
                        mask_win.set_color_type_Dec(!color_type_dec);
                    } else if event.text == "c" || event.text == "C" { // copy color code
                        if let Ok(mut clipboard) = Clipboard::new() {
                            clipboard.set_text(mask_win.get_color_str().to_string())
                                .unwrap_or_else(|e| { log_util::log_error(format!("Error in clipboard.set_text: {:?}", e)); });
                        }
                    }
                }
            });
        }

        { // code for new pin_win
            let mask_win_clone = mask_win.as_weak();
            let max_pin_win_id_clone = max_pin_win_id.clone();
            let pin_wins_clone = pin_wins.clone();
            let pin_windows_clone = pin_windows.clone();
            let message_sender_clone = message_sender.clone();
            mask_win.on_new_pin_win(move |rect| {
                if (rect.width * rect.height) < 1. { return; } // ignore too small rect
                if let Some(mask_win) = mask_win_clone.upgrade() {
                    let mut max_pin_win_id = max_pin_win_id_clone.lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let message_sender_clone = message_sender_clone.clone();

                    if let Ok(pin_win) = PinWin::new(
                        bac_buffer_rc.clone(), rect,
                        mask_win.get_offset_x(), mask_win.get_offset_y(), mask_win.get_scale_factor(),
                        *max_pin_win_id, message_sender_clone
                    ) {
                        let pin_window_clone = pin_win.pin_window.as_weak();
                        
                        let pin_wins_clone_clone = pin_wins_clone.clone();
                        let pin_windows_clone_clone = pin_windows_clone.clone();
                        let id = *max_pin_win_id;

                        if let Some(pin_window) = pin_window_clone.upgrade() {
                            pin_window.window().on_close_requested(move || {
                                // this is necessary for systemed close
                                pin_wins_clone_clone.lock()
                                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                                    .remove(&id);
                                pin_windows_clone_clone.lock()
                                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                                    .remove(&id);
                                slint::CloseRequestResponse::HideWindow
                            });
                        }
                        
                        pin_wins_clone.lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .insert(*max_pin_win_id, pin_win);
                        pin_windows_clone.lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner())
                            .insert(*max_pin_win_id, pin_window_clone);
            
                        *max_pin_win_id += 1;
                        let _ = mask_win.hide();
                    }
                }
            });
        }

        // event listen
        let pin_windows_clone = pin_windows.clone();
        // let pin_wins_clone = pin_wins.clone();
        let toolbar_window_clone: slint::Weak<ToolbarWindow> = toolbar.get_window();
        std::thread::spawn(move || {
            loop {
                if let Ok(message) = message_reciever.recv() {
                    match message {
                        ShotterMessage::Move(id) => {
                            ScreenShotter::pin_win_move_hander(pin_windows_clone.clone(), id, toolbar_window_clone.clone());
                        },
                        ShotterMessage::Close(id) => {
                            pin_windows_clone.lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner())
                                .remove(&id);
                            // pin_wins_clone.lock()
                            //     .unwrap_or_else(|poisoned| poisoned.into_inner())
                            //     .remove(&id); // TODO: clear pin_wins
                        },
                        ShotterMessage::ShowToolbar(x, y, id, pin_window) => {
                            toolbar_window_clone.upgrade_in_event_loop(move |win| {
                                win.invoke_show_pos(x, y, id as i32);
                            }).unwrap_or_else(|e| log_util::log_error(format!("Error in change toolbar pos: {:?}", e)));
                            // focus the pin window
                            pin_window.upgrade_in_event_loop(move |win| {
                                win.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
                                    winit_win.focus_window();
                                    winit_win.request_redraw(); // TODO to fix the error win size
                                });
                            }).unwrap_or_else(|e| log_util::log_error(format!("Error in focus the pin window: {:?}", e)));
                        },
                        ShotterMessage::HideToolbar(if_force) => {
                            toolbar_window_clone.upgrade_in_event_loop(move |win| {
                                win.invoke_try_hide(if_force);
                            }).unwrap_or_else(|e| log_util::log_error(format!("Error in HideToolbar: {:?}", e)));
                        },
                        ShotterMessage::OperatePin(id, operation) => {
                            let pin_windows = pin_windows_clone.lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            if let Some(pin_window) = pin_windows.get(&id) {
                                match operation {
                                    PinOperation::Close() => {
                                        pin_window.upgrade_in_event_loop(move |win| {
                                            win.invoke_close();
                                        }).unwrap_or_else(|e| log_util::log_error(format!("Error in Close: {:?}", e)));
                                    },
                                    PinOperation::Hide() => {
                                        pin_window.upgrade_in_event_loop(move |win| {
                                            win.invoke_hide();
                                        }).unwrap_or_else(|e| log_util::log_error(format!("Error in Hide: {:?}", e)));
                                    },
                                    PinOperation::Save() => {
                                        pin_window.upgrade_in_event_loop(move |win| {
                                            win.invoke_save();
                                        }).unwrap_or_else(|e| log_util::log_error(format!("Error in Save: {:?}", e)));
                                    },
                                    PinOperation::Copy() => {
                                        pin_window.upgrade_in_event_loop(move |win| {
                                            win.invoke_copy();
                                        }).unwrap_or_else(|e| log_util::log_error(format!("Error in Copy: {:?}", e)));
                                    },
                                    PinOperation::TriggerDraw() => {
                                        pin_window.upgrade_in_event_loop(move |win| {
                                            win.invoke_trigger_draw();
                                        }).unwrap_or_else(|e| log_util::log_error(format!("Error in TriggerDraw: {:?}", e)));
                                    },
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(ScreenShotter{
            _mask_win: mask_win,
            _toolbar: toolbar,
            _max_pin_win_id: max_pin_win_id,
            _pin_windows: pin_windows,
            _pin_wins: pin_wins,
            _bac_rects: bac_rects,
        })
    }

    fn pin_win_move_hander(pin_windows: Arc<Mutex<HashMap<u32, slint::Weak<PinWindow>>>>, move_win_id: u32, toolbar_window: slint::Weak<ToolbarWindow>) {
        fn inner_run(pin_windows: Arc<Mutex<HashMap<u32, slint::Weak<PinWindow>>>>, move_win_id: u32, toolbar_window: slint::Weak<ToolbarWindow>) -> Result<(), Box<dyn Error>> {
            let padding = 10;
            let pin_windows = pin_windows.lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let move_win = &pin_windows[&move_win_id].upgrade()
                .ok_or("Error in pin_win_move_hander: can get move_win".to_string())?;

            let move_pos = move_win.window().position();
            let move_size = move_win.window().size();
            let move_bottom = move_pos.y + move_size.height as i32;
            let move_right = move_pos.x + move_size.width as i32;

            toolbar_window.upgrade()
                .ok_or("Error in pin_win_move_hander: can get toolbar_window".to_string())?
                .invoke_win_move(move_right, move_bottom);

            for pin_win_id in pin_windows.keys(){
                if move_win_id != *pin_win_id {
                    let other_win = &pin_windows[pin_win_id].upgrade()
                        .ok_or("Error in pin_win_move_hander: can get other_win".to_string())?;
                    let other_pos = other_win.window().position();
                    let other_size = other_win.window().size();
                    let other_bottom = other_pos.y + other_size.height as i32;
                    let other_right = other_pos.x + other_size.width as i32;

                    let mut delta_x = 0;
                    let mut delta_y = 0;
                    
                    if move_pos.x <= other_right && move_right >= other_pos.x && move_pos.y <= other_bottom && move_bottom >= other_pos.y {
                        if (move_right - other_pos.x).abs() < padding {
                            move_win.set_is_stick_x(true);
                            delta_x = move_right - other_pos.x - 2; // -1 to fix the border width
                        } else if (move_right - other_right).abs() < padding {
                            move_win.set_is_stick_x(true);
                            delta_x = move_right - other_right;
                        } else if (move_pos.x - other_right).abs() < padding {
                            move_win.set_is_stick_x(true);
                            delta_x = move_pos.x - other_right + 2;
                        } else if (move_pos.x - other_pos.x).abs() < padding {
                            move_win.set_is_stick_x(true);
                            delta_x = move_pos.x - other_pos.x;
                        }

                        if (move_bottom - other_pos.y).abs() < padding {
                            move_win.set_is_stick_y(true);
                            delta_y = move_bottom - other_pos.y - 2;
                        } else if (move_pos.y - other_bottom).abs() < padding {
                            move_win.set_is_stick_y(true);
                            delta_y = move_pos.y - other_bottom + 2;
                        } else if (move_bottom - other_bottom).abs() < padding {
                            move_win.set_is_stick_y(true);
                            delta_y = move_bottom - other_bottom;
                        } else if (move_pos.y - other_pos.y).abs() < padding {
                            move_win.set_is_stick_y(true);
                            delta_y = move_pos.y - other_pos.y;
                        }
                    }
                    
                    if delta_x != 0 || delta_y != 0 {
                        move_win.window().set_position(slint::PhysicalPosition::new(move_pos.x - delta_x, move_pos.y - delta_y));
                    }
                }
            }
            Ok(())
        }
        
        slint::invoke_from_event_loop(move || {
            inner_run(pin_windows, move_win_id, toolbar_window)
                .unwrap_or_else(|e| { log_util::log_error(format!("Error in pin_win_move_hander: {:?}", e)); });
        }).unwrap_or_else(|e| { log_util::log_error(format!("Error in pin_win_move_hander: {:?}", e)); });
    }

}