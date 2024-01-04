use i_slint_backend_winit::WinitWindowAccessor;

pub struct Setting {
    pub setting_win: SettingWindow,
}

impl Setting {
    pub fn new() -> Setting {
        let setting_win = SettingWindow::new().unwrap();

        let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
        setting_win.set_version(version.into());

        {
            let setting_win_clone = setting_win.as_weak();
            setting_win.on_minimize(move || {
                setting_win_clone.unwrap().window().with_winit_window(|winit_win| {
                    winit_win.set_minimized(true);
                });
            });
        }

        {
            let setting_win_clone = setting_win.as_weak();
            setting_win.on_close(move || {
                setting_win_clone.unwrap().hide().unwrap();
            });
        }


        {
            let setting_win_clone = setting_win.as_weak();
            setting_win.on_win_move(move || {
                setting_win_clone.unwrap().window().with_winit_window(|winit_win| {
                    winit_win.drag_window();
                });
            });
        }

        Setting {
            setting_win
        }
    }
}

slint::slint! {
    import { CheckBox, StandardListView, StyleMetrics } from "std-widgets.slint";
    import { AboutPage, BaseSettingPage, ScreenShotterSettingPage, SearchSettingPage } from "src/core/application/setting/pages/pages.slint";
    import { SideBar } from "src/core/application/setting/side_bar.slint";
    import { TitleBar } from "src/core/application/setting/title_bar.slint";

    export component SettingWindow inherits Window {
        width: 500px;
        height: 400px;
        title: @tr("设置");
        icon: @image-url("assets/logo.png");
        no-frame: true;
        background: transparent;

        callback minimize <=> title_bar.minimize;
        callback close <=> title_bar.close;
        callback win_move <=> title_bar.win_move;

        in property <string> version;

        Rectangle {
            height: root.height - 4px;
            width: root.width - 4px;

            background: StyleMetrics.window-background;
            border-color: StyleMetrics.window-background.brighter(1).with_alpha(0.2);
            border-width: 2px;
            border-radius: 5px;
            clip: true;

            VerticalLayout {
                title_bar := TitleBar {}
                HorizontalLayout {
                    side-bar := SideBar {
                        model: [
                            @tr("Menu" => "基础"),
                            @tr("Menu" => "搜索"),
                            @tr("Menu" => "截图"),
                            @tr("Menu" => "关于"),
                        ];
                    }
                    Rectangle {
                        if(side-bar.current-item == 0) : BaseSettingPage {}
                        if(side-bar.current-item == 1) : SearchSettingPage {}
                        if(side-bar.current-item == 2) : ScreenShotterSettingPage {}
                        if(side-bar.current-item == 3) : AboutPage {version: version;}
                    }
                }
            }
        }
    }
}