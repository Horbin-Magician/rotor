use std::sync::mpsc::Sender;

use slint::Image;
use chrono;

use super::Rect;

pub struct PinWin {
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
        
        let pin_window_clone = pin_window.as_weak();
        let move_sender_clone = move_sender.clone();
        pin_window.on_win_move(move |mut delta_x, mut delta_y| {
            let pin_window_clone = pin_window_clone.unwrap();
            let now_pos = pin_window_clone.window().position().to_logical(pin_window_clone.window().scale_factor());
            let is_stick_x = pin_window_clone.get_is_stick_x();
            let is_stick_y = pin_window_clone.get_is_stick_y();

            if is_stick_x {
                println!("delta_x: {}", delta_x);
                if delta_x.abs() > 20. {
                    pin_window_clone.set_is_stick_x(false);
                } else {
                    delta_x = 0.;
                }
            }
            if is_stick_y {
                println!("delta_y: {}", delta_y);
                if delta_y.abs() > 20. {
                    pin_window_clone.set_is_stick_y(false);
                } else {
                    delta_y = 0.;
                }
            }
            if !is_stick_x || !is_stick_y {
                pin_window_clone.window().set_position(
                    slint::LogicalPosition::new(now_pos.x + delta_x, now_pos.y + delta_y)
                );
                move_sender_clone.send(id).unwrap();
            }
        });

        pin_window.show().unwrap();

        PinWin {
            pin_window,
        }
    }

    // TODO
    fn close_event() {
        // emit sgn_close(this);
    }

    // TODO
    fn minimize() {
        // setWindowState(Qt::WindowMinimized);
    }

    // TODO
    fn on_complete_screen() {
        // QClipboard *board = QApplication::clipboard();
        // board->setPixmap(m_originPainting.copy(m_windowRect.toRect())); // 把图片放入剪切板
        // quitScreenshot();
    }

    // TODO
    fn on_save_screen() {
        // SettingModel& settingModel = SettingModel::getInstance();
        // QVariant savePath = settingModel.getConfig(settingModel.Flag_Save_Path);
        let file_name = "Rotor_".to_owned() + chrono::Local::now().format("Rotor_%Y-%m-%d-%H-%M-%S").to_string().as_str();
        // QString fileName = QFileDialog::getSaveFileName(this, QStringLiteral("保存图片"), savePath.toString() + getFileName(), "PNG Files (*.PNG)");
        // if (fileName.length() > 0) {
        //     QPixmap pic = m_originPainting.copy(m_windowRect.toRect());
        //     pic.save(fileName, "png");
        
        //     QStringList listTmp = fileName.split("/");
        //     listTmp.pop_back();
        //     QString savePath = listTmp.join('/') + '/';

        //     settingModel.setConfig(settingModel.Flag_Save_Path, QVariant(savePath));
        // }
    }
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
        in-out property <length> img_width;
        in-out property <length> img_height;

        in-out property <length> win_width: img_width + win_border_width * 2;
        in-out property <length> win_height: img_height + win_border_width * 2;

        in-out property <bool> is_stick_x;
        in-out property <bool> is_stick_y;

        callback win_move(length, length);

        width <=> win_width;
        height <=> win_height;

        // TODO:
        // if (e->type() == QEvent::ActivationChange) {
        //     if(QApplication::activeWindow() != this && QApplication::activeWindow() != m_toolbar) m_toolbar->hide();
        //     else m_toolbar->show();
        // }
        // TODO:
        // if (keyEvent->key() == Qt::Key_H) minimize(); // H键最小化
        // else if (keyEvent->key() == Qt::Key_Enter || keyEvent->key() == Qt::Key_Return) onCompleteScreen();
        // else if (keyEvent->key() == Qt::Key_Escape) quitScreenshot();
        // else if ((keyEvent->modifiers() & Qt::ControlModifier) && keyEvent->key() == Qt::Key_S) onSaveScreen();

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
                    moved => { root.win_move(self.mouse-x - self.pressed-x, self.mouse-y - self.pressed-y);}

                    resize_touch_area := TouchArea {
                        x: win_width - 10px;
                        y: win_height - 10px;
                        mouse-cursor: nwse-resize;
                        // An alternative way to resize when there is no wheel event
                        moved => {
                            win_width = win_width + self.mouse-x - self.pressed-x;
                            win_height = win_height + self.mouse-y - self.pressed-y;
                            if (win_width < 10px) {win_width = 10px};
                            if (win_height < 10px) {win_height = 10px}
                        }
                    }
                }
            }
        }

        // tool_bar := PopupWindow {
        //     close-on-click: false;
        //     height: 30px;
        //     width: 120px;
        //     x: root.win_width - 120px;
        //     y: root.win_height + 4px;
        //     z: 1;
            
        //     Rectangle { height:100%; width: 100%; background: yellow; }
        //     // HorizontalLayout {
        //     //     Button {
        //     //         // 保存截图
        //     //         height: 30px;
        //     //         width: 30px;
        //     //     }

        //     //     Button { 
        //     //         // 最小化截图
        //     //         height: 30px;
        //     //         width: 30px;
        //     //     }

        //     //     Button { 
        //     //         // 退出截图
        //     //         height: 30px;
        //     //         width: 30px;
        //     //     }

        //     //     Button { 
        //     //         // 完成截图
        //     //         height: 30px;
        //     //         width: 30px;
        //     //     }
        //     // }
        //     // void Toolbar::movePosition(QRect rect)
        //     //     this->move(rect.bottomRight().x() - 120, rect.bottomRight().y() + 4);

        //     // bool Toolbar::event(QEvent *e)
        //     //     if (e->type() == QEvent::ActivationChange)
        //     //         if(QApplication::activeWindow() != this && QApplication::activeWindow() != this->parent()) this->hide();
        // }
    }
}