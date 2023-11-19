
pub struct Setting {
    pub setting_win: SettingWindow,
}

impl Setting {
    pub fn new() -> Setting {
        let setting_win = SettingWindow::new().unwrap();

        Setting {
            setting_win
        }
    }
}

slint::slint! {
    import { CheckBox, StandardListView, StyleMetrics } from "std-widgets.slint";
    import { AboutPage, BaseSettingPage, ScreenShotterSettingPage, SearchSettingPage } from "src/core/application/setting/pages/pages.slint";
    import { GallerySettings } from "src/core/application/setting/gallery_settings.slint";
    import { SideBar } from "src/core/application/setting/side_bar.slint";

    export component SettingWindow inherits Window {
        preferred-width: 700px;
        preferred-height: 500px;
        title: @tr("设置");
        icon: @image-url("assets/logo.png");

        HorizontalLayout {
            side-bar := SideBar {
                model: [
                    @tr("Menu" => "基础"),
                    @tr("Menu" => "搜索"),
                    @tr("Menu" => "截图"),
                    @tr("Menu" => "关于"),
                ];
            }

            if(side-bar.current-item == 0) : BaseSettingPage {}
            if(side-bar.current-item == 1) : SearchSettingPage {}
            if(side-bar.current-item == 2) : ScreenShotterSettingPage {}
            if(side-bar.current-item == 3) : AboutPage {}
        }
    }
}