use std::borrow::Cow;
use wfd::DialogParams;
use image;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use arboard::{Clipboard, ImageData};
use slint::{SharedPixelBuffer, Rgba8Pixel};
use i_slint_backend_winit::WinitWindowAccessor;
use chrono;

use crate::core::application::setting::app_config::AppConfig;
use super::Rect;
use super::ShotterMessage;

pub struct PinWin {
    _img_rc: Arc<Mutex<SharedPixelBuffer<Rgba8Pixel>>>,
    _id: u32,
    pub pin_window: PinWindow,
}

impl PinWin {
    pub fn new(img_rc: Arc<Mutex<SharedPixelBuffer<Rgba8Pixel>>>, rect: Rect, offset_x: i32, offset_y: i32, id: u32, message_sender: Sender<ShotterMessage>) -> PinWin {
        let pin_window = PinWindow::new().unwrap();
        let border_width = pin_window.get_win_border_width();

        let scale_factor = pin_window.window().scale_factor();
        pin_window.window().set_position(slint::LogicalPosition::new((rect.x + offset_x as f32) / scale_factor - border_width, (rect.y + offset_y as f32) / scale_factor - border_width));
        pin_window.set_bac_image(slint::Image::from_rgba8((*img_rc.lock().unwrap()).clone()));
        pin_window.set_img_x(rect.x / scale_factor);
        pin_window.set_img_y(rect.y / scale_factor);
        pin_window.set_img_width(rect.width / scale_factor);
        pin_window.set_img_height(rect.height / scale_factor);

        pin_window.set_scale_factor(scale_factor);

        { // code for window move
            let pin_window_clone = pin_window.as_weak();
            let message_sender_clone = message_sender.clone();
            pin_window.on_win_move(move |mut delta_x, mut delta_y| {

                let pin_window_clone = pin_window_clone.unwrap();
                let now_pos = pin_window_clone.window().position().to_logical(pin_window_clone.window().scale_factor());
                let is_stick_x = pin_window_clone.get_is_stick_x();
                let is_stick_y = pin_window_clone.get_is_stick_y();

                if is_stick_x {
                    if delta_x.abs() > 20. {
                        pin_window_clone.set_is_stick_x(false);
                    } else {
                        delta_x = 0.;
                    }
                }
                if is_stick_y {
                    if delta_y.abs() > 20. {
                        pin_window_clone.set_is_stick_y(false);
                    } else {
                        delta_y = 0.;
                    }
                }
                
                if !is_stick_x || !is_stick_y {
                    let change_pos_x = now_pos.x + delta_x;
                    let change_pos_y = now_pos.y + delta_y;
                    pin_window_clone.window().set_position(slint::LogicalPosition::new(change_pos_x, change_pos_y));
                    message_sender_clone.send(ShotterMessage::Move(id)).unwrap();
                }
            });
        }

        { // code for function
            { // for close
                let pin_window_clone = pin_window.as_weak();
                let message_sender_clone = message_sender.clone();
                pin_window.on_close(move || {
                    pin_window_clone.unwrap().hide().unwrap();
                    message_sender_clone.send(ShotterMessage::Close(id)).unwrap();
                });
    
                let pin_window_clone = pin_window.as_weak();
                pin_window.on_hide(move || {
                    pin_window_clone.unwrap().window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
                        winit_win.set_minimized(true);
                    });
                });
            }

            { // for save and copy
                // save
                let pin_window_clone = pin_window.as_weak();
                let img_rc_clone = img_rc.clone();
                let buffer = (*img_rc_clone.lock().unwrap()).clone();
                let scale_factor = pin_window.get_scale_factor();
                let img_x = pin_window.get_img_x() * scale_factor;
                let img_y = pin_window.get_img_y() * scale_factor;
                let img_height = pin_window.get_img_height() * scale_factor;
                let img_width = pin_window.get_img_width() * scale_factor;
                pin_window.on_save(move || {
                    let mut img = image::DynamicImage::ImageRgba8(
                        image::RgbaImage::from_vec(
                            buffer.width() as u32, buffer.height() as u32, buffer.as_bytes().to_vec()
                        ).unwrap()
                    );
                    img = img.crop(img_x as u32, img_y as u32, img_width as u32, img_height as u32);
                    
                    let app_config = AppConfig::global().lock().unwrap();
                    let save_path = app_config.get_save_path();
    
                    let file_name = chrono::Local::now().format("Rotor_%Y-%m-%d-%H-%M-%S.png").to_string();
                    let params = DialogParams {
                        title: "Select an image to save",
                        file_types: vec![("PNG Files", "*.png")],
                        default_extension: "png",
                        file_name: &file_name,
                        default_folder: &save_path,
                        ..Default::default()
                    };
                    pin_window_clone.unwrap().hide().unwrap();
    
                    let dialog_result = wfd::save_dialog(params);
                    if let Ok(file_path_result) = dialog_result {
                        img.save(file_path_result.selected_file_path).unwrap();
                    }
    
                    pin_window_clone.unwrap().invoke_close();
                });

                //copy
                let pin_window_clone = pin_window.as_weak();
                let img_rc_clone = img_rc.clone();
                let buffer = (*img_rc_clone.lock().unwrap()).clone();
                pin_window.on_copy(move || {
                    let mut img = image::DynamicImage::ImageRgba8(
                        image::RgbaImage::from_vec(
                            buffer.width() as u32, buffer.height() as u32, buffer.as_bytes().to_vec()
                        ).unwrap()
                    );
                    img = img.crop(img_x as u32, img_y as u32, img_width as u32, img_height as u32);
    
                    let mut clipboard = Clipboard::new().unwrap();
                    let img_data = ImageData {
                        width: img.width() as usize,
                        height: img.height() as usize,
                        bytes: Cow::from(img.to_rgba8().to_vec())
                    };
                    clipboard.set_image(img_data).unwrap();
                    
                    pin_window_clone.unwrap().invoke_close();
                });
            }

