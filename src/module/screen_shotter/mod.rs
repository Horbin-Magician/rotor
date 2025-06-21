mod pin_win;
mod toolbar;
mod shotter_record;

use arboard::Clipboard;
use image::{self, GenericImageView, Rgba};
use std::{collections::HashMap, error::Error, sync::{mpsc::{self, Sender}, Arc, Mutex}};
use slint::{ComponentHandle, Rgba8Pixel, SharedPixelBuffer, Weak};
#[cfg(target_os = "windows")] // TODO: enable for macOS
use i_slint_backend_winit::{winit::platform::windows::WindowExtWindows, WinitWindowAccessor};
use global_hotkey::hotkey::HotKey;
use xcap::Monitor;
#[cfg(target_os = "windows")] // TODO: enable for macOS
use windows::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};

use crate::util::{img_util, log_util, sys_util};
use crate::core::application::app_config::AppConfig;
use crate::ui::{MaskWindow, PinWindow, ToolbarWindow, Rect};
use crate::ui::PinState;
use super::{Module, ModuleMessage};
use pin_win::PinWin;
use toolbar::Toolbar;
use shotter_record::ShotterRecord;


pub enum PinOperation {
    Close(),
    Hide(),
    Save(),
    Copy(),
    TriggerDraw(),
    ReturnDraw(),
}

pub enum ShotterMessage {
    Move(u32),
    Close(u32),
    ShowToolbar(i32, i32, u32, PinState, Weak<PinWindow>),
    HideToolbar(bool),
    OperatePin(u32, PinOperation),
    UpdateRecord(u32),
    RestorePinWins,
}

pub struct ScreenShotter {
    _mask_win: MaskWindow,
    _toolbar: Toolbar,
    max_pin_win_id: Arc<Mutex<u32>>,
    pin_windows: Arc<Mutex<HashMap<u32, slint::Weak<PinWindow>>>>,
    pin_wins: Arc<Mutex<HashMap<u32, PinWin>>>,
    _bac_rects: Arc<Mutex<Vec<(u32, u32, u32, u32)>>>,
    _bac_buffer_rc: Arc<Mutex<SharedPixelBuffer<Rgba8Pixel>>>,
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
        *self.max_pin_win_id
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            = 0;
        self.pin_windows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        self.pin_wins
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
    }
}

