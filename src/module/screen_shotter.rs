use slint::{SharedPixelBuffer, Rgba8Pixel};
use i_slint_backend_winit::WinitWindowAccessor;
use screenshots::Screen;
use winit::dpi::PhysicalSize;

pub struct ScreenShotter {
    screens: Vec<Screen>,
    pub mask_win: MaskWindow,
    state: u8, // 0:未开始截图 1:正在截图，未按下左键 2:正在截图，已按下左键
    // shotter_win_list: Vec<ShotterWindow>,
    // amplifer_tool: Amplifier, // 放大取色器
    // start_point: slint::PhysicalPosition, // 用于检测误操作
    // window_rect: slint::PhysicalRect, // 当前鼠标选区最小的矩形窗口
}

impl ScreenShotter{
    pub fn new() -> ScreenShotter {
        // get screens and info
        let screens = Screen::all().unwrap();
        let primary_screen = Self::get_prime_screen(&screens).unwrap();
        // init MaskWindow
        let mask_win = MaskWindow::new().unwrap();

        // there is an animation when the window is first show. The mask window does not need the animation
        mask_win.show().unwrap();
        mask_win.hide().unwrap();

        mask_win.window().set_position( slint::PhysicalPosition::new(0, 0) );

        let primary_screen_clone = primary_screen.clone();
        let mask_win_clone = mask_win.as_weak();
        mask_win.on_shot(move || {
            let mask_win_clone = mask_win_clone.unwrap();
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
            mask_win_clone.window().with_winit_window(|winit_win: &winit::window::Window| {
                // +1 to fix the bug
                winit_win.set_inner_size(PhysicalSize::new(primary_screen_clone.display_info.width, primary_screen_clone.display_info.height + 1));
                winit_win.focus_window();
            });

            //     m_state = 1;
            //     CaptureScreen(m_originPainting, m_backgroundScreen); // 捕获屏幕
            //     initAmplifier(); // 初始化鼠标放大器
            //     emit cursorPosChange(cursor().pos().x(), cursor().pos().y()); // 更新鼠标的位置
            //     updateMouseWindow(); // 更新鼠标区域窗口
            //     show(); // 展示窗口
            //     this->activateWindow();
            //     this->setFocus();
        });

        mask_win.on_point_event(move |event| {
            println!("{:?}", event);
            if event.button == slint::platform::PointerEventButton::Left {
                match event.kind {
                    slint::private_unstable_api::re_exports::PointerEventKind::Down => {
                        println!("鼠标左键按下啦");
                    },
                    slint::private_unstable_api::re_exports::PointerEventKind::Up => {
                        println!("鼠标左键释放啦");
                    },
                    _ => {}
                }
            }
        });

        let mask_win_clone = mask_win.as_weak();
        mask_win.on_key_released(move |event| {
            println!("{:?}", event);
            if event.text == slint::SharedString::from(slint::platform::Key::Escape) {
                mask_win_clone.unwrap().hide().unwrap();
            }

            //     if(e->type() == QEvent::KeyPress){
            //         QKeyEvent* keyEvent = (QKeyEvent*) e;
            //         if (keyEvent->key() == Qt::Key_Escape) endShot(); // Esc键退出截图
            //         else if (keyEvent->key() == Qt::Key_H) SwitcHideShotterWin();// 隐藏其他窗口
            //         else if (keyEvent->key() == Qt::Key_Z) m_amplifierTool->switchColorType(); // Z键切换颜色
            //         else if (keyEvent->key() == Qt::Key_C){ // C键复制颜色
            //             QString colorStr = m_amplifierTool->getColorStr();
            //             QClipboard *clipboard = QGuiApplication::clipboard();
            //             clipboard->setText(colorStr);
            //         }
            //         else keyEvent->ignore();
            //     } else if(e->type() == QEvent::MouseMove){
            //         QMouseEvent* mouseEvent = (QMouseEvent*) e;
            //         emit cursorPosChange(mouseEvent->position().x(), mouseEvent->position().y());
            //         updateMouseWindow(); // 更新当前鼠标选中的窗口
            //         update();
            //     } else if(e->type() == QEvent::MouseButtonPress){ // 按下左键，开始绘制截图窗口
            //         QMouseEvent* mouseEvent = (QMouseEvent*) e;
            //         if (mouseEvent->button() == Qt::LeftButton) {
            //             m_startPoint = QPoint(mouseEvent->pos().x() * m_scaleRate, mouseEvent->pos().y() * m_scaleRate);
            //             m_state = 2;
            //         }
            //     } else if(e->type() == QEvent::MouseButtonRelease){ // 松开左键，新建截图窗口
            //         QMouseEvent* mouseEvent = (QMouseEvent*) e;
            //         if (m_state == 2 && mouseEvent->button() == Qt::LeftButton) {
            //             ShotterWindow* shotterWindow = new ShotterWindow(m_originPainting, m_windowRect);
            //             connect(shotterWindow, &ShotterWindow::sgn_close, this, &ScreenShotter::onShotterWindowClose);
            //             connect(shotterWindow, &ShotterWindow::sgn_move, this, &ScreenShotter::onShotterWindowMove);
            //             m_ShotterWindowList.append(shotterWindow);
            //             endShot(); // 结束截图
            //         }
            //     }
            //     return QWidget::event(e);
        });

        ScreenShotter{
            screens,
            mask_win,
            state: 0,
        }
    }

