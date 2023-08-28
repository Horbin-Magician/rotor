mod amplifier;
mod pin_win;

use std::sync::{mpsc, Arc, Mutex};

use slint::{SharedPixelBuffer, Rgba8Pixel};
use i_slint_backend_winit::WinitWindowAccessor;
use screenshots::Screen;

use amplifier::Amplifier;
use pin_win::PinWin;

pub struct ScreenShotter {
    pub mask_win: MaskWindow,
    screens: Vec<Screen>,
    pin_wins: Arc<Mutex<Vec<PinWin>>>,
    amplifier: Amplifier, // 放大取色器
}

impl ScreenShotter{
    pub fn new() -> ScreenShotter {
        // get screens and info
        let screens = Screen::all().unwrap();
        let primary_screen = Self::get_prime_screen(&screens).unwrap();
        
        let mask_win = MaskWindow::new().unwrap(); // init MaskWindow
        let amplifier = Amplifier::new(); // init Amplifier


        // there is an animation when the window is first show. The mask window does not need the animation
        mask_win.show().unwrap();
        mask_win.hide().unwrap();

        mask_win.window().set_position(slint::PhysicalPosition::new(0, 0) );
        mask_win.set_state(0);

        let pin_wins: Arc<Mutex<Vec<PinWin>>> =  Arc::new(Mutex::new(Vec::new()));

        let primary_screen_clone = primary_screen.clone();
        let mask_win_clone = mask_win.as_weak();
        mask_win.on_shot(move || {
            let mask_win_clone = mask_win_clone.unwrap();
            mask_win_clone.set_scale_factor(mask_win_clone.window().scale_factor());
            mask_win_clone.set_state(1);
            mask_win_clone.set_select_rect(Rect{x: 0., y: 0., height: 0., width: 0.});
            let physical_width = primary_screen_clone.display_info.width;
            let physical_height = primary_screen_clone.display_info.height;

            mask_win_clone.set_bac_image(
                slint::Image::from_rgba8(
                    SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                        primary_screen_clone.capture().unwrap().rgba(),
                        physical_width,
                        physical_height,
                    )
                )
            );

            mask_win_clone.show().unwrap();
            // +1 to fix the bug
            mask_win_clone.window().set_size(slint::PhysicalSize::new(primary_screen_clone.display_info.width, primary_screen_clone.display_info.height + 1));
            mask_win_clone.window().with_winit_window(|winit_win: &winit::window::Window| {
                winit_win.focus_window();
            });

            // TODO 显示鼠标放大器
        });

        let mask_win_clone = mask_win.as_weak();
        mask_win.on_key_released(move |event| {
            println!("{:?}", event);
            if event.text == slint::SharedString::from(slint::platform::Key::Escape) {
                mask_win_clone.unwrap().hide().unwrap();
            } else if event.text == slint::SharedString::from("Z") {
                println!("切换颜色");
            } else if event.text == slint::SharedString::from("C") {
                println!("复制颜色");
            }
        });

        let mask_win_clone = mask_win.as_weak();
        let pin_wins_clone = pin_wins.clone();
        mask_win.on_new_pin_win(move |img, rect| {
            println!("{:?}", rect);
            let pin_win = PinWin::new(img, rect);
            let mut pin_wins = pin_wins_clone.lock().unwrap();
            let index = pin_wins.len();
            let pin_window_clone = pin_win.pin_window.as_weak();
            pin_wins.push(pin_win);

            let pin_wins_clone = pin_wins_clone.clone();
            pin_window_clone.unwrap().window().on_close_requested(move || {
                pin_wins_clone.lock().unwrap().remove(index);
                slint::CloseRequestResponse::HideWindow
            });
            mask_win_clone.unwrap().hide().unwrap();
        });

        ScreenShotter{
            screens,
            mask_win,
            pin_wins,
            amplifier,
        }
    }

    fn get_prime_screen(screens: &Vec<Screen>) -> Option<&Screen> {
        for screen in screens {
            if screen.display_info.is_primary { return Some(screen); }
        }
        return None;
    }

    // fn new_pin_win(&mut self) {
    //     self.pin_wins.push(pin_win);
    // }

    fn on_hot_key(modifiers: i32, key: i32) {
        // if(m_state == 1)return;
        // if(vk == (UINT)0x43) Shot();
        // else if(vk == (UINT)0x48) HideAll();
    }

    fn end_shot() {
        // m_amplifierTool->hide(); // 隐藏放大器
        // this->hide();
        // m_state = 0;
        // foreach (ShotterWindow* win, m_ShotterWindowList) win->show();
        // if(m_ShotterWindowList.length()>0) m_ShotterWindowList.last()->raise();
        // m_isHidden = false;
    }

    // TODO: 贴贴功能：
    fn on_pin_win_move() {
        // foreach (ShotterWindow* otherWin, m_ShotterWindowList) {
        //     if(otherWin != shotterWindow){
        //         QRect rectA = shotterWindow->geometry();
        //         QRect rectB = otherWin->geometry();
        //         int padding = 10;

        //         if(!(rectA.top() > rectB.bottom()) && !(rectA.bottom() < rectB.top())){
        //             if( qAbs(rectA.right() - rectB.left()) < padding)
        //                 shotterWindow->stick(STICK_TYPE::RIGHT_LEFT, otherWin);
        //             else if( qAbs(rectA.right() - rectB.right()) < padding)
        //                 shotterWindow->stick(STICK_TYPE::RIGHT_RIGHT, otherWin);
        //             else if( qAbs(rectA.left() - rectB.right()) < padding)
        //                 shotterWindow->stick(STICK_TYPE::LEFT_RIGHT, otherWin);
        //             else if( qAbs(rectA.left() - rectB.left()) < padding)
        //                 shotterWindow->stick(STICK_TYPE::LEFT_LEFT, otherWin);
        //         }

        //         if(!(rectA.right() < rectB.left()) && !(rectA.left() > rectB.right())){
        //             if( qAbs(rectA.top() - rectB.bottom()) < padding)
        //                 shotterWindow->stick(STICK_TYPE::UPPER_LOWER, otherWin);
        //             else if( qAbs(rectA.bottom() - rectB.top()) < padding)
        //                 shotterWindow->stick(STICK_TYPE::LOWER_UPPER, otherWin);
        //             else if( qAbs(rectA.top() - rectB.top()) < padding)
        //                 shotterWindow->stick(STICK_TYPE::UPPER_UPPER, otherWin);
        //             else if( qAbs(rectA.bottom() - rectB.bottom()) < padding)
        //                 shotterWindow->stick(STICK_TYPE::LOWER_LOWER, otherWin);
        //         }
        //     }
        // }
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
                    border-color: blue;
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