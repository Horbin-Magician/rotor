use std::borrow::Cow;
use wfd::DialogParams;
use image;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use arboard::{Clipboard, ImageData};
use slint::{Model, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel, ComponentHandle};
use i_slint_backend_winit::WinitWindowAccessor;
use chrono;

use crate::util::sys_util;
use crate::core::application::app_config::AppConfig;
use crate::ui::{PinWindow, Rect, Direction, PinState};
use super::ShotterMessage;

pub struct PinWin {
    _img_rc: Arc<Mutex<SharedPixelBuffer<Rgba8Pixel>>>,
    _id: u32,
    pub pin_window: PinWindow,
}

impl PinWin {
    pub fn new(img_rc: Arc<Mutex<SharedPixelBuffer<Rgba8Pixel>>>, rect: Rect, offset_x: i32, offset_y: i32, true_scale_factor: f32, id: u32, message_sender: Sender<ShotterMessage>) -> PinWin {
        let pin_window = PinWindow::new().unwrap();
        sys_util::forbid_window_animation(pin_window.window());

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
            pin_window.on_win_move(move |mut delta_x, mut delta_y, mouse_direction| {
                let pin_window_clone = pin_window_clone.unwrap();
                let now_pos = pin_window_clone.window().position().to_logical(pin_window_clone.window().scale_factor());
                let mut img_x = pin_window_clone.get_img_x();
                let mut img_y = pin_window_clone.get_img_y();
                let mut img_width = pin_window_clone.get_img_width();
                let mut img_height = pin_window_clone.get_img_height();
                let zoom_factor = pin_window_clone.get_zoom_factor() as f32 / 100.;
                
                let limit_px = 5.;
                let img_width_px = img_width * zoom_factor;
                let img_height_px = img_height * zoom_factor;
                match mouse_direction {
                    Direction::Center => {
                        let is_stick_x = pin_window_clone.get_is_stick_x();
                        let is_stick_y = pin_window_clone.get_is_stick_y();
                        if is_stick_x {
                            if delta_x.abs() > 20. { pin_window_clone.set_is_stick_x(false); }
                            else { delta_x = 0.; }
                        }
                        if is_stick_y {
                            if delta_y.abs() > 20. { pin_window_clone.set_is_stick_y(false); }
                            else { delta_y = 0.; }
                        }
                        if is_stick_x && is_stick_y { return; }
                    },
                    Direction::Upper => {
                        if (img_height_px - delta_y) < limit_px { delta_y = img_height_px - limit_px; }
                        img_y += delta_y / zoom_factor;
                        img_height -= delta_y / zoom_factor;
                        delta_x = 0.;
                    },
                    Direction::Lower => {
                        if (img_height_px + delta_y) < limit_px { delta_y = limit_px - img_height_px; }
                        pin_window_clone.set_delta_img_height(delta_y / zoom_factor);
                        delta_x = 0.;
                        delta_y = 0.;
                    },
                    Direction::Left => {
                        if (img_width_px - delta_x) < limit_px { delta_x = img_width_px - limit_px; }
                        img_x += delta_x / zoom_factor;
                        img_width -= delta_x / zoom_factor;
                        delta_y = 0.;
                    },
                    Direction::Right => {
                        if (img_width_px + delta_x) < limit_px { delta_x = limit_px - img_width_px; }
                        pin_window_clone.set_delta_img_width(delta_x / zoom_factor);
                        delta_x = 0.;
                        delta_y = 0.;
                    },
                    Direction::LeftUpper => {
                        if (img_width_px - delta_x) < limit_px { delta_x = img_width_px - limit_px; }
                        if (img_height_px - delta_y) < limit_px { delta_y = img_height_px - limit_px; }
                        img_x += delta_x / zoom_factor;
                        img_y += delta_y / zoom_factor;
                        img_width -= delta_x / zoom_factor;
                        img_height -= delta_y / zoom_factor;
                    },
                    Direction::LeftLower => {
                        if (img_width_px - delta_x) < limit_px { delta_x = img_width_px - limit_px; }
                        if (img_height_px + delta_y) < limit_px { delta_y = limit_px - img_height_px; }
                        img_x += delta_x / zoom_factor;
                        img_width -= delta_x / zoom_factor;
                        pin_window_clone.set_delta_img_height(delta_y / zoom_factor);
                        delta_y = 0.;
                    },
                    Direction::RightUpper => {
                        if (img_width_px + delta_x) < limit_px { delta_x = limit_px - img_width_px; }
                        if (img_height_px - delta_y) < limit_px { delta_y = img_height_px - limit_px; }
                        img_y += delta_y / zoom_factor;
                        pin_window_clone.set_delta_img_width(delta_x / zoom_factor);
                        img_height -= delta_y / zoom_factor;
                        delta_x = 0.;
                    },
                    Direction::RightLower => {
                        if (img_width_px + delta_x) < limit_px { delta_x = limit_px - img_width_px; }
                        if (img_height_px + delta_y) < limit_px { delta_y = limit_px - img_height_px; }
                        pin_window_clone.set_delta_img_width(delta_x / zoom_factor);
                        pin_window_clone.set_delta_img_height(delta_y / zoom_factor);
                        delta_x = 0.;
                        delta_y = 0.;
                    },
                }
                
                let change_pos_x = now_pos.x + delta_x;
                let change_pos_y = now_pos.y + delta_y;
                pin_window_clone.set_img_x(img_x);
                pin_window_clone.set_img_y(img_y);
                pin_window_clone.set_img_width(img_width);
                pin_window_clone.set_img_height(img_height);
                pin_window_clone.window().set_position(slint::LogicalPosition::new(change_pos_x, change_pos_y));
                message_sender_clone.send(ShotterMessage::Move(id)).unwrap();
            });
        }

