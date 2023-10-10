
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
    import { AboutPage, ControlsPage } from "src/core/application/setting/pages/pages.slint";
    import { GallerySettings } from "src/core/application/setting/gallery_settings.slint";
    import { SideBar } from "src/core/application/setting/side_bar.slint";

    export component SettingWindow inherits Window {
        preferred-width: 700px;
        preferred-height: 500px;
        title: @tr("Slint Widgets Gallery");
        icon: @image-url("assets/logo.png");

        HorizontalLayout {
            side-bar := SideBar {
                title: @tr("Slint Widgets Gallery");
                model: [@tr("Menu" => "Controls"), @tr("Menu" => "About")];
            }

            if(side-bar.current-item == 0) : ControlsPage {}
            if(side-bar.current-item == 1) : AboutPage {}
        }
    }
}