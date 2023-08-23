
enum DIRECTION {
    UPPER,
    LOWER,
    LEFT,
    RIGHT,
    LEFTUPPER,
    LEFTLOWER,
    RIGHTLOWER,
    RIGHTUPPER,NONE
} // 方位枚举

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

struct PinWin {
    // const int M_PADDING = 6; // 内边距，决定拖拽的触发。
    // Toolbar* m_toolbar;
    // bool m_isStickX;
    // bool m_isStickY;
    // float m_scaleRate;
    // DIRECTION m_direction;
    // QRectF m_geoRect; // 对应缩放前显示的虚拟窗口
    // QRectF m_windowRect; // 对应图片区域的虚拟窗口
    // float m_zoom; // 窗口缩放的数值
    // bool m_isPressed; // 鼠标是否按下
    // QPoint m_movePos; // 拖动的距离
    // QPixmap m_originPainting; // 屏幕原画
}

impl PinWin {
    fn new() -> PinWin {
        // ShotterWindow::ShotterWindow(std::shared_ptr<QPixmap> originPainting, QRectF windowRect, QWidget *parent):QWidget(parent)
        // {
        //     setWindowFlags(Qt::FramelessWindowHint | Qt::Window | Qt::WindowStaysOnTopHint);
        //     setWindowTitle("小云视窗");

        //     m_originPainting = originPainting->copy();
        //     m_isStickX = false;
        //     m_isStickY = false;
        //     m_isPressed = false;
        //     m_windowRect = windowRect;
        //     m_scaleRate = QGuiApplication::primaryScreen()->devicePixelRatio();
        //     m_zoom = 1;
        //     m_direction = NONE;

        //     setGeometry(m_windowRect.x()/m_scaleRate, m_windowRect.y()/m_scaleRate, m_windowRect.width()/m_scaleRate, m_windowRect.height()/m_scaleRate);
        //     m_geoRect = geometry();
        //     setMouseTracking(true); // 开启鼠标实时追踪
        //     show();

        //     initToolbar();
        // }

        PinWin {
            
        }
    }

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

    fn getMouseRegion() {
        // DIRECTION ShotterWindow::getMouseRegion(const QPoint &cursor)
        // {
        //     DIRECTION ret_dir = NONE;

        //     QPoint pt_lu = rect().topLeft(); // left upper
        //     QPoint pt_rl = rect().bottomRight(); // right lower

        //     int x = cursor.x();
        //     int y = cursor.y();

        //     /// 获得鼠标当前所处窗口的边界方向
        //     if(pt_lu.x() + M_PADDING >= x
        //     && pt_lu.x() <= x
        //     && pt_lu.y() + M_PADDING >= y
        //     && pt_lu.y() <= y) {
        //         ret_dir = LEFTUPPER; // 左上角
        //         this->setCursor(QCursor(Qt::SizeFDiagCursor));
        //     } else if(x >= pt_rl.x() - M_PADDING
        //            && x <= pt_rl.x()
        //            && y >= pt_rl.y() - M_PADDING
        //            && y <= pt_rl.y()) {
        //         ret_dir = RIGHTLOWER; // 右下角
        //         this->setCursor(QCursor(Qt::SizeFDiagCursor));
        //     } else if(x <= pt_lu.x() + M_PADDING
        //            && x >= pt_lu.x()
        //            && y >= pt_rl.y() - M_PADDING
        //            && y <= pt_rl.y()) {
        //         ret_dir = LEFTLOWER; // 左下角
        //         this->setCursor(QCursor(Qt::SizeBDiagCursor));
        //     } else if(x <= pt_rl.x()
        //            && x >= pt_rl.x() - M_PADDING
        //            && y >= pt_lu.y()
        //            && y <= pt_lu.y() + M_PADDING) {
        //         ret_dir = RIGHTUPPER; // 右上角
        //         this->setCursor(QCursor(Qt::SizeBDiagCursor));
        //     } else if(x <= pt_lu.x() + M_PADDING
        //            && x >= pt_lu.x()) {
        //         ret_dir = LEFT; // 左边
        //         this->setCursor(QCursor(Qt::SizeHorCursor));
        //     } else if( x <= pt_rl.x()
        //             && x >= pt_rl.x() - M_PADDING) {
        //         ret_dir = RIGHT; // 右边
        //         this->setCursor(QCursor(Qt::SizeHorCursor));
        //     }else if(y >= pt_lu.y()
        //           && y <= pt_lu.y() + M_PADDING){
        //         ret_dir = UPPER; // 上边
        //         this->setCursor(QCursor(Qt::SizeVerCursor));
        //     } else if(y <= pt_rl.y()
        //            && y >= pt_rl.y() - M_PADDING) {
        //         ret_dir = LOWER; // 下边
        //         this->setCursor(QCursor(Qt::SizeVerCursor));
        //     }else {
        //         ret_dir = NONE; // 默认
        //         this->setCursor(QCursor(Qt::SizeAllCursor));
        //     }
        //     return ret_dir;
        // }
    }

