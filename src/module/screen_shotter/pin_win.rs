use std::borrow::Cow;
use std::error::Error;
use wfd::DialogParams;
use std::sync::mpsc::Sender;
use arboard::{Clipboard, ImageData};
use slint::{Model, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel, ComponentHandle};
use i_slint_backend_winit::WinitWindowAccessor;
use chrono;

use crate::util::{log_util, sys_util, img_util};
use crate::core::application::app_config::AppConfig;
use crate::ui::{PinWindow, Rect, Direction, PinState};
use super::ShotterMessage;

pub struct PinWin {
    _id: u32,
    pub pin_window: PinWindow,
}

impl PinWin {
    pub fn new(
        img: SharedPixelBuffer<Rgba8Pixel>,
        rect: Rect,
        offset_x: i32,
        offset_y: i32,
        true_scale_factor: f32,
        id: u32,
        message_sender: Sender<ShotterMessage>,
    ) -> Result<PinWin, Box<dyn Error>> {
        let pin_window = PinWindow::new()?;
        sys_util::forbid_window_animation(pin_window.window());

        { // set bash properties
            let border_width = pin_window.get_win_border_width();
            let scale_factor = pin_window.window().scale_factor();

            pin_window.set_scale_factor(scale_factor);
            
            pin_window.window().set_position(slint::LogicalPosition::new((rect.x + offset_x as f32) / scale_factor - border_width, (rect.y + offset_y as f32) / scale_factor - border_width));
            
            pin_window.set_bac_image(slint::Image::from_rgba8(img));
            pin_window.set_img_x(rect.x / scale_factor);
            pin_window.set_img_y(rect.y / scale_factor);
            pin_window.set_img_width(rect.width / scale_factor);
            pin_window.set_img_height(rect.height / scale_factor);
            
            let zoom_delta = AppConfig::global()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get_zoom_delta();
            pin_window.set_zoom_delta(zoom_delta.into());
        }

        { // code for window move
            let pin_window_clone = pin_window.as_weak();
            let message_sender_clone = message_sender.clone();
            pin_window.on_win_move(move |mut delta_x, mut delta_y, mouse_direction| {
                if let Some(pin_window_clone) = pin_window_clone.upgrade() {
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
                    let _ = message_sender_clone.send(ShotterMessage::Move(id));
                }
            });
        }

        { // code for focuse change
            let pin_window_clone = pin_window.as_weak();
            let message_sender_clone = message_sender.clone();
            pin_window.on_focus_trick(
                move |has_focus| {
                    if let Some(pin_window) = pin_window_clone.upgrade() {
                        if has_focus {
                            let position = pin_window.window().position();
                            let scale_factor = pin_window.window().scale_factor();
                            let width = pin_window.get_win_width();
                            let height = pin_window.get_win_height();
                            let left_bottom_x = position.x + (width * scale_factor) as i32;
                            let left_bottom_y = position.y + (height * scale_factor) as i32;
                            let _ = message_sender_clone.send(ShotterMessage::ShowToolbar(left_bottom_x, left_bottom_y, id, pin_window.as_weak()));
                        } else if !pin_window.window().is_visible() || pin_window.window().is_minimized() {
                            let _ = message_sender_clone.send(ShotterMessage::HideToolbar(true));
                        } else {
                            let _ = message_sender_clone.send(ShotterMessage::HideToolbar(false));
                        }
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
                    let _ = message_sender_clone.send(ShotterMessage::HideToolbar(true));
                    if let Some(pin_window) = pin_window_clone.upgrade() {
                        let _ = pin_window.hide();
                    }
                    let _ = message_sender_clone.send(ShotterMessage::Close(id));
                });

                let message_sender_clone = message_sender.clone();
                pin_window.window().on_close_requested(move || {
                    let _ = message_sender_clone.send(ShotterMessage::HideToolbar(true));
                    let _ = message_sender_clone.send(ShotterMessage::Close(id));
                    slint::CloseRequestResponse::HideWindow
                });
    
                let pin_window_clone = pin_window.as_weak();
                let message_sender_clone = message_sender.clone();
                pin_window.on_hide(move || {
                    let _ = message_sender_clone.send(ShotterMessage::HideToolbar(true));
                    if let Some(pin_window) = pin_window_clone.upgrade() {
                        pin_window.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
                            winit_win.set_minimized(true);
                        });
                    }
                });
            }

