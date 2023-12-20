use std::borrow::Cow;
use image;
use image::{ImageBuffer, Rgba};
use std::sync::{Arc, Mutex};
use std::sync::mpsc::Sender;
use arboard::{Clipboard, ImageData};
use slint::{SharedPixelBuffer, Rgba8Pixel};
use i_slint_backend_winit::WinitWindowAccessor;
use chrono;

use super::Rect;
use super::ShotterMessage;

pub struct PinWin {
    img_rc: Arc<Mutex<SharedPixelBuffer<Rgba8Pixel>>>,
    id: u32,
    pub pin_window: PinWindow,
}

impl PinWin {
    pub fn new(img_rc: Arc<Mutex<SharedPixelBuffer<Rgba8Pixel>>>, rect: Rect, id: u32, message_sender: Sender<ShotterMessage>) -> PinWin {
        let pin_window = PinWindow::new().unwrap();
        let border_width = pin_window.get_win_border_width();
        pin_window.window().set_position(slint::LogicalPosition::new(rect.x - border_width, rect.y - border_width));
        pin_window.set_scale_factor(pin_window.window().scale_factor());

        pin_window.set_bac_image(slint::Image::from_rgba8((*img_rc.lock().unwrap()).clone()));
        pin_window.set_img_x(rect.x);
        pin_window.set_img_y(rect.y);
        pin_window.set_img_width(rect.width);
        pin_window.set_img_height(rect.height);

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

        { // code for key press
            let img_rc_clone = img_rc.clone();
            let pin_window_clone = pin_window.as_weak();
            let message_sender_clone = message_sender.clone();
            pin_window.on_key_release(move |event| {
                let pin_window = pin_window_clone.unwrap();
                if event.text == slint::SharedString::from(slint::platform::Key::Escape) { // close win
                    pin_window.hide().unwrap();
                    message_sender_clone.send(ShotterMessage::Close(id)).unwrap();
                } else if event.text == "h" { // hide win
                    pin_window.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
                        winit_win.set_minimized(true);
                    });
                } else if event.text == "s" { // save pic
                    println!("TODO: save");
                    let buffer = (*img_rc_clone.lock().unwrap()).clone();

                    let file_name = "Rotor_".to_owned() + chrono::Local::now().format("Rotor_%Y-%m-%d-%H-%M-%S").to_string().as_str();
                    
                    // SettingModel& settingModel = SettingModel::getInstance();
                    // QVariant savePath = settingModel.getConfig(settingModel.Flag_Save_Path);
                    // QString fileName = QFileDialog::getSaveFileName(this, QStringLiteral("保存图片"), savePath.toString() + getFileName(), "PNG Files (*.PNG)");
                    // if (fileName.length() > 0) {
                    //     QPixmap pic = m_originPainting.copy(m_windowRect.toRect());
                    //     pic.save(fileName, "png");
                    //     QStringList listTmp = fileName.split("/");
                    //     listTmp.pop_back();
                    //     QString savePath = listTmp.join('/') + '/';
                    //     settingModel.setConfig(settingModel.Flag_Save_Path, QVariant(savePath));
                    // }

                    image::save_buffer(
                        std::path::Path::new(&(file_name + ".png")),
                        buffer.as_bytes(),
                        buffer.width(),
                        buffer.height(),
                        image::ColorType::Rgba8,
                    ).unwrap();
                } else if event.text == slint::SharedString::from(slint::platform::Key::Return) { // copy pic and close
                    let buffer = (*img_rc_clone.lock().unwrap()).clone();
                    
                    let scale_factor = pin_window.get_scale_factor();
                    let img_x = pin_window.get_img_x() * scale_factor;
                    let img_y = pin_window.get_img_y() * scale_factor;
                    let img_height = pin_window.get_img_height() * scale_factor;
                    let img_width = pin_window.get_img_width() * scale_factor;

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
                    
                    pin_window_clone.unwrap().hide().unwrap();
                    message_sender_clone.send(ShotterMessage::Close(id)).unwrap();
                }
            });
        }

        pin_window.show().unwrap();
        PinWin {
            img_rc,
            id,
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
        in property <length> win_border_width: 2px;
        in property <float> scale_factor;

        in property <length> img_x;
        in property <length> img_y;
        in-out property <int> zoom_factor: 10; // neet to be divided by ten
        in-out property <length> img_width;
        in-out property <length> img_height;

        in-out property <length> win_width: (img_width * zoom_factor / 10) + win_border_width * 2;
        in-out property <length> win_height: (img_height * zoom_factor / 10) + win_border_width * 2;

        in-out property <bool> is_stick_x;
        in-out property <bool> is_stick_y;

        callback win_move(length, length);
        callback key_release(KeyEvent);

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
                width: win_width - win_border_width * 2;
                height: win_height - win_border_width * 2;

                source-clip-x: img_x / 1px  * root.scale_factor;
                source-clip-y: img_y / 1px  * root.scale_factor;
                source-clip-width: img_width / 1px  * root.scale_factor;
                source-clip-height: img_height / 1px  * root.scale_factor;

                move_touch_area := TouchArea {
                    mouse-cursor: move;
                    moved => {
                        root.win_move(self.mouse-x - self.pressed-x, self.mouse-y - self.pressed-y);
                    }

                    scroll-event(event) => {
                        if (event.delta-y > 0) {
                            if (root.zoom_factor < 50) { root.zoom_factor = root.zoom_factor + 1; }
                        } else if (event.delta-y < 0) {
                            if (root.zoom_factor > 2) { root.zoom_factor = root.zoom_factor - 1; }
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