    fn get_prime_screen(screens: &Vec<Screen>) -> Option<&Screen> {
        for screen in screens {
            if screen.display_info.is_primary { return Some(screen); }
        }
        return None;
    }

    // void ScreenShotter::onHotkey(unsigned int fsModifiers, unsigned int  vk)
    // {
    //     if(m_state == 1)return;
    //     if(vk == (UINT)0x43) Shot();
    //     else if(vk == (UINT)0x48) HideAll();
    // }

    // void ScreenShotter::HideAll()
    // {
    //     bool ifMinimize = false;
    //     foreach (ShotterWindow* win, m_ShotterWindowList) {
    //         if(win->windowState() != Qt::WindowMinimized){
    //             ifMinimize = true;
    //             break;
    //         }
    //     }
    //     foreach (ShotterWindow* win, m_ShotterWindowList) {
    //         if(ifMinimize == true) win->minimize();
    //         else win->setWindowState(Qt::WindowNoState);
    //     }
    // }

    // // 绘制背景和选区
    // void ScreenShotter::paintEvent(QPaintEvent *)
    // {
    //     QPainter painter(this);
    //     // 绘制背景
    //     painter.drawPixmap(0, 0, m_desktopRect.width(), m_desktopRect.height(), *m_backgroundScreen);
    //     // 绘制选区
    //     if (!m_windowRect.isEmpty()) {
    //         QPen pen = painter.pen();
    //         pen.setColor(QColor(0,175,255));
    //         pen.setWidth(2);
    //         pen.setJoinStyle(Qt::MiterJoin);
    //         painter.setPen(pen);
    //         float x = m_windowRect.x()  / m_scaleRate;
    //         float y = m_windowRect.y()  / m_scaleRate;
    //         float width = m_windowRect.width() / m_scaleRate;
    //         float height = m_windowRect.height() / m_scaleRate;
    //         QRectF scaledRect = QRectF(x, y, width, height);
    //         painter.drawPixmap(QPointF(x, y), *m_originPainting, m_windowRect); // 绘制截屏编辑窗口
    //         painter.drawRect(scaledRect); // 绘制边框线
    //     }
    // }
    
    // 更新鼠标区域窗口
    fn update_mouse_win() {
        //     POINT pt;
        //     ::GetCursorPos(&pt); // 获得当前鼠标位置
        //     if (m_state == 1){
        //         ::EnableWindow((HWND)winId(), FALSE);
        //         // 获得当前位置桌面上的子窗口
        //         HWND hwnd = ::ChildWindowFromPointEx(::GetDesktopWindow(), pt, CWP_SKIPDISABLED | CWP_SKIPINVISIBLE | CWP_SKIPTRANSPARENT);
        //         RECT temp_window;
        //         ::DwmGetWindowAttribute(hwnd, DWMWA_EXTENDED_FRAME_BOUNDS, &temp_window, sizeof(temp_window));
        //         m_windowRect.setRect(temp_window.left,temp_window.top, temp_window.right - temp_window.left, temp_window.bottom - temp_window.top);
        //         ::EnableWindow((HWND)winId(), TRUE);
        //     }else if (m_state == 2){
        //         const int& rx = (pt.x >= m_startPoint.x()) ? m_startPoint.x() : pt.x;
        //         const int& ry = (pt.y >= m_startPoint.y()) ? m_startPoint.y() : pt.y;
        //         const int& rw = abs(pt.x - m_startPoint.x());
        //         const int& rh = abs(pt.y - m_startPoint.y());
        //         m_windowRect.setRect(rx, ry, rw, rh); // 改变大小
        //     }
        //     m_amplifierTool->onSizeChange(m_windowRect.width(), m_windowRect.height());
    }