            { // for save and copy
                // save
                let pin_window_clone = pin_window.as_weak();
                pin_window.on_save(move || {
                    if let Some(pin_window) = pin_window_clone.upgrade() {
                        let scale_factor = pin_window.get_scale_factor();
                        let border_width = (pin_window.get_win_border_width() * scale_factor).ceil() as u32;

                        match pin_window.window().take_snapshot() {
                            Ok(buffer) => {
                                let mut img = img_util::shared_pixel_buffer_to_dynamic_image(&buffer);
                                img = img.crop(border_width, border_width, img.width() - 2 * border_width, img.height() - 2 * border_width);
        
                                std::thread::spawn(move || {
                                    let save_path = AppConfig::global()
                                        .lock()
                                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                                        .get_save_path();
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
                                        img.save(file_path_result.selected_file_path)
                                            .unwrap_or_else(|e| log_util::log_error(format!("Failed to save image: {}", e)));
                                    }
                                });
                            },
                            Err(e) => log_util::log_error(format!("Failed to take snapshot: {}", e)),
                        }
                        
                        pin_window.invoke_close();
                    }
                });

                //copy
                let pin_window_clone = pin_window.as_weak();
                pin_window.on_copy(move || {
                    if let Some(pin_window) = pin_window_clone.upgrade() {
                        let scale_factor = pin_window.get_scale_factor();
                        let border_width = (pin_window.get_win_border_width() * scale_factor).ceil() as u32;

                        match pin_window.window().take_snapshot() {
                            Ok(buffer) => {
                                let mut img = img_util::shared_pixel_buffer_to_dynamic_image(&buffer);
                                img = img.crop(border_width, border_width, img.width() - 2 * border_width, img.height() - 2 * border_width);
            
                                std::thread::spawn(move || {
                                    let img_data = ImageData {
                                        width: img.width() as usize,
                                        height: img.height() as usize,
                                        bytes: Cow::from(img.to_rgba8().to_vec())
                                    };
                                    if let Ok(mut clipboard) = Clipboard::new() {
                                        clipboard.set_image(img_data)
                                            .unwrap_or_else(|e| log_util::log_error(format!("Failed to copy image to clipboard: {}", e)));
                                    }
                                });
                            },
                            Err(e) => log_util::log_error(format!("Failed to take snapshot: {}", e)),
                        }

                        pin_window.invoke_close();
                    }
                });
            }

            { // code for draw
                let pin_window_clone = pin_window.as_weak();
                pin_window.on_trigger_draw(move || {
                    if let Some(pin_window) = pin_window_clone.upgrade() {
                        let state = pin_window.get_state();
                        if state == PinState::DrawFree {
                            pin_window.set_state(PinState::Normal);
                        } else {
                            pin_window.set_state(PinState::DrawFree);
                        }
                    }
                });

                let pin_window_clone = pin_window.as_weak();
                pin_window.on_draw_path(move |path| {
                    if let Some(pin_window) = pin_window_clone.upgrade() {
                        let pathes_rc = pin_window.get_pathes();
                        if let Some(pathes) = pathes_rc.as_any().downcast_ref::<VecModel<SharedString>>() {
                            pathes.push(path);
                        }
                    }
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
                
                if let Some(pin_window) = pin_window_clone.upgrade() {
                    if let Ok(app_config) = AppConfig::global().try_lock() {
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
                    }
                }
            });
        }

        { // code for update record
            let message_sender_clone = message_sender.clone();
            pin_window.on_update_record(move || {
                let _ = message_sender_clone.send(ShotterMessage::UpdateRecord(id));
            });
        }

        let _ = pin_window.show();
        
        { // trick: fix the bug of error scale_factor
            let scale_factor = pin_window.window().scale_factor();
            if scale_factor != true_scale_factor {
                pin_window.set_extra_zoom_factor(((scale_factor / true_scale_factor) * 100.) as i32);
            }
        }

        Ok(PinWin {
            _id: id,
            pin_window,
        })
    }
}