impl ScreenShotter{
    pub fn new() -> Result<ScreenShotter, Box<dyn Error>> {
        let mask_win = MaskWindow::new()?; // init MaskWindow
        #[cfg(target_os = "windows")] // TODO: enable for macOS
        sys_util::forbid_window_animation(mask_win.window());
        #[cfg(target_os = "windows")] // TODO: enable for macOS
        mask_win.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
            winit_win.set_skip_taskbar(true);
        });

        let max_pin_win_id: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
        let pin_wins: Arc<Mutex<HashMap<u32, PinWin>>> =  Arc::new(Mutex::new(HashMap::new()));
        let pin_windows: Arc<Mutex<HashMap<u32, slint::Weak<PinWindow>>>> =  Arc::new(Mutex::new(HashMap::new()));
        let (message_sender, message_reciever) = mpsc::channel::<ShotterMessage>();

        // set message_sender to AppConfig
        let mut app_config = AppConfig::global()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        app_config.set_shotter_msg_sender(message_sender.clone());
        drop(app_config);

        let toolbar = Toolbar::new(message_sender.clone())?;

        let bac_buffer_rc = Arc::new(Mutex::new(
            SharedPixelBuffer::<Rgba8Pixel>::new(1, 1)
        ));

        let bac_rects = Arc::new(Mutex::new(Vec::new()));

        { // code for shot
            let bac_buffer_rc_clone = bac_buffer_rc.clone();
            let mask_win_clone = mask_win.as_weak();
            mask_win.on_shot(move || {
                // get screens and info
                let mut point = sys_util::get_cursor_pos();
                if let Ok(monitor) = Monitor::from_point(point.0, point.1) {
                    if let Some(mask_win) = mask_win_clone.upgrade() {
                        // refresh img
                        let monitor_img = monitor.capture_image().unwrap_or_default();
                        let mut bac_buffer = bac_buffer_rc_clone.lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        *bac_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                            &monitor_img,
                            monitor_img.width(),
                            monitor_img.height(),
                        );
                        mask_win.set_bac_image(slint::Image::from_rgba8(bac_buffer.clone()));
                        mask_win.set_detected(false);
                        mask_win.set_select_rect( crate::ui::Rect{ x: -1, y: -1, width: 0, height: 0 });
    
                        // refresh window
                        mask_win.window().set_position(slint::PhysicalPosition::new(monitor.x().unwrap_or_default(), monitor.y().unwrap_or_default()));
                        
                        let _ = mask_win.show();
                        #[cfg(target_os = "windows")] // TODO: enable for macOS
                        mask_win.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
                            winit_win.focus_window();
                        });
                    }
                }
            });
        }

        { // refresh rgb code str
            let mask_win_clone = mask_win.as_weak();
            let bac_buffer_rc_clone = bac_buffer_rc.clone();
            let bac_rects_clone = bac_rects.clone();
            mask_win.on_mouse_move(move |mouse_x, mouse_y, color_type_dec, auto_detect| {
                let mask_win = mask_win_clone.upgrade().unwrap();
                let scale_factor = mask_win.window().scale_factor();
                let bac_buffer = bac_buffer_rc_clone.lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let width = bac_buffer.width();
                let height = bac_buffer.height();
                let img = img_util::shared_pixel_buffer_to_dynamic_image(&bac_buffer);
                
                let mouse_x_phs = ((mouse_x * scale_factor) as u32).clamp(0, width-1);
                let mouse_y_phs = ((mouse_y * scale_factor) as u32).clamp(0, height-1);
                let pixel: Rgba<u8> = img.get_pixel(mouse_x_phs, mouse_y_phs);
                let (r, g, b) = (pixel[0], pixel[1], pixel[2]);
                if color_type_dec { mask_win.set_color_str(format!("RGB:({},{},{})", r, g, b).into());
                } else { mask_win.set_color_str(format!("#{:02X}{:02X}{:02X}", r, g, b).into()); }

                // auto detect rect
                if auto_detect {
                    let detected = mask_win.get_detected();
                    if detected == false {
                        if let Ok(mut bac_rect_guard) = bac_rects_clone.lock() {
                            *bac_rect_guard = img_util::detect_rect(&img.to_rgba8());
                        }
                        mask_win.set_detected(true);
                    }
                    let bac_rects = bac_rects_clone.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    #[cfg(target_os = "windows")] // TODO: enable for macOS
                    sys_util::enable_window(mask_win.window(), false);
                    #[cfg(target_os = "windows")] // TODO: enable for macOS
                    let (window_x, window_y, window_width, window_height) = sys_util::get_point_window_rect(mouse_x_phs as i32, mouse_y_phs as i32);
                    #[cfg(target_os = "windows")] // TODO: enable for macOS
                    let window_area = (window_width * window_height) as u32;
                    #[cfg(target_os = "windows")] // TODO: enable for macOS
                    sys_util::enable_window(mask_win.window(), true);

                    let mut if_set = false;
                    for (x, y, width , height) in bac_rects.iter() {
                        #[cfg(target_os = "windows")] // TODO: enable for macOS
                        if width * height > window_area { break; }
                        if *x <= mouse_x_phs && mouse_x_phs <= *x + width && *y <= mouse_y_phs && mouse_y_phs <= *y + height {
                            mask_win.set_select_rect(
                                crate::ui::Rect {
                                    x: *x as i32 + 1,
                                    y: *y as i32 + 1,
                                    width: *width as i32 - 1,
                                    height: *height as i32 - 1
                                }
                            );
                            if_set = true;
                            break;
                        }
                    }
                    if if_set == false {
                        #[cfg(target_os = "windows")] // TODO: enable for macOS
                        mask_win.set_select_rect(
                            crate::ui::Rect{ x: window_x, y: window_y, width: window_width, height: window_height }
                        );
                    }
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
            let bac_buffer_rc_clone = bac_buffer_rc.clone();
            mask_win.on_new_pin_win(move |rect| {
                if (rect.width * rect.height) < 1 { return; } // ignore too small rect
                if let Some(mask_win) = mask_win_clone.upgrade() {
                    let mut max_pin_win_id = max_pin_win_id_clone.lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let message_sender_clone = message_sender_clone.clone();
                    
                    let img = (*bac_buffer_rc_clone).lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .clone();
                    
                    let position =  mask_win.window().position();
                    if let Ok(pin_win) = PinWin::new(
                        img, rect,
                        position.x, position.y,
                        *max_pin_win_id, message_sender_clone
                    ) {
                        let pin_window_clone = pin_win.pin_window.as_weak();
                        
                        ShotterRecord::save_record_img(*max_pin_win_id, bac_buffer_rc_clone.clone());
                        Self::update_pin_win_record(*max_pin_win_id, &pin_win.pin_window);

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

        { // code for new pin_win
            let pin_windows_clone = pin_windows.clone();
            let pin_wins_clone = pin_wins.clone();
            let max_pin_win_id_clone = max_pin_win_id.clone();
            let message_sender_clone = message_sender.clone();
            mask_win.on_restore_pin_wins(move || {
                ScreenShotter::restore_pin_wins(
                    max_pin_win_id_clone.clone(),
                    message_sender_clone.clone(),
                    pin_windows_clone.clone(),
                    pin_wins_clone.clone()
                ).unwrap_or_else(|e| log_util::log_error(format!("Error in restore_pin_wins: {:?}", e)));
            });
        }

        // event listen
        let pin_windows_clone = pin_windows.clone();
        // let pin_wins_clone = pin_wins.clone();
        let toolbar_window_clone: slint::Weak<ToolbarWindow> = toolbar.get_window();
        let mask_win_clone = mask_win.as_weak();
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
                            
                            ShotterRecord::global().lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner())
                                .del_shotter(id)
                                .unwrap_or_else(|e| log_util::log_error(format!("Error in del_shotter: {:?}", e)));
                        },
                        ShotterMessage::ShowToolbar(x, y, id, pin_state, pin_window) => {
                            toolbar_window_clone.upgrade_in_event_loop(move |win| {
                                win.invoke_show_pos(x, y, id as i32);
                                if pin_state == PinState::DrawFree {
                                    win.set_active_name("绘制".into());
                                } else {
                                    win.set_active_name("".into());
                                }
                            }).unwrap_or_else(|e| log_util::log_error(format!("Error in change toolbar pos: {:?}", e)));
                            // focus the pin window
                            pin_window.upgrade_in_event_loop(move |win| {
                                #[cfg(target_os = "windows")] // TODO: enable for macOS
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
                                    PinOperation::ReturnDraw() => {
                                        pin_window.upgrade_in_event_loop(move |win| {
                                            win.invoke_return_draw();
                                        }).unwrap_or_else(|e| log_util::log_error(format!("Error in ReturnDraw: {:?}", e)));
                                    },
                                }
                            }
                        },
                        ShotterMessage::UpdateRecord(id) => {
                            let pin_windows = pin_windows_clone.lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            if let Some(pin_window) = pin_windows.get(&id) {
                                pin_window.upgrade_in_event_loop(move |win| {
                                    Self::update_pin_win_record(id, &win);
                                }).unwrap_or_else(|e| log_util::log_error(format!("Error in update pin_win record: {:?}", e)));
                            }
                        },
                        ShotterMessage::RestorePinWins => {
                            mask_win_clone.upgrade_in_event_loop(move |win| {
                                win.invoke_restore_pin_wins();
                            }).unwrap_or_else(|e| log_util::log_error(format!("Error in RestorePinWins: {:?}", e)));
                        }, 
                    }
                }
            }
        });
        
        message_sender.send(ShotterMessage::RestorePinWins)
            .unwrap_or_else(|e| log_util::log_error(format!("Error in send RestorePinWins: {:?}", e)));

        Ok(ScreenShotter{
            _mask_win: mask_win,
            _toolbar: toolbar,
            max_pin_win_id,
            pin_windows,
            pin_wins,
            _bac_rects: bac_rects,
            _bac_buffer_rc: bac_buffer_rc,
        })
    }
    
    fn update_pin_win_record(id:u32, pin_win: &PinWindow) {
        let position = pin_win.window().position();
        let rect_x = pin_win.get_img_x();
        let rect_y = pin_win.get_img_y();
        let rect_width = pin_win.get_img_width();
        let rect_height = pin_win.get_img_height();
        ShotterRecord::global().lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .update_shotter(id, shotter_record::ShotterConfig{
                pos_x: position.x,
                pos_y: position.y,
                rect: (rect_x as i32, rect_y as i32, rect_width, rect_height),
                zoom_factor: pin_win.get_zoom_factor(),
            }).unwrap_or_else(|e| log_util::log_error(format!("Error in update_shotter: {:?}", e)));
    }

    fn restore_pin_wins(
        max_pin_win_id: Arc<Mutex<u32>>,
        message_sender: Sender<ShotterMessage>,
        pin_windows: Arc<Mutex<HashMap<u32, slint::Weak<PinWindow>>>>,
        pin_wins: Arc<Mutex<HashMap<u32, PinWin>>>,
    ) -> Result<(), Box<dyn Error>> {
        // clear all pin win
        {
            let mut pin_windows = pin_windows.lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            for (_, pin_win) in pin_windows.iter() {
                if let Some(pin_win) = pin_win.upgrade() {
                    pin_win.window().hide().unwrap();
                }
            }
            pin_windows.clear();
            pin_wins.lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clear();
        }
        let mut max_id = 0;

        let mut record = ShotterRecord::global().lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let shotters = record.get_shotters();

        let mut invalid_shotter_ids = Vec::new();

        if let Some(shotters) = shotters {
            for (id, shotter) in shotters {
                let id = id.parse::<u32>()?;
                if (id+1) > max_id { max_id = id + 1; }
    
                let img_buffer;
                if let Ok(img) = ShotterRecord::load_record_img(id) {
                    img_buffer = img;
                } else {
                    invalid_shotter_ids.push(id);
                    log_util::log_error(format!("Error in restore_pin_wins: can not load img, id: {}", id));
                    continue;
                }
    
                let message_sender_clone = message_sender.clone();
                let rect = Rect{x: shotter.rect.0, y: shotter.rect.1, width: shotter.rect.2, height: shotter.rect.3};
    
                let mut pos_x = 0;
                let mut pos_y = 0;
                if let Ok(_) = Monitor::from_point(shotter.pos_x, shotter.pos_y) {
                    pos_x = shotter.pos_x;
                    pos_y = shotter.pos_y;
                } else {
                    for m in Monitor::all().unwrap() {
                        if m.is_primary().unwrap_or_else(|_|false) { 
                            pos_x = m.x().unwrap_or_default();
                            pos_y = m.y().unwrap_or_default();
                        }
                    }
                }
                
                let offset_x = pos_x - rect.x as i32;
                let offset_y = pos_y - rect.y as i32;
                let pin_win = PinWin::new(
                    img_buffer, rect, offset_x, offset_y, id, message_sender_clone
                )?;
                pin_win.pin_window.set_zoom_factor(shotter.zoom_factor);
    
                pin_windows.lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .insert(id, pin_win.pin_window.as_weak());
                pin_wins.lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .insert(id, pin_win);
            }
        }

        for id in invalid_shotter_ids {
            record.del_shotter(id)?;
        }

        *max_pin_win_id.lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = max_id;
        
        Ok(())
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