use std::sync::mpsc::Sender;
use std::thread;
// use muda::{Menu, MenuItem, ContextMenu, MenuEvent};
use slint::Image;
use i_slint_backend_winit::WinitWindowAccessor;
use raw_window_handle::HasRawWindowHandle;
use windows_sys::Win32::UI::WindowsAndMessaging;

use super::toolbar::Toolbar;
use super::Rect;

pub struct PinWin {
    _toolbar: Toolbar,
    pub pin_window: PinWindow,
}

impl PinWin {
    pub fn new(img: Image, rect: Rect, id: u32, move_sender: Sender<u32>) -> PinWin {
        let pin_window = PinWindow::new().unwrap();
        let border_width = pin_window.get_win_border_width();
        pin_window.window().set_position(slint::LogicalPosition::new(rect.x - border_width, rect.y - border_width));
        pin_window.set_scale_factor(pin_window.window().scale_factor());

        pin_window.set_bac_image(img);
        pin_window.set_img_x(rect.x);
        pin_window.set_img_y(rect.y);
        pin_window.set_img_width(rect.width);
        pin_window.set_img_height(rect.height);
        
        let toolbar = Toolbar::new();
        let toolbar_win_width = toolbar.toolbar_window.get_win_width();
        let pin_win_height = pin_window.get_win_height();
        let pin_win_width = pin_window.get_win_width();
        let now_pos = pin_window.window().position().to_logical(pin_window.window().scale_factor());
        toolbar.toolbar_window.window().set_position(
            slint::LogicalPosition::new(now_pos.x + pin_win_width - toolbar_win_width, now_pos.y + pin_win_height)
        );

        // let save_item = MenuItem::new("保存", true, None);
        // let minimize_item = MenuItem::new("最小化", true, None);
        // let exit_item = MenuItem::new("退出", true, None);
        // let finish_item = MenuItem::new("完成", true, None);
        // let menu = Menu::with_items(&[&save_item, &minimize_item, &exit_item, &finish_item]).unwrap();

        { // code for window move
            let pin_window_clone = pin_window.as_weak();
            let toolbar_window_clone = toolbar.toolbar_window.as_weak();
            let move_sender_clone = move_sender.clone();
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
                    move_sender_clone.send(id).unwrap();
                    
                    let toolbar_win_clone = toolbar_window_clone.unwrap();
                    let toolbar_win_width = toolbar_win_clone.get_win_width();
                    let pin_win_height = pin_window_clone.get_win_height();
                    let pin_win_width = pin_window_clone.get_win_width();
                    toolbar_win_clone.window().set_position(
                        slint::LogicalPosition::new(change_pos_x + pin_win_width - toolbar_win_width, change_pos_y + pin_win_height)
                    );
                }
            });
        }

        // { // code for right_menu
        //     let pin_window_clone = pin_window.as_weak();
        //     let pin_window = pin_window_clone.unwrap();
        //     let toolbar_window_clone = toolbar.toolbar_window.as_weak();
        //     pin_window.on_show_menu(move |mouse_x, mouse_y| {
        //         let pin_window = pin_window_clone.unwrap();
        //         pin_window.window().with_winit_window(|winit_win: &i_slint_backend_winit::winit::window::Window| {
        //             let raw_window_handle = winit_win.raw_window_handle();
        //             if let raw_window_handle::RawWindowHandle::Win32(win32_window_handle) = raw_window_handle {
        //                 let hwnd = win32_window_handle.hwnd as isize;
        //                 let position = muda::LogicalPosition { x: mouse_x, y: mouse_y };
        //                 menu.show_context_menu_for_hwnd(hwnd as isize, Some(position.into()));
        //             }
        //         });
        //     });

        //     let pin_window_clone = pin_window.as_weak();
        //     let save_item_id = save_item.id().clone();
        //     let minimize_item_id = minimize_item.id().clone();
        //     let exit_item_id = exit_item.id().clone();
        //     let finish_item_id = finish_item.id().clone();
        //     thread::spawn(move || {
        //         loop {
        //             let pin_window_clone = pin_window_clone.clone();
        //             if let Ok(event) = MenuEvent::receiver().recv() {
        //                 if event.id == save_item_id {
        //                     println!("save_item");
        //                 } else if event.id == minimize_item_id {
        //                     println!("minimize_item");
        //                 } else if event.id == exit_item_id {
        //                     println!("exit_item");
        //                     slint::invoke_from_event_loop(
        //                         move || {
        //                             let pin_window = pin_window_clone.unwrap();
        //                             pin_window.window().hide().unwrap();
        //                         }
        //                     ).unwrap();
        //                 } else if event.id == finish_item_id {
        //                     println!("finish_item");
        //                 }
        //             }
        //         }
        //     });
        // }

        pin_window.show().unwrap();
        toolbar.show();
        PinWin {
            _toolbar: toolbar,
            pin_window,
        }
    }

    // // TODO
    // fn on_complete_screen() {
    //     // QClipboard *board = QApplication::clipboard();
    //     // board->setPixmap(m_originPainting.copy(m_windowRect.toRect())); // 把图片放入剪切板
    //     // quitScreenshot();
    // }

    // // TODO
    // fn on_save_screen() {
    //     // SettingModel& settingModel = SettingModel::getInstance();
    //     // QVariant savePath = settingModel.getConfig(settingModel.Flag_Save_Path);
    //     let file_name = "Rotor_".to_owned() + chrono::Local::now().format("Rotor_%Y-%m-%d-%H-%M-%S").to_string().as_str();
    //     // QString fileName = QFileDialog::getSaveFileName(this, QStringLiteral("保存图片"), savePath.toString() + getFileName(), "PNG Files (*.PNG)");
    //     // if (fileName.length() > 0) {
    //     //     QPixmap pic = m_originPainting.copy(m_windowRect.toRect());
    //     //     pic.save(fileName, "png");
        
    //     //     QStringList listTmp = fileName.split("/");
    //     //     listTmp.pop_back();
    //     //     QString savePath = listTmp.join('/') + '/';

    //     //     settingModel.setConfig(settingModel.Flag_Save_Path, QVariant(savePath));
    //     // }
    // }
}

slint::slint! {
    import { Button } from "std-widgets.slint";

    export component PinWindow inherits Window {
        no-frame: true;
        always-on-top: true;
        title: "小云视窗";

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
        callback show_menu(length, length);

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
                    property <bool> right_pressed: false;

                    moved => {
                        if (self.right_pressed) {return;} // right button alse trigger this event
                        root.win_move(self.mouse-x - self.pressed-x, self.mouse-y - self.pressed-y);
                    }

                    pointer-event(event) => {
                        if(event.button == PointerEventButton.right) {
                            self.right_pressed = true;
                            if (event.kind == PointerEventKind.up) {
                                show-menu(self.mouse-x, self.mouse-y);
                            }
                        } else if(event.button == PointerEventButton.left) {
                            if (event.kind == PointerEventKind.down) {
                                self.right_pressed = false;
                            }
                        }
                    }

                    scroll-event(event) => {
                        debug(root.zoom_factor);
                        if (event.delta-y > 0) {
                            if (root.zoom_factor < 50) {
                                root.zoom_factor = root.zoom_factor + 1;
                            }
                        } else if (event.delta-y < 0) {
                            if (root.zoom_factor > 2) {
                                root.zoom_factor = root.zoom_factor - 1;
                            }
                        }
                        return accept;
                    }
                }
            }
        }
    }
}