    fn event() {
        // bool ShotterWindow::event(QEvent *e)
        // {
        //     if (e->type() == QEvent::ActivationChange) {
        //         if(QApplication::activeWindow() != this && QApplication::activeWindow() != m_toolbar) m_toolbar->hide();
        //         else m_toolbar->show();
        //     }
        //     if(e->type() == QEvent::KeyPress){
        //         QKeyEvent* keyEvent = (QKeyEvent*) e;
        //         qDebug()<<(int)keyEvent->modifiers();
        //         qDebug()<<(int)Qt::Key_Control;
        //         if (keyEvent->key() == Qt::Key_H) minimize(); // H键最小化
        //         else if (keyEvent->key() == Qt::Key_Enter || keyEvent->key() == Qt::Key_Return) onCompleteScreen();
        //         else if (keyEvent->key() == Qt::Key_Escape) quitScreenshot();
        //         else if ((keyEvent->modifiers() & Qt::ControlModifier) && keyEvent->key() == Qt::Key_S) onSaveScreen();
        //         else keyEvent->ignore();
        //     }
        //     return QWidget::event(e);
        // }
    }

    fn mousePressEvent() {
        // void ShotterWindow::mousePressEvent(QMouseEvent *e)
        // {
        //     if (e->button() == Qt::LeftButton) {
        //         m_isPressed = true;
        //         m_movePos = e->position().toPoint();
        //     }
        // }
    }

    fn mouseReleaseEvent() {
        // void ShotterWindow::mouseReleaseEvent(QMouseEvent *e)
        // {
        //     if (e->button() == Qt::LeftButton) m_isPressed = false;
        // }
    }

    fn mouseMoveEvent() {
        // void ShotterWindow::mouseMoveEvent(QMouseEvent *e)
        // {
        //     if(!m_isPressed) m_direction = getMouseRegion(e->pos());
        //     if(m_isPressed) {
        //         if(m_direction != NONE) {
        //             // 鼠标进行拖拉拽
        //             QPointF globalPosition = e->globalPosition();
        //             QRectF geo = geometry();
        //             switch(m_direction) {
        //                 case LEFT: geo.setLeft(e->globalPosition().x()); break;
        //                 case RIGHT: geo.setRight(globalPosition.x()); break;
        //                 case UPPER: geo.setTop(globalPosition.y()); break;
        //                 case LOWER: geo.setBottom(globalPosition.y()); break;
        //                 case LEFTUPPER: geo.setTopLeft(globalPosition.toPoint()); break;
        //                 case RIGHTUPPER: geo.setTopRight(globalPosition.toPoint()); break;
        //                 case LEFTLOWER: geo.setBottomLeft(globalPosition.toPoint()); break;
        //                 case RIGHTLOWER: geo.setBottomRight(globalPosition.toPoint()); break;
        //                 default: break;
        //             }
        //             QRectF tmpRect = zoomRect(geo, 1/m_zoom);
        //             if(tmpRect.width() <= 0 || tmpRect.height() <= 0) close();
        //             float x = m_windowRect.x() + (tmpRect.x() - m_geoRect.x()) * m_scaleRate / m_zoom;
        //             float y = m_windowRect.y() + (tmpRect.y() - m_geoRect.y()) * m_scaleRate / m_zoom;
        //             float width = tmpRect.width() * m_scaleRate;
        //             float height = tmpRect.height() * m_scaleRate;
        //             m_geoRect = zoomRect(geo, 1/m_zoom);
        //             m_windowRect.setRect(x, y, width, height);
        //             setGeometry(geo.toRect());
        //             update();
        //         }
        //         else {
        //             QPoint delta = e->position().toPoint() - m_movePos;
        //             if(m_isStickX){
        //                 if(abs(delta.x()) > 20) m_isStickX = false;
        //                 else delta.setX(0);
        //             }
        //             if(m_isStickY){
        //                 if(abs(delta.y()) > 20) m_isStickY = false;
        //                 else delta.setY(0);
        //             }
        //             if(m_isStickX == false || m_isStickY == false){
        //                 move(pos() + delta);
        //                 m_geoRect.moveTo(m_geoRect.topLeft() + delta);
        //                 emit sgn_move(this);
        //             }
        //         }
        //         emit sgn_rect_change(this->geometry());
        //     }
        // }
    }

