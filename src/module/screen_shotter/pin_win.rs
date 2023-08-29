use slint::Image;
use chrono;

use super::Rect;

enum STICK_TYPE {
    UPPER_UPPER,
    UPPER_LOWER,
    LOWER_UPPER,
    LOWER_LOWER,
    LEFT_RIGHT,
    LEFT_LEFT,
    RIGHT_RIGHT,
    RIGHT_LEFT
} // 方位枚举

pub struct PinWin {
    pub pin_window: PinWindow,
}

impl PinWin {
    pub fn new(img: Image, rect: Rect) -> PinWin {
        let pin_window = PinWindow::new().unwrap();
        let border_width = 2.;
        pin_window.window().set_position(slint::LogicalPosition::new(rect.x - border_width, rect.y - border_width));
        pin_window.set_win_border_width(border_width);
        pin_window.set_scale_factor(pin_window.window().scale_factor());

        pin_window.set_bac_image(img);
        pin_window.set_img_x(rect.x);
        pin_window.set_img_y(rect.y);
        pin_window.set_img_width(rect.width);
        pin_window.set_img_height(rect.height);
        
        let pin_window_clone = pin_window.as_weak();
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
            if is_stick_x == false || is_stick_y == false {
                pin_window_clone.window().set_position(
                    slint::LogicalPosition::new(now_pos.x + delta_x, now_pos.y + delta_y)
                );
            }
            // emit sgn_rect_change(this->geometry());
        });

        // 鼠标进行拖拉拽
        // QPointF globalPosition = e->globalPosition();
        // QRectF geo = geometry();
        // switch(m_direction) {
        //     case LEFT: geo.setLeft(e->globalPosition().x()); break;
        //     case RIGHT: geo.setRight(globalPosition.x()); break;
        //     case UPPER: geo.setTop(globalPosition.y()); break;
        //     case LOWER: geo.setBottom(globalPosition.y()); break;
        //     case LEFTUPPER: geo.setTopLeft(globalPosition.toPoint()); break;
        //     case RIGHTUPPER: geo.setTopRight(globalPosition.toPoint()); break;
        //     case LEFTLOWER: geo.setBottomLeft(globalPosition.toPoint()); break;
        //     case RIGHTLOWER: geo.setBottomRight(globalPosition.toPoint()); break;
        //     default: break;
        // }
        // QRectF tmpRect = zoomRect(geo, 1/m_zoom);
        // if(tmpRect.width() <= 0 || tmpRect.height() <= 0) close();
        // float x = m_windowRect.x() + (tmpRect.x() - m_geoRect.x()) * m_scaleRate / m_zoom;
        // float y = m_windowRect.y() + (tmpRect.y() - m_geoRect.y()) * m_scaleRate / m_zoom;
        // float width = tmpRect.width() * m_scaleRate;
        // float height = tmpRect.height() * m_scaleRate;
        // m_geoRect = zoomRect(geo, 1/m_zoom);
        // m_windowRect.setRect(x, y, width, height);
        // setGeometry(geo.toRect());
        // update();

        pin_window.show().unwrap();

        PinWin {
            pin_window,
        }
    }

    // TODO
    fn stick() {
        // void ShotterWindow::stick(STICK_TYPE stick_type, ShotterWindow * shotterWindow)
        // {
        //     QPoint delta;
        //     switch(stick_type){
        //         case STICK_TYPE::RIGHT_LEFT: delta = QPoint((shotterWindow->geometry().left() - geometry().right()), 0); break;
        //         case STICK_TYPE::RIGHT_RIGHT: delta = QPoint((shotterWindow->geometry().right() - geometry().right()), 0); break;
        //         case STICK_TYPE::LEFT_RIGHT: delta = QPoint((shotterWindow->geometry().right() - geometry().left()), 0); break;
        //         case STICK_TYPE::LEFT_LEFT: delta = QPoint((shotterWindow->geometry().left() - geometry().left()), 0); break;
        //         case STICK_TYPE::UPPER_LOWER: delta = QPoint(0, shotterWindow->geometry().bottom() - geometry().top()); break;
        //         case STICK_TYPE::UPPER_UPPER: delta = QPoint(0, shotterWindow->geometry().top() - geometry().top()); break;
        //         case STICK_TYPE::LOWER_UPPER: delta = QPoint(0, shotterWindow->geometry().top() - geometry().bottom()); break;
        //         case STICK_TYPE::LOWER_LOWER: delta = QPoint(0, shotterWindow->geometry().bottom() - geometry().bottom()); break;
        //         default:break;
        //     }
        //     if(stick_type == STICK_TYPE::RIGHT_LEFT || stick_type == STICK_TYPE::RIGHT_RIGHT || stick_type == STICK_TYPE::LEFT_RIGHT || stick_type == STICK_TYPE::LEFT_LEFT){
        //         m_isStickX = true;
        //     }else if(stick_type == STICK_TYPE::UPPER_LOWER || stick_type == STICK_TYPE::UPPER_UPPER || stick_type == STICK_TYPE::LOWER_UPPER || stick_type == STICK_TYPE::LOWER_LOWER){
        //         m_isStickY = true;
        //     }
        //     move(pos() + delta);
        //     m_geoRect.moveTo(m_geoRect.topLeft() + delta);
        // }
    }

    // TODO
    fn closeEvent() {
        // void ShotterWindow::closeEvent(QCloseEvent *event)
        // {
        //     emit sgn_close(this);
        // }
    }

    // TODO
    fn minimize() {
        // setWindowState(Qt::WindowMinimized);
    }

    // TODO
    fn onCompleteScreen() {
        // QClipboard *board = QApplication::clipboard();
        // board->setPixmap(m_originPainting.copy(m_windowRect.toRect())); // 把图片放入剪切板
        // quitScreenshot();
    }

    // TODO
    fn onSaveScreen() {
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
    }

    fn getFileName() -> String {
        "Rotor_".to_owned() + chrono::Local::now().format("Rotor_%Y-%m-%d-%H-%M-%S").to_string().as_str()
    }
}

