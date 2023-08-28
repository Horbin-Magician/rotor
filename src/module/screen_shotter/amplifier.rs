

pub struct Amplifier {
    // float m_scaleRate;
    // QSize m_screenSize; // 外部组件的大小信息
    // QPoint m_cursorPoint; // 鼠标的位置
    // int m_sideLength; // 取色放大器的边长
    // int m_imageHeight; // 放大区的高度
    // std::shared_ptr<QPixmap> m_originPainting; // 屏幕原画
    // int m_colorType;
    // QString m_colorStr;
}

impl Amplifier {
    pub fn new() -> Amplifier {
        // Amplifier::Amplifier(std::shared_ptr<QPixmap> originPainting, QWidget *parent) :QWidget(parent){
        //     m_originPainting = originPainting;

        //     setWindowFlags(Qt::FramelessWindowHint | Qt::WindowSystemMenuHint); // 设置成无边框对话框
        //     setMouseTracking(true); // 开启鼠标实时追踪

        //     // 设置默认大小
        //     m_scaleRate = QGuiApplication::primaryScreen()->devicePixelRatio();
        //     m_sideLength = 120;
        //     m_imageHeight = 90;
        //     setFixedSize(m_sideLength, m_imageHeight + 50);
        //     // 设置默认值
        //     m_colorType = 0;
        //     m_colorStr = "";

        //     hide(); // 默认隐藏
        // }
        Amplifier {

        }
    }

    fn switch_color_type() {
        //     if(m_colorType == 0) m_colorType = 1;
        //     else m_colorType = 0;
        //     update();
        //     return m_colorType;
    }

    fn get_color_str() {
        //     return m_colorStr;
    }

    fn on_size_change(w: i32, h: i32) {
        //     m_screenSize = QSize(w, h);
    }

    fn on_position_change(x: i32, y: i32) {
        //     m_cursorPoint = QPoint(x, y);
        //     raise();
        //     int dest_x = x + 4;
        //     int dest_y = y + 26;

        //     // 超出屏幕检测
        //     const QSize& parent_size = parentWidget()->size();
        //     if (dest_y + height() > parent_size.height()) {
        //         dest_y = y - 26 - height();
        //     }
        //     if (dest_x + width() > parent_size.width()) {
        //         dest_x = x - 4 - width();
        //     }

        //     move(dest_x, dest_y);
    }

    fn paint_event() {
        // /// 绘制鼠标拖拽时选区矩形的右下顶点的放大图;
        // void Amplifier::paintEvent(QPaintEvent *) {
        //     QPainter painter(this);
        //     /// 绘制背景
        //     painter.fillRect(rect(), QColor(0, 0, 0, 160));
        //     /// 绘制放大图;
        //     QPixmap endPointImage;

        //     int scale = 4;
        //     int x_start = m_cursorPoint.x() * m_scaleRate - m_sideLength * m_scaleRate / 4 / 2;
        //     int y_start = m_cursorPoint.y() * m_scaleRate - m_imageHeight * m_scaleRate / 4 / 2;
        //     int x_end = m_cursorPoint.x() * m_scaleRate + m_sideLength * m_scaleRate / 4 / 2;
        //     int y_end = m_cursorPoint.y() * m_scaleRate + m_imageHeight * m_scaleRate / 4 / 2;

        //     if(x_start < 0) x_start = 0;
        //     if(y_start < 0) y_start = 0;
        //     if(x_end > m_originPainting->width()) x_end = m_originPainting->width() - 1;
        //     if(y_end > m_originPainting->height()) y_end = m_originPainting->height() - 1;

        //     endPointImage = m_originPainting->copy(QRect(QPoint(x_start, y_start), QPoint(x_end, y_end)));
        //     endPointImage = endPointImage.scaled((x_end - x_start) * 4, (y_end - y_start) * 4);

        //     int offset_x = x_start / m_scaleRate * 4 + m_sideLength / 2 - m_cursorPoint.x() * 4;
        //     int offset_y = y_start / m_scaleRate * 4 + m_imageHeight / 2 - m_cursorPoint.y() * 4;
        //     painter.drawPixmap(offset_x, offset_y, endPointImage);

        //     // 绘制十字
        //     painter.setPen(QPen(QColor(0, 180, 255 , 180), 4));
        //     // 竖线;
        //     painter.drawLine(QPoint(m_sideLength >> 1, 0), QPoint(m_sideLength >> 1, m_imageHeight - 4));
        //     // 横线;
        //     painter.drawLine(QPoint(0, m_imageHeight >> 1), QPoint(m_sideLength, m_imageHeight >> 1));

        //     // 绘制大图内边框
        //     painter.setPen(QPen(Qt::white, 2));
        //     painter.drawRect(2,2,width()-4, m_imageHeight-4);

        //     // 绘制外边框
        //     painter.setPen(QPen(Qt::black, 1));
        //     painter.drawRect(0,0,width()-1,height()-1);

        //     // 当前选中矩形的宽高信息;
        //     QString select_screen_info = QString("%1×%2").arg(m_screenSize.width()).arg(m_screenSize.height());

        //     // 当前鼠标像素值的RGB信息
        //     QImage image = m_originPainting->toImage();
        //     QColor cursor_pixel = image.pixel(m_cursorPoint * m_scaleRate);
        //     if(m_colorType == 0) m_colorStr = QString("RGB(%1,%2,%3)").arg(cursor_pixel.red()).arg(cursor_pixel.green()).arg(cursor_pixel.blue());
        //     if(m_colorType == 1) m_colorStr = QString("HSV(%1,%2,%3)").arg(cursor_pixel.hue()).arg(cursor_pixel.saturation()).arg(cursor_pixel.value());
        //     // 绘制坐标轴相关数据
        //     painter.setPen(Qt::white);
        //     painter.drawText(QPoint(6, m_imageHeight + 14), select_screen_info);
        //     painter.drawText(QPoint(6, m_imageHeight + 28), m_colorStr);
        //     painter.drawText(QPoint(6, m_imageHeight + 42), "Z键切换 C键复制");
        // }
    }
}


slint::slint! {
    export component AmplifierWin inherits Window {

    }
}