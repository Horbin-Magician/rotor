mod pin_win;
mod toolbar;

use arboard::Clipboard;
use image::{self, GenericImageView, Rgba};
use std::{sync::{Arc, Mutex, mpsc, mpsc::Sender}, collections::HashMap};
use slint::{Rgba8Pixel, SharedPixelBuffer, Weak};
use i_slint_backend_winit::{winit::platform::windows::WindowExtWindows, WinitWindowAccessor};
use global_hotkey::hotkey::{HotKey, Modifiers, Code};
use xcap::Monitor;
use windows_sys::Win32::{UI::WindowsAndMessaging::GetCursorPos, Foundation::POINT};
use windows_sys::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

use super::{Module, ModuleMessage};
use pin_win::PinWin;
use pin_win::PinWindow;
use toolbar::Toolbar;

pub enum PinOperation {
    Close(),
    Hide(),
    Save(),
    Copy(),
}

pub enum ShotterMessage {
    Move(u32),
    Close(u32),
    ShowToolbar(i32, i32, u32, Weak<PinWindow>),
    HideToolbar(bool),
    OperatePin(u32, PinOperation),
}

pub struct ScreenShotter {
    pub mask_win: MaskWindow,
    id: Option<u32>,
    _max_pin_win_id: Arc<Mutex<u32>>,
    _pin_wins: Arc<Mutex<HashMap<u32, PinWin>>>,
    _toolbar: Toolbar,
}

impl Module for ScreenShotter {
    fn run(&self) -> Sender<ModuleMessage> {
        let (msg_sender, msg_reciever) = mpsc::channel();
        let mask_win_clone = self.mask_win.as_weak();
        std::thread::spawn(move || {
            loop {
                match msg_reciever.recv().unwrap() {
                    ModuleMessage::Trigger => {
                        mask_win_clone.upgrade_in_event_loop(move |win| {
                            win.invoke_shot();
                        }).unwrap();
                    }
                }
            }
        });
        msg_sender
    }

    fn get_hotkey(&mut self) -> HotKey {
        let hotkey = HotKey::new(Some(Modifiers::SHIFT), Code::KeyC);
        self.id = Some(hotkey.id());
        hotkey
    }

    fn get_id(&self) -> Option<u32> {
        self.id
    }
}