    fn init_amplifier() {
        //     m_amplifierTool.reset(new Amplifier(m_originPainting, this));
        //     connect(this, &ScreenShotter::cursorPosChange, m_amplifierTool.get(), &Amplifier::onPostionChange);
        //     m_amplifierTool->show();
        //     m_amplifierTool->raise();
    }

    fn end_shot() {
        //     m_amplifierTool->hide(); // 隐藏放大器
        //     this->hide();
        //     m_state = 0;
        //     foreach (ShotterWindow* win, m_ShotterWindowList) win->show();
        //     if(m_ShotterWindowList.length()>0) m_ShotterWindowList.last()->raise();
        //     m_isHidden = false;
    }

    // fn on_pin_win_move(pin_win: &PinWindow) {
    //     //     foreach (ShotterWindow* otherWin, m_ShotterWindowList) {
    //     //         if(otherWin != shotterWindow){
    //     //             QRect rectA = shotterWindow->geometry();
    //     //             QRect rectB = otherWin->geometry();
    //     //             int padding = 10;

    //     //             if(!(rectA.top() > rectB.bottom()) && !(rectA.bottom() < rectB.top())){
    //     //                 if( qAbs(rectA.right() - rectB.left()) < padding)
    //     //                     shotterWindow->stick(STICK_TYPE::RIGHT_LEFT, otherWin);
    //     //                 else if( qAbs(rectA.right() - rectB.right()) < padding)
    //     //                     shotterWindow->stick(STICK_TYPE::RIGHT_RIGHT, otherWin);
    //     //                 else if( qAbs(rectA.left() - rectB.right()) < padding)
    //     //                     shotterWindow->stick(STICK_TYPE::LEFT_RIGHT, otherWin);
    //     //                 else if( qAbs(rectA.left() - rectB.left()) < padding)
    //     //                     shotterWindow->stick(STICK_TYPE::LEFT_LEFT, otherWin);
    //     //             }

    //     //             if(!(rectA.right() < rectB.left()) && !(rectA.left() > rectB.right())){
    //     //                 if( qAbs(rectA.top() - rectB.bottom()) < padding)
    //     //                     shotterWindow->stick(STICK_TYPE::UPPER_LOWER, otherWin);
    //     //                 else if( qAbs(rectA.bottom() - rectB.top()) < padding)
    //     //                     shotterWindow->stick(STICK_TYPE::LOWER_UPPER, otherWin);
    //     //                 else if( qAbs(rectA.top() - rectB.top()) < padding)
    //     //                     shotterWindow->stick(STICK_TYPE::UPPER_UPPER, otherWin);
    //     //                 else if( qAbs(rectA.bottom() - rectB.bottom()) < padding)
    //     //                     shotterWindow->stick(STICK_TYPE::LOWER_LOWER, otherWin);
    //     //             }
    //     //         }
    //     //     }
    // }

    fn switch_hide_pin_win() {
        //     if(m_isHidden == false){
        //         endShot();
        //         foreach (ShotterWindow* win, m_ShotterWindowList) win->hide();
        //         m_isHidden = true;
        //         Shot();
        //     }
    }
}

slint::slint! {
    export component MaskWindow inherits Window {
        no-frame: true;
        always-on-top: true;
        forward-focus: focus_scope;
        
        in property <image> bac_image;

        callback shot();
        callback point_event(PointerEvent);
        callback key_released(KeyEvent);

        VerticalLayout {
            Image {
                source: bac_image;
                width: root.width;
                height: root.height;
                image-fit: contain;

                Rectangle {
                    background: rgba(0, 0, 0, 0.5);

                    focus_scope := FocusScope {
                        key-released(event) => {
                            debug(123);
                            root.key_released(event);
                            accept
                        }
                    }

                    TouchArea {
                        pointer-event(event) => {
                            root.point_event(event);
                        }
                    }
                }
            }
        }
    }
}