        { // code for focuse change
            let pin_window_clone = pin_window.as_weak();
            let message_sender_clone = message_sender.clone();
            pin_window.on_focus_trick(
                move |has_focus| {
                    let pin_window = pin_window_clone.unwrap();
                    if has_focus {
                        let position = pin_window.window().position();
                        let scale_factor = pin_window.window().scale_factor();
                        let width = pin_window.get_win_width();
                        let height = pin_window.get_win_height();
                        let left_bottom_x = position.x + (width * scale_factor) as i32;
                        let left_bottom_y = position.y + (height * scale_factor) as i32;
                        message_sender_clone.send(ShotterMessage::ShowToolbar(left_bottom_x, left_bottom_y, id, pin_window.as_weak())).unwrap();
                    } else if !pin_window.window().is_visible() || pin_window.window().is_minimized() {
                        message_sender_clone.send(ShotterMessage::HideToolbar(true)).unwrap();
                    } else {
                        message_sender_clone.send(ShotterMessage::HideToolbar(false)).unwrap();
                    }
                    true
                }
            );
        }

        { // code for function
            { // for close and hide
                let pin_window_clone = pin_window.as_weak();
                let message_sender_clone = message_sender.clone();
                pin_window.on_close(move || {
                    message_sender_clone.send(ShotterMessage::HideToolbar(true)).unwrap();
                    pin_window_clone.unwrap().hide().unwrap();
                    message_sender_clone.send(ShotterMessage::Close(id)).unwrap();
                });
    
                let pin_window_clone = pin_window.as_weak();
                let message_sender_clone = message_sender.clone();
                pin_window.on_hide(move || {
                    message_sender_clone.send(ShotterMessage::HideToolbar(true)).unwrap();
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
                pin_window.on_save(move || {
                    let pin_window = pin_window_clone.unwrap();
                    let scale_factor = pin_window.get_scale_factor();
                    let img_x = pin_window.get_img_x() * scale_factor;
                    let img_y = pin_window.get_img_y() * scale_factor;
                    let img_height = pin_window.get_img_height() * scale_factor;
                    let img_width = pin_window.get_img_width() * scale_factor;
                    
                    let mut img = PinWin::shared_pixel_buffer_to_dynamic_image(&buffer);
                    img = img.crop(img_x as u32, img_y as u32, img_width as u32, img_height as u32);
                    
                    pin_window.invoke_close();
                    
                    std::thread::spawn(move || {
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
                        let dialog_result = wfd::save_dialog(params);
                        if let Ok(file_path_result) = dialog_result {
                            img.save(file_path_result.selected_file_path).unwrap();
                        }
                    });
                });

                //copy
                let pin_window_clone = pin_window.as_weak();
                let img_rc_clone = img_rc.clone();
                let buffer = (*img_rc_clone.lock().unwrap()).clone();
                pin_window.on_copy(move || {
                    let pin_window = pin_window_clone.unwrap();
                    let scale_factor = pin_window.get_scale_factor();
                    let img_x = pin_window.get_img_x() * scale_factor;
                    let img_y = pin_window.get_img_y() * scale_factor;
                    let img_height = pin_window.get_img_height() * scale_factor;
                    let img_width = pin_window.get_img_width() * scale_factor;

                    let mut img = PinWin::shared_pixel_buffer_to_dynamic_image(&buffer);
                    img = img.crop(img_x as u32, img_y as u32, img_width as u32, img_height as u32);
                    
                    pin_window.invoke_close();

                    std::thread::spawn(move || {
                        let mut clipboard = Clipboard::new().unwrap();
                        let img_data = ImageData {
                            width: img.width() as usize,
                            height: img.height() as usize,
                            bytes: Cow::from(img.to_rgba8().to_vec())
                        };
                        clipboard.set_image(img_data).unwrap();
                    });
                });
            }

            { // code for draw
                let pin_window_clone = pin_window.as_weak();
                pin_window.on_trigger_draw(move || {
                    let pin_window = pin_window_clone.unwrap();
                    let state = pin_window.get_state();
                    if state == PinState::DrawFree {
                        pin_window.set_state(PinState::Normal);
                    } else {
                        pin_window.set_state(PinState::DrawFree);
                    }
                });

                let pin_window_clone = pin_window.as_weak();
                pin_window.on_draw_path(move |path| {
                    let pin_window = pin_window_clone.unwrap();
                    let pathes_rc = pin_window.get_pathes();
                    let pathes = pathes_rc.as_any().downcast_ref::<VecModel<SharedString>>()
                        .expect("We know we set a VecModel earlier");
                    pathes.push(path.into());
                });
            }
        }

        { // code for key press
            let pin_window_clone = pin_window.as_weak();
            pin_window.on_key_release(move |shortcut| {
                //TODO: handle F1-F12
                let mut text = shortcut.text.to_string();
                if text == "\u{1b}" { text = "Esc".into(); } // escape
                else if text == " " { text = "Space".into(); } // space
                else if text == "\n" { text = "Enter".into(); } // enter
                else if text.as_str() > "\u{1f}" && text.as_str() < "\u{7f}" { text = text.to_uppercase(); } // char
                else { return; } // exclude other control string
                
                let mut shortcut_str = String::new();
                if shortcut.modifiers.control { shortcut_str += "Ctrl+"; }
                if shortcut.modifiers.shift { shortcut_str += "Shift+"; }
                if shortcut.modifiers.meta { shortcut_str += "Win+"; }
                if shortcut.modifiers.alt { shortcut_str += "Alt+"; }
                else { shortcut_str += &text; }

                let pin_window = pin_window_clone.unwrap();
                let app_config = AppConfig::global().lock().unwrap();
                let default = "unkown".to_string();
                let shortcut_pinwin_save = app_config.get_shortcut("pinwin_save").unwrap_or(&default);
                let shortcut_pinwin_close = app_config.get_shortcut("pinwin_close").unwrap_or(&default);
                let shortcut_pinwin_copy = app_config.get_shortcut("pinwin_copy").unwrap_or(&default);
                let shortcut_pinwin_hide = app_config.get_shortcut("pinwin_hide").unwrap_or(&default);

                if shortcut_str.eq(shortcut_pinwin_save){
                    pin_window.invoke_save();
                } else if shortcut_str == *shortcut_pinwin_close {
                    pin_window.invoke_close();
                } else if shortcut_str == *shortcut_pinwin_copy {
                    pin_window.invoke_copy();
                } else if shortcut_str == *shortcut_pinwin_hide {
                    pin_window.invoke_hide();
                }
            });
        }

        pin_window.show().unwrap();
        
        // trick: fix the bug of error scale_factor
        if scale_factor != true_scale_factor {
            pin_window.set_zoom_factor(((scale_factor / true_scale_factor) * 100.) as i32);
        }

        PinWin {
            _img_rc: img_rc,
            _id: id,
            pin_window,
        }
    }

    fn shared_pixel_buffer_to_dynamic_image(buffer: &SharedPixelBuffer<Rgba8Pixel>) -> image::DynamicImage {
        image::DynamicImage::ImageRgba8(
            image::RgbaImage::from_vec(
                buffer.width(), buffer.height(), buffer.as_bytes().to_vec()
            ).unwrap()
        )
    }
}
