mod amplifier;
mod pin_win;

use std::{sync::{Arc, Mutex, mpsc, mpsc::Sender}, collections::HashMap, rc::Rc};
use slint::{SharedPixelBuffer, Rgba8Pixel};
use i_slint_backend_winit::WinitWindowAccessor;
use global_hotkey::hotkey::{HotKey, Modifiers, Code};
use screenshots::Screen;

use super::{Module, ModuleMessage};
use amplifier::Amplifier;
use pin_win::PinWin;
use pin_win::PinWindow;


pub struct ScreenShotter {
    pub mask_win: MaskWindow,
    id: Option<u32>,
    _max_pin_win_id: Arc<Mutex<u32>>,
    _pin_wins: Rc<Mutex<HashMap<u32, PinWin>>>,
    _amplifier: Amplifier, // 放大取色器
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
        return msg_sender;
    }

    fn get_hotkey(&mut self) -> HotKey {
        let hotkey = HotKey::new(Some(Modifiers::SHIFT), Code::KeyC);
        self.id = Some(hotkey.id());
        return  hotkey;
    }

    fn get_id(&self) -> Option<u32> {
        return self.id;
    }
}

impl ScreenShotter{
    pub fn new() -> ScreenShotter {
        // get screens and info
        let screens = Screen::all().unwrap();
        let mut primary_screen = None ;
        for screen in screens {
            if screen.display_info.is_primary {
                primary_screen = Some(screen);
            }
        }
        
        let mask_win = MaskWindow::new().unwrap(); // init MaskWindow
        let amplifier = Amplifier::new(); // init Amplifier

        mask_win.window().set_position(slint::PhysicalPosition::new(0, 0) );
        mask_win.set_state(0);

        let max_pin_win_id: Arc<Mutex<u32>> = Arc::new(Mutex::new(0));
        let pin_wins: Rc<Mutex<HashMap<u32, PinWin>>> =  Rc::new(Mutex::new(HashMap::new()));
        let pin_windows: Arc<Mutex<HashMap<u32, slint::Weak<PinWindow>>>> =  Arc::new(Mutex::new(HashMap::new()));
        let (move_sender, move_reciever) = mpsc::channel::<u32>();

        let primary_screen_clone = primary_screen.unwrap();
        let mask_win_clone = mask_win.as_weak();
        mask_win.on_shot(move || {
            let mask_win = mask_win_clone.unwrap();
            
            mask_win.set_scale_factor(mask_win.window().scale_factor());
            mask_win.set_state(1);
            mask_win.set_select_rect(Rect{x: -1., y: -1., height: -1., width: -1.});
            let physical_width = primary_screen_clone.display_info.width;
            let physical_height = primary_screen_clone.display_info.height;

            mask_win.set_bac_image(
                slint::Image::from_rgba8(
                    SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                        &primary_screen_clone.capture().unwrap(),
                        physical_width,
                        physical_height,
                    )
                )
            );

            mask_win.show().unwrap();
            // +1 to fix the bug
            mask_win.window().set_size(slint::PhysicalSize::new(primary_screen_clone.display_info.width, primary_screen_clone.display_info.height + 1));
            mask_win.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
                winit_win.focus_window();
            });

