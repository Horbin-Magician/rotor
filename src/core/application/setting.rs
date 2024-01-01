
pub struct Setting {
    pub setting_win: SettingWindow,
}

impl Setting {
    pub fn new() -> Setting {
        let setting_win = SettingWindow::new().unwrap();

        let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
        setting_win.set_version(version.into());

        Setting {
            setting_win
        }
    }
}

slint::slint! {
    import { CheckBox, StandardListView, StyleMetrics } from "std-widgets.slint";
    import { AboutPage, BaseSettingPage, ScreenShotterSettingPage, SearchSettingPage } from "src/core/application/setting/pages/pages.slint";
    import { SideBar } from "src/core/application/setting/side_bar.slint";

    export component SettingWindow inherits Window {
        width: 500px;
        height: 400px;
        title: @tr("设置");
        icon: @image-url("assets/logo.png");

        in property <string> version;

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