impl ScreenShotter{
    pub fn new() -> ScreenShotter {
        let mask_win = MaskWindow::new().unwrap(); // init MaskWindow
        mask_win.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
            winit_win.set_skip_taskbar(true);
        });

        let max_pin_win_id: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
        let pin_wins: Arc<Mutex<HashMap<u32, PinWin>>> =  Arc::new(Mutex::new(HashMap::new()));
        let pin_windows: Arc<Mutex<HashMap<u32, slint::Weak<PinWindow>>>> =  Arc::new(Mutex::new(HashMap::new()));
        let (message_sender, message_reciever) = mpsc::channel::<ShotterMessage>();

        let toolbar = Toolbar::new(message_sender.clone());

        let bac_buffer_rc = Arc::new(Mutex::new(
            SharedPixelBuffer::<Rgba8Pixel>::new(1, 1)
        ));

        { // code for shot
            let bac_buffer_rc_clone = Arc::clone(&bac_buffer_rc);
            let mask_win_clone = mask_win.as_weak();
            mask_win.on_shot(move || {
                // get screens and info
                let mut point = POINT{x: 0, y: 0};
                unsafe { GetCursorPos(&mut point); }
                let monitor = Monitor::from_point(point.x, point.y).unwrap();
                let physical_width = monitor.width();
                let physical_height = monitor.height();
                let monitor_img = monitor.capture_image().unwrap();
                let scale_factor = unsafe{ 
                    let mut dpi_x: u32 = 0;
                    let mut dpi_y: u32 = 0;
                    GetDpiForMonitor(monitor.id() as isize, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
                    dpi_x as f32 / 96.0
                };

                let mask_win = mask_win_clone.unwrap();

                // refresh img
                let mut bac_buffer = bac_buffer_rc_clone.lock().unwrap();
                *bac_buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                    &monitor_img,
                    physical_width,
                    physical_height,
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

                mask_win.show().unwrap();
                mask_win.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
                    winit_win.focus_window();
                });
            });
        }

        { // refresh rgb code str
            let mask_win_clone = mask_win.as_weak();
            let bac_buffer_rc_clone = Arc::clone(&bac_buffer_rc);
            mask_win.on_refresh_rgb_trick(move |mouse_x, mouse_y, color_type_dec| {
                let mask_win = mask_win_clone.unwrap();
                let scale_factor = mask_win.window().scale_factor();

                let bac_buffer = bac_buffer_rc_clone.lock().unwrap();
                let width = bac_buffer.width();
                let height = bac_buffer.height();
                let img: image::DynamicImage = image::DynamicImage::ImageRgba8(
                    image::RgbaImage::from_vec(width, height, bac_buffer.as_bytes().to_vec()).unwrap()
                );
                
                let point_x = ((mouse_x * scale_factor) as u32).clamp(0, width-1);
                let point_y = ((mouse_y * scale_factor) as u32).clamp(0, height-1);
                let pixel: Rgba<u8> = img.get_pixel(point_x, point_y);
                let (r, g, b) = (pixel[0], pixel[1], pixel[2]);
                if color_type_dec { mask_win.set_color_str(format!("RGB:({},{},{})", r, g, b).into());
                } else { mask_win.set_color_str(format!("#{:02X}{:02X}{:02X}", r, g, b).into()); }
                true
            });
        }

        { // code for key release
            let mask_win_clone = mask_win.as_weak();
            mask_win.on_key_released(move |event| {
                let mask_win = mask_win_clone.unwrap();
                if event.text == slint::SharedString::from(slint::platform::Key::Escape) {
                    mask_win.set_mouse_left_press(false);
                    mask_win.hide().unwrap();
                } else if event.text == "z" || event.text == "Z"  { // switch Dec or Hex
                    let color_type_dec = mask_win_clone.unwrap().get_color_type_Dec();
                    mask_win.set_color_type_Dec(!color_type_dec);
                } else if event.text == "c" || event.text == "C" { // copy color code
                    let mut clipboard = Clipboard::new().unwrap();
                    clipboard.set_text(mask_win.get_color_str().to_string()).unwrap();
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
                let mask_win = mask_win_clone.unwrap();
                let mut max_pin_win_id = max_pin_win_id_clone.lock().unwrap();
                let message_sender_clone = message_sender_clone.clone();
                let pin_win = PinWin::new(
                    bac_buffer_rc.clone(), rect,
                    mask_win.get_offset_x(), mask_win.get_offset_y(), mask_win.get_scale_factor(),
                    *max_pin_win_id, message_sender_clone
                );
                
                let pin_window_clone = pin_win.pin_window.as_weak();
                
                let pin_wins_clone_clone = pin_wins_clone.clone();
                let pin_windows_clone_clone = pin_windows_clone.clone();
                let id = *max_pin_win_id;
                pin_window_clone.unwrap().window().on_close_requested(move || {
                    // this is necessary for systemed close
                    pin_wins_clone_clone.lock().unwrap().remove(&id);
                    pin_windows_clone_clone.lock().unwrap().remove(&id);
                    slint::CloseRequestResponse::HideWindow
                });
                
                pin_wins_clone.lock().unwrap().insert(*max_pin_win_id, pin_win);
                pin_windows_clone.lock().unwrap().insert(*max_pin_win_id, pin_window_clone);
    
                *max_pin_win_id += 1;
                mask_win.hide().unwrap();
            });
        }

        // event listen
        let pin_windows_clone = pin_windows.clone();
        // let pin_wins_clone = pin_wins.clone();
        let toolbar_window_clone: slint::Weak<toolbar::ToolbarWindow> = toolbar.get_window();
        std::thread::spawn(move || {
            loop {
                if let Ok(message) = message_reciever.recv() {
                    match message {
                        ShotterMessage::Move(id) => {
                            ScreenShotter::pin_win_move_hander(pin_windows.clone(), id, toolbar_window_clone.clone());
                        },
                        ShotterMessage::Close(id) => {
                            pin_windows_clone.lock().unwrap().remove(&id);
                            // pin_wins_clone.lock().unwrap().remove(&id); // TODO: clear pin_wins
                        },
                        ShotterMessage::ShowToolbar(x, y, id, pin_window) => {
                            toolbar_window_clone.upgrade_in_event_loop(move |win| {
                                win.invoke_show_pos(x, y, id as i32);
                            }).unwrap();
                            // focus the pin window
                            pin_window.upgrade_in_event_loop(move |win| {
                                win.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
                                    winit_win.focus_window();
                                    winit_win.request_redraw(); // TODO to fix the error win size
                                });
                            }).unwrap();
                        },
                        ShotterMessage::HideToolbar(if_force) => {
                            toolbar_window_clone.upgrade_in_event_loop(move |win| {
                                win.invoke_try_hide(if_force);
                            }).unwrap();
                        },
                        ShotterMessage::OperatePin(id, operation) => {
                            let pin_windows = pin_windows_clone.lock().unwrap();
                            if let Some(pin_window) = pin_windows.get(&id) {
                                match operation {
                                    PinOperation::Close() => {
                                        pin_window.upgrade_in_event_loop(move |win| {
                                            win.invoke_close();
                                        }).unwrap();
                                    },
                                    PinOperation::Hide() => {
                                        pin_window.upgrade_in_event_loop(move |win| {
                                            win.invoke_hide();
                                        }).unwrap();
                                    },
                                    PinOperation::Save() => {
                                        pin_window.upgrade_in_event_loop(move |win| {
                                            win.invoke_save();
                                        }).unwrap();
                                    },
                                    PinOperation::Copy() => {
                                        pin_window.upgrade_in_event_loop(move |win| {
                                            win.invoke_copy();
                                        }).unwrap();
                                    },
                                }
                            }
                        }
                    }
                }
            }
        });

        ScreenShotter{
            id: None,
            mask_win,
            _max_pin_win_id: max_pin_win_id,
            _pin_wins: pin_wins,
            _toolbar: toolbar,
        }
    }

    fn pin_win_move_hander(pin_windows: Arc<Mutex<HashMap<u32, slint::Weak<PinWindow>>>>, move_win_id: u32, toolbar_window: slint::Weak<toolbar::ToolbarWindow>) {
        slint::invoke_from_event_loop(move || {
            let padding = 10;
            let pin_windows = pin_windows.lock().unwrap();
            let move_win = &pin_windows[&move_win_id].unwrap();

            let move_pos = move_win.window().position();
            let move_size = move_win.window().size();
            let move_bottom = move_pos.y + move_size.height as i32;
            let move_right = move_pos.x + move_size.width as i32;

            toolbar_window.unwrap().invoke_win_move(move_right, move_bottom);

            for pin_win_id in pin_windows.keys(){
                if move_win_id != *pin_win_id {
                    let other_win = &pin_windows[pin_win_id].unwrap();
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
        }).unwrap();
    }

}

slint::slint! {
    struct Rect {
        x: length,
        y: length,
        width: length,
        height: length,
    }
    export component MaskWindow inherits Window {
        no-frame: true;
        forward-focus: focus_scope;
        title: "截图窗口";
        
        property <Rect> select_rect;
        property <Point> mouse_down_pos;
        property <Point> mouse_move_pos;

        in property <image> bac_image;
        
        in-out property <bool> mouse_left_press;
        in-out property <float> scale_factor: 0;
        in-out property <int> offset_x;
        in-out property <int> offset_y;
        
        in-out property <bool> color_type_Dec: true;
        in-out property <string> color_str: "RGB:(???,???,???)";
        
        callback shot();
        callback key_released(KeyEvent);
        callback new_pin_win(Rect);
        pure callback refresh_rgb_trick(float, float, bool) -> bool;

        always-on-top: refresh_rgb_trick(touch_area.mouse-x / 1px, touch_area.mouse-y / 1px, color_type_Dec);

        bac-img := Image {
            height: 100%;
            width: 100%;

            source: bac_image;
            Rectangle {
                background: black.with_alpha(0.5);
                focus_scope := FocusScope {
                    key-released(event) => {
                        root.key_released(event);
                        accept
                    }
                }

                touch_area := TouchArea {
                    mouse-cursor: crosshair;

                    pointer-event(event) => {
                        if(event.button == PointerEventButton.left) {
                            if(event.kind == PointerEventKind.down) {
                                root.mouse_left_press = true;
                                root.mouse_down_pos.x = clamp(touch_area.mouse-x, 0, root.width);
                                root.mouse_down_pos.y = clamp(touch_area.mouse-y, 0, root.height);
                                root.mouse_move_pos.x = clamp(touch_area.mouse-x, 0, root.width);
                                root.mouse_move_pos.y = clamp(touch_area.mouse-y, 0, root.height);
                            } else if (event.kind == PointerEventKind.up) {
                                root.new_pin_win(root.select_rect);
                                root.mouse_left_press = false;
                                root.select-rect.x = -1px;
                                root.select-rect.y = -1px;
                                root.select-rect.width = 0px;
                                root.select-rect.height = 0px;
                            }
                        }
                    }
                    moved() => {
                        if(mouse_left_press == true) {
                            root.mouse_move_pos.x = clamp(touch_area.mouse-x, 0, (root.width)-1px);
                            root.mouse_move_pos.y = clamp(touch_area.mouse-y, 0, (root.height)-1px);

                            root.select-rect.x = ceil(min(root.mouse_down_pos.x, root.mouse_move_pos.x) / 1px  * root.scale_factor) * 1px;
                            root.select-rect.y = ceil(min(root.mouse_down_pos.y, root.mouse_move_pos.y) / 1px  * root.scale_factor) * 1px;
                            root.select-rect.width = ceil(abs(( (root.mouse_move_pos.x) - root.mouse_down_pos.x) / 1px)  * root.scale_factor) * 1px;
                            root.select-rect.height = ceil(abs(( (root.mouse_move_pos.y) - root.mouse_down_pos.y) / 1px)  * root.scale_factor) * 1px;
                        }
                    }
                }

                select_border := Rectangle {
                    border-color: rgb(0, 175, 255);
                    border-width: 1px;

                    x: root.select-rect.x / (root.scale_factor) - self.border-width;
                    y: root.select-rect.y / (root.scale_factor) - self.border-width;
                    width: root.select-rect.width / root.scale_factor + self.border-width * 2;
                    height: root.select-rect.height / root.scale_factor + self.border-width * 2;

                    select_win := Image {
                        source: bac_image;
                        image-fit: fill;

                        x: select_border.border-width;
                        y: select_border.border-width;
                        width: root.select-rect.width / root.scale_factor;
                        height: root.select-rect.height / root.scale_factor;
                        
                        source-clip-x: root.select-rect.x / 1px;
                        source-clip-y: root.select-rect.y / 1px;
                        source-clip-width: root.select-rect.width / 1px;
                        source-clip-height: root.select-rect.height / 1px;
                    }
                }
            }
        }

        amplifier := Rectangle {
            width: 120px;
            height:146px; // 90px + 50px + 2px * 3
            x: ((touch-area.mouse-x) + self.width > root.width) ? 
                min(touch-area.mouse-x, root.width) - self.width : max(touch-area.mouse-x, 0);
            y: ((touch-area.mouse-y) + 25px + self.height > root.height) ? 
                min((touch-area.mouse-y) - 25px, root.height) - self.height : max(touch-area.mouse-y + 25px, 0);
            background: black.with_alpha(0.6);

            VerticalLayout {
                alignment: start;
                spacing: 2px;
                Rectangle {
                    width: 100%;
                    height: 90px;
                    border-width: 1px;
                    border-color: white;
                    
                    Image {
                        width: (parent.width) - 2px;
                        height: (parent.height) - 2px;
                        source: bac_image;
                        image-fit: fill;
                        source-clip-x: (touch-area.mouse-x / 1px - self.width / 8px) * root.scale_factor;
                        source-clip-y: (touch-area.mouse-y / 1px - self.height / 8px) * root.scale_factor;
                        source-clip-width: (self.width / 4px) * root.scale_factor;
                        source-clip-height: (self.height / 4px) * root.scale_factor;
                    }
                }

                Text {
                    horizontal-alignment: center;
                    text: touch_area.pressed ?
                        @tr(
                            "宽{}×高{}",
                            round(root.select_rect.width / 1px * root.scale-factor),
                            round(root.select_rect.height / 1px * root.scale-factor)
                        ) : "左键划选区域";
                    color: white;
                } // draw width and height

                Text {
                    horizontal-alignment: center;
                    text: color_str;
                    color: white;
                } // draw RGB color code

                Text {
                    horizontal-alignment: center;
                    text: "Z键切换 C键复制";
                    color: white;
                } // draw tips
            }

            // draw cross curve
            Path {
                y: 0px;
                width: 100%;
                height: 90px;
                commands: "M 60 0 v 90";
                stroke: rgba(0, 180, 255, 0.7);
                stroke-width: 2px;
            } // draw vertical lines

            Path {
                y: 0px;
                width: 100%;
                height: 90px;
                commands: "M 0 45 L 120 45";
                stroke: rgba(0, 180, 255, 0.7);
                stroke-width: 2px;
            } // draw horizontal lines
        }
    }
}