            // TODO 显示鼠标放大器
        });

        let mask_win_clone = mask_win.as_weak();
        mask_win.on_key_released(move |event| {
            if event.text == slint::SharedString::from(slint::platform::Key::Escape) {
                mask_win_clone.unwrap().hide().unwrap();
            } else if event.text == *"Z" {
                println!("切换颜色");
            } else if event.text == *"C" {
                println!("复制颜色");
            }
        });

        let mask_win_clone = mask_win.as_weak();
        let max_pin_win_id_clone = max_pin_win_id.clone();
        let pin_wins_clone = pin_wins.clone();
        let pin_windows_clone = pin_windows.clone();
        let move_sender_clone = move_sender.clone();
        mask_win.on_new_pin_win(move |img, rect| {
            let mut max_pin_win_id = max_pin_win_id_clone.lock().unwrap();
            let move_sender_clone = move_sender_clone.clone();

            let pin_win = PinWin::new(img, rect, *max_pin_win_id, move_sender_clone);

            let pin_window_clone = pin_win.pin_window.as_weak();
            let pin_wins_clone_clone = pin_wins_clone.clone();
            let pin_windows_clone_clone = pin_windows_clone.clone();
            let id = *max_pin_win_id;
            pin_window_clone.unwrap().window().on_close_requested(move || {
                pin_wins_clone_clone.lock().unwrap().remove(&id);
                pin_windows_clone_clone.lock().unwrap().remove(&id);
                slint::CloseRequestResponse::HideWindow
            });

            let pin_window_clone = pin_win.pin_window.as_weak();
            pin_wins_clone.lock().unwrap().insert(*max_pin_win_id, pin_win);
            pin_windows_clone.lock().unwrap().insert(*max_pin_win_id, pin_window_clone);

            *max_pin_win_id += 1;
            mask_win_clone.unwrap().hide().unwrap();
        });

        // tile function
        std::thread::spawn(move || {
            loop {
                if let Ok(id) = move_reciever.recv() {
                    let pin_windows = pin_windows.clone();
                    ScreenShotter::pin_win_move_hander(pin_windows, id);
                }
            }
        });

        ScreenShotter{
            id: None,
            mask_win,
            _max_pin_win_id: max_pin_win_id,
            _pin_wins: pin_wins,
            _amplifier: amplifier,
        }
    }

    fn pin_win_move_hander(pin_wins: Arc<Mutex<HashMap<u32, slint::Weak<PinWindow>>>>, move_win_id: u32) {
        slint::invoke_from_event_loop(move || {
            let padding = 10;
            let pin_wins = pin_wins.lock().unwrap();
            let move_win = &pin_wins[&move_win_id].unwrap();
            for pin_win_id in pin_wins.keys(){
                if move_win_id != *pin_win_id {
                    let other_win = &pin_wins[pin_win_id].unwrap();
                    
                    let move_pos = move_win.window().position();
                    let move_size = move_win.window().size();
                    let other_pos = other_win.window().position();
                    let other_size = other_win.window().size();

                    let move_bottom = move_pos.y + move_size.height as i32;
                    let move_right = move_pos.x + move_size.width as i32;
                    let other_bottom = other_pos.y + other_size.height as i32;
                    let other_right = other_pos.x + other_size.width as i32;

                    println!("move_pos: {:?}", move_pos);
                    let mut delta_x = 0;
                    let mut delta_y = 0;
                    
                    if !(move_pos.x > other_right) && !(move_right < other_pos.x) {
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
                    }

                    if !(move_pos.y > other_bottom) && !(move_bottom < other_pos.y) {
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

                    move_win.window().set_position(slint::PhysicalPosition::new(move_pos.x - delta_x, move_pos.y - delta_y));
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
        always-on-top: true;
        forward-focus: focus_scope;
        
        in-out property <image> bac_image;
        in property <float> scale_factor;
        in-out property <Rect> select_rect;
        in-out property <Point> mouse_down_pos;
        in-out property <Point> mouse_move_pos;
        in-out property <int> state; // 0:before shot; 1:shotting before left button press; 2:shotting，left button press

        callback shot();
        callback key_released(KeyEvent);
        callback new_pin_win(image, Rect);

        Image {
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
                                root.mouse_down_pos.x = touch_area.mouse-x;
                                root.mouse_down_pos.y = touch_area.mouse-y;
                                root.mouse_move_pos.x = touch_area.mouse-x;
                                root.mouse_move_pos.y = touch_area.mouse-y;
                            } else if (event.kind == PointerEventKind.up) {
                                root.new_pin_win(root.bac_image, root.select_rect);
                            }
                        }
                    }
                    moved() => {
                        root.mouse_move_pos.x = touch_area.mouse-x;
                        root.mouse_move_pos.y = touch_area.mouse-y;

                        root.select-rect.x = min(root.mouse_down_pos.x, root.mouse_move_pos.x);
                        root.select-rect.y = min(root.mouse_down_pos.y, root.mouse_move_pos.y);
                        root.select-rect.width = abs((root.mouse_move_pos.x - root.mouse_down_pos.x) / 1px) * 1px;
                        root.select-rect.height = abs((root.mouse_move_pos.y - root.mouse_down_pos.y) / 1px) * 1px;
                    }
                }

                select_border := Rectangle {
                    border-color: rgb(0, 175, 255);
                    border-width: 2px;

                    x: root.select-rect.x - self.border-width;
                    y: root.select-rect.y - self.border-width;
                    width: root.select-rect.width + self.border-width * 2;
                    height: root.select-rect.height + self.border-width * 2;

                    select_win := Image {
                        source: bac_image;
                        image-fit: fill;

                        x: select_border.border-width;
                        y: select_border.border-width;
                        width: root.select-rect.width;
                        height: root.select-rect.height;

                        source-clip-x: root.select-rect.x / 1px  * root.scale_factor;
                        source-clip-y: root.select-rect.y / 1px  * root.scale_factor;
                        source-clip-width: root.select-rect.width / 1px  * root.scale_factor;
                        source-clip-height: root.select-rect.height / 1px  * root.scale_factor;
                    }
                }

            }
        }
    }
}