            { // for show toolbar
                let pin_window_clone = pin_window.as_weak();
                let message_sender_clone = message_sender.clone();
                pin_window.on_show_toolbar(move |x, y| {
                    let pin_window = pin_window_clone.unwrap();
                    let position = pin_window.window().position();
                    let scale = pin_window.window().scale_factor();
                    let center_x = position.x + (x * scale) as i32;
                    let center_y = position.y + (y * scale) as i32;
                    message_sender_clone.send(ShotterMessage::ShowToolbar(center_x, center_y)).unwrap();
                });
            }

            { // for move toolbar
                let message_sender_clone = message_sender.clone();
                pin_window.on_move_toolbar(move |x, y| {
                    message_sender_clone.send(ShotterMessage::MoveToolbar(x, y)).unwrap();
                });
            }

            { // for finish toolbar
                let message_sender_clone = message_sender.clone();
                pin_window.on_finish_toolbar(move || {
                    message_sender_clone.send(ShotterMessage::FinishToolbar(id)).unwrap();
                });
            }
        }

        { // code for key press
            let pin_window_clone = pin_window.as_weak();
            pin_window.on_key_release(move |event| {
                let pin_window = pin_window_clone.unwrap();
                if event.text == slint::SharedString::from(slint::platform::Key::Escape) { // close win
                    pin_window.invoke_close();
                } else if event.text == "h" { // hide win
                    pin_window.invoke_hide();
                } else if event.text == "s" { // save pic
                    pin_window.invoke_save();
                } else if event.text == slint::SharedString::from(slint::platform::Key::Return) { // copy pic and close
                    pin_window.invoke_copy();
                }
            });
        }

        pin_window.show().unwrap();
        PinWin {
            _img_rc: img_rc,
            _id: id,
            pin_window,
        }
    }
}

slint::slint! {
    import { Button } from "std-widgets.slint";

    export component PinWindow inherits Window {
        no-frame: true;
        always-on-top: true;
        title: "小云视窗";
        forward-focus: key_focus;
        
        in property <image> bac_image;
        in property <length> win_border_width: 1px;
        in property <float> scale_factor;

        in property <length> img_x;
        in property <length> img_y;
        in-out property <int> zoom_factor: 10; // neet to be divided by ten
        in-out property <length> img_width;
        in-out property <length> img_height;

        in-out property <length> win_width: (img_width * zoom_factor / 10) + win_border_width * 2;
        in-out property <length> win_height: (img_height * zoom_factor / 10) + win_border_width * 2;

        in-out property <bool> can_move: false;
        in-out property <bool> is_stick_x: false;
        in-out property <bool> is_stick_y: false;
        in-out property <bool> is_shown_toolbar: false;
        in-out property <length> toolbar_center_x;
        in-out property <length> toolbar_center_y;

        callback win_move(length, length);
        callback key_release(KeyEvent);
        callback show_toolbar(float, float);
        callback move_toolbar(float, float);
        callback finish_toolbar();

        callback close();
        callback hide();
        callback save();
        callback copy();

        width <=> win_width;
        height <=> win_height;

        image_border := Rectangle {
            border-color: rgb(0, 175, 255);
            border-width: win_border_width;
            pin_image := Image {
                source: bac_image;
                image-fit: contain;

                x: win_border_width;
                y: win_border_width;
                width: (root.width) - win_border_width * 2;
                height: (root.height) - win_border_width * 2;

                source-clip-x: img_x / 1px  * root.scale_factor;
                source-clip-y: img_y / 1px  * root.scale_factor;
                source-clip-width: img_width / 1px  * root.scale_factor;
                source-clip-height: img_height / 1px  * root.scale_factor;

                move_touch_area := TouchArea {
                    mouse-cursor: move;
                    moved => {
                        if (root.can_move) {
                            root.win_move((self.mouse-x) - self.pressed-x, (self.mouse-y) - self.pressed-y);
                        }
                    }
                    
                    pointer-event(event) => {
                        if (event.kind == PointerEventKind.down) {
                            if (event.button == PointerEventButton.left) {
                                root.can_move = true;
                            } else if (event.button == PointerEventButton.right){
                                root.is_shown_toolbar = true;
                                root.toolbar_center_x = self.mouse-x;
                                root.toolbar_center_y = self.mouse-y;
                                root.show_toolbar(self.mouse-x / 1px, self.mouse-y / 1px);
                            }
                        } else if (event.kind == PointerEventKind.up) {
                            if (event.button == PointerEventButton.left) {
                                root.can_move = false;
                            } else if (event.button == PointerEventButton.right){
                                root.is_shown_toolbar = false;
                                root.finish_toolbar();
                            }
                        } else if (event.kind == PointerEventKind.move) {
                            if (root.is_shown_toolbar) {
                                root.move_toolbar(((self.mouse-x) - root.toolbar_center_x) / 1px, ((self.mouse-y) - root.toolbar_center_y) / 1px);
                            }
                        }
                    }

                    scroll-event(event) => {
                        if (event.delta-y > 0) {
                            if (root.zoom_factor < 50) { root.zoom_factor = root.zoom_factor + 1; }
                        } else if (event.delta-y < 0) {
                            if (root.zoom_factor > 2) { root.zoom_factor = (root.zoom_factor) - 1; }
                        }
                        accept
                    }

                    key_focus := FocusScope {
                        key-released(event) => {
                            key_release(event);
                            accept;
                        }
                    }
                }
            }
        }
    }
}