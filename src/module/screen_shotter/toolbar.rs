slint::slint! {
    export component Toolbar inherits PopupWindow {

    }
    // Toolbar::Toolbar(QWidget *parent): QWidget{parent}
    // {
    //     initUI();
    // }

    // void Toolbar::movePosition(QRect rect)
    // {
    //     this->move(rect.bottomRight().x() - 120, rect.bottomRight().y() + 4);
    // }

    // bool Toolbar::event(QEvent *e)
    // {
    //     if (e->type() == QEvent::ActivationChange)
    //         if(QApplication::activeWindow() != this && QApplication::activeWindow() != this->parent()) this->hide();

    //     return QWidget::event(e);
    // }

    // void Toolbar::initUI()
    // {
    //     setWindowFlags(Qt::FramelessWindowHint | Qt::Tool | Qt::WindowStaysOnTopHint);
    //     setFixedHeight(30);
    //     setFixedWidth(120);

    //     int fontId = QFontDatabase::addApplicationFont(":/picture/Resources/iconfont.ttf");
    //     QFontDatabase::applicationFontFamilies(fontId);
    //     QFont iconFont = QFont("iconfont");

    //     m_layout = new QHBoxLayout();
    //     m_layout->setAlignment(Qt::AlignRight);
    //     m_layout->setContentsMargins(0,0,0,0);
    //     m_layout->setSpacing(0);
    //     this->setLayout(m_layout);

    //     QPushButton* saveBtn = new QPushButton();
    //     saveBtn->setFont(iconFont);
    //     saveBtn->setText(QChar(0xe936));
    //     saveBtn->setFixedSize(30, 30);
    //     connect(saveBtn,&QPushButton::clicked, [this]{emit sgn_save();});
    //     saveBtn->setToolTip("保存截图 (Ctrl+S)");
    //     m_layout->addWidget(saveBtn);

    //     QPushButton* miniBtn = new QPushButton();
    //     miniBtn->setFont(iconFont);
    //     miniBtn->setText(QChar(0xe650));
    //     miniBtn->setFixedSize(30, 30);
    //     connect(miniBtn,&QPushButton::clicked, [this]{emit sgn_minimize();});
    //     miniBtn->setToolTip("最小化截图 (H)");
    //     m_layout->addWidget(miniBtn);

    //     QPushButton* closeBtn = new QPushButton();
    //     closeBtn->setFont(iconFont);
    //     closeBtn->setText(QChar(0xe65d));
    //     closeBtn->setFixedSize(30, 30);
    //     connect(closeBtn,&QPushButton::clicked, [this]{emit sgn_close();});
    //     closeBtn->setToolTip("关闭截图 (Esc)");
    //     m_layout->addWidget(closeBtn);

    //     QPushButton* completeBtn = new QPushButton();
    //     completeBtn->setFont(iconFont);
    //     completeBtn->setText(QChar(0xec9e));
    //     completeBtn->setFixedSize(30, 30);
    //     connect(completeBtn,&QPushButton::clicked, [this]{emit sgn_complete();});
    //     completeBtn->setToolTip("完成截图 (Enter)");
    //     m_layout->addWidget(completeBtn);

    //     this->setStyleSheet("QPushButton{font-size:16px;border:0px} QPushButton:hover{color:rgb(0,175,255)}");
    // }
}