    fn paintEvent() {
        // void ShotterWindow::paintEvent(QPaintEvent *)
        // {
        //     QPainter painter(this);
        //     QPen pen = painter.pen();
        //     pen.setColor(QColor(0,175,255));
        //     pen.setWidth(2);
        //     pen.setJoinStyle(Qt::MiterJoin);
        //     painter.setPen(pen);

        //     painter.setRenderHint(QPainter::SmoothPixmapTransform);

        //     QRect windowRect = rect();
        //     painter.drawPixmap(rect(), m_originPainting, m_windowRect); // 绘制截屏编辑窗口
        //     painter.drawRect(windowRect.x()+1, windowRect.y()+1, windowRect.width()-2, windowRect.height()-2); // 绘制边框线C
        // }
    }

    fn wheelEvent() {
        // void ShotterWindow::wheelEvent(QWheelEvent *e)
        // {
        //     if(e->angleDelta().y() > 0) m_zoom = m_zoom + 0.1;
        //     else if (e->angleDelta().y() < 0 && m_zoom > 0.1) m_zoom = m_zoom - 0.1;
        //     setGeometry(zoomRect(m_geoRect, m_zoom).toRect());
        //     emit sgn_rect_change(this->geometry());
        // }
    }

    fn changeEvent() {
        // void ShotterWindow::changeEvent(QEvent *event)
        // {
        //     if(QEvent::WindowStateChange == event->type()){
        //         QWindowStateChangeEvent * stateEvent = dynamic_cast<QWindowStateChangeEvent*>(event);
        //         if(Q_NULLPTR != stateEvent){
        //             if(Qt::WindowMinimized == stateEvent->oldState()){
        //                 // 为了去除最小化后再打开时出现的白框闪烁，目前认为是windows的BUG
        //                 setWindowFlags(Qt::Window | Qt::WindowStaysOnTopHint);
        //                 setWindowFlags(Qt::FramelessWindowHint | Qt::Window | Qt::WindowStaysOnTopHint);
        //                 show();
        //             }
        //         }
        //     }
        // }
    }

    fn closeEvent() {
        // void ShotterWindow::closeEvent(QCloseEvent *event)
        // {
        //     emit sgn_close(this);
        // }
    }

    fn getFileName() {
        // QDateTime currentTime = QDateTime::currentDateTime();
        // QString file_name = "Rotor_" + currentTime.toString(QStringLiteral("yyyy-MM-dd-HH-mm-ss"));
        // return file_name;
    }

    fn minimize() {
        // setWindowState(Qt::WindowMinimized);
    }

    fn initToolbar() {
        // m_toolbar = new Toolbar(this);

        // connect(this, &ShotterWindow::sgn_rect_change, m_toolbar, &Toolbar::movePosition);
        // emit sgn_rect_change(this->geometry());

        // connect(m_toolbar, &Toolbar::sgn_complete, this, &ShotterWindow::onCompleteScreen);
        // connect(m_toolbar, &Toolbar::sgn_save, this, &ShotterWindow::onSaveScreen);
        // connect(m_toolbar, &Toolbar::sgn_minimize, this, &ShotterWindow::minimize);
        // connect(m_toolbar, &Toolbar::sgn_close, this, &ShotterWindow::quitScreenshot);

        // m_toolbar->show();
    }

    fn zoomRect() {
        // QRectF ShotterWindow::zoomRect(const QRectF &rect, float zoom)
        // {
        //     return QRectF(rect.x(), rect.y(), rect.width()*zoom, rect.height()*zoom);
        // }
    }

    fn onCompleteScreen() {
        // QClipboard *board = QApplication::clipboard();
        // board->setPixmap(m_originPainting.copy(m_windowRect.toRect())); // 把图片放入剪切板
        // quitScreenshot();
    }

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

    fn quitScreenshot() {
        close();
    }
}

slint::slint! {
    export component PinWindow inherits PopupWindow {

    }
}