slint::slint! {
    import { Button } from "std-widgets.slint";

    export component PinWindow inherits Window {
        no-frame: true;
        always-on-top: true;
        title: "小云视窗";

        in property <image> bac_image;
        in property <length> win_border_width;
        in property <float> scale_factor;

        in property <length> img_x;
        in property <length> img_y;
        in property <length> img_width;
        in property <length> img_height;
        

        in-out property <bool> is_stick_x;
        in-out property <bool> is_stick_y;

        callback win_move(length, length);

        width: img_width + win_border_width * 2;
        height: img_height + win_border_width * 2;

        // TODO:
        // if (e->type() == QEvent::ActivationChange) {
        //     if(QApplication::activeWindow() != this && QApplication::activeWindow() != m_toolbar) m_toolbar->hide();
        //     else m_toolbar->show();
        // }
        // if(e->type() == QEvent::KeyPress){
        //     QKeyEvent* keyEvent = (QKeyEvent*) e;
        //     qDebug()<<(int)keyEvent->modifiers();
        //     qDebug()<<(int)Qt::Key_Control;
        //     if (keyEvent->key() == Qt::Key_H) minimize(); // H键最小化
        //     else if (keyEvent->key() == Qt::Key_Enter || keyEvent->key() == Qt::Key_Return) onCompleteScreen();
        //     else if (keyEvent->key() == Qt::Key_Escape) quitScreenshot();
        //     else if ((keyEvent->modifiers() & Qt::ControlModifier) && keyEvent->key() == Qt::Key_S) onSaveScreen();
        //     else keyEvent->ignore();
        // }

        image_border := Rectangle {
            border-color: blue;
            border-width: win_border_width;

            pin_image := Image {
                source: bac_image;
                image-fit: contain;

                x: win_border_width;
                y: win_border_width;
                width: img_width;
                height: img_height;

                source-clip-x: img_x / 1px  * root.scale_factor;
                source-clip-y: img_y / 1px  * root.scale_factor;
                source-clip-width: img_width / 1px  * root.scale_factor;
                source-clip-height: img_height / 1px  * root.scale_factor;

                move_touch_area := TouchArea {
                    moved => {
                        root.win_move(self.mouse-x - self.pressed-x, self.mouse-y - self.pressed-y);
                    }
                    mouse-cursor: move;

                    resize_touch_area := TouchArea {
                        x: img_width - 6px;
                        y: img_height - 6px;
                        mouse-cursor: nwse-resize;
                    }
                }
            }
        }

        PopupWindow {
            close-on-click: false;
            height: 30px;
            width: 120px;

            HorizontalLayout {
                Button {
                    // 保存截图
                    height: 30px;
                    width: 30px;
                }

                Button { 
                    // 最小化截图
                    height: 30px;
                    width: 30px;
                }

                Button { 
                    // 关闭截图
                    height: 30px;
                    width: 30px;
                }

                Button { 
                    // 完成截图
                    height: 30px;
                    width: 30px;
                }
            }

            // void Toolbar::movePosition(QRect rect)
            //     this->move(rect.bottomRight().x() - 120, rect.bottomRight().y() + 4);

            // bool Toolbar::event(QEvent *e)
            //     if (e->type() == QEvent::ActivationChange)
            //         if(QApplication::activeWindow() != this && QApplication::activeWindow() != this->parent()) this->hide();
        }
    }
}