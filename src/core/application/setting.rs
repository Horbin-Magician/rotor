pub mod app_config;

use i_slint_backend_winit::WinitWindowAccessor;
use windows_sys::Win32::UI::WindowsAndMessaging;
use app_config::AppConfig;

use crate::core::util::net_util;

pub struct Setting {
    pub setting_win: SettingWindow,
}

impl Setting {
    pub fn new() -> Setting {
        let setting_win = SettingWindow::new().unwrap();

        {
            let width: f32 = 500.;
            let height: f32 = 400.;
            let x_screen: f32;
            let y_screen: f32;
            unsafe{
                x_screen = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXSCREEN) as f32;
                y_screen = WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CYSCREEN) as f32;
            }
            let x_pos = ((x_screen - width * setting_win.window().scale_factor()) * 0.5) as i32;
            let y_pos = ((y_screen - height * setting_win.window().scale_factor()) * 0.4) as i32;
            setting_win.window().set_position(slint::WindowPosition::Physical(slint::PhysicalPosition::new(x_pos, y_pos)));
        }

        {   
            let version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");
            setting_win.set_version(version.into());
            
            let app_config = AppConfig::global().lock().unwrap();
            setting_win.set_power_boot(app_config.get_power_boot());
            setting_win.set_theme(app_config.get_theme() as i32);
        }

        { // power boot
            setting_win.on_power_boot_changed(move |power_boot| {
                let mut app_config = AppConfig::global().lock().unwrap();
                app_config.set_power_boot(power_boot);
            });
        }

        {// theme
            setting_win.on_theme_changed(move |theme| {
                let mut app_config = AppConfig::global().lock().unwrap();
                app_config.set_theme(theme as u8);
            });
            // if theme == 0 {
            //     setting_win_clone.global::<Palette>().set_color_scheme("unknown");
            // } else {
            //     setting_win_clone.global::<Palette>().set_color_scheme("unknown");
            // }
            // // Palette.color-scheme = self.checked ? ColorScheme.dark : ColorScheme.light; 
        }

        { // minimize, close, win move
            let setting_win_clone = setting_win.as_weak();
            setting_win.on_minimize(move || {
                setting_win_clone.unwrap().window().with_winit_window(|winit_win| {
                    winit_win.set_minimized(true);
                });
            });

            let setting_win_clone = setting_win.as_weak();
            setting_win.on_close(move || {
                setting_win_clone.unwrap().hide().unwrap();
            });

            let setting_win_clone = setting_win.as_weak();
            setting_win.on_win_move(move || {
                setting_win_clone.unwrap().window().with_winit_window(|winit_win| {
                    let _ = winit_win.drag_window();
                });
            });
        }

        { // update
            let setting_win_clone = setting_win.as_weak();
            setting_win.on_check_update(move || {
                let setting_window = setting_win_clone.unwrap();
                setting_window.set_block(true);

                let setting_win_clone = setting_win_clone.clone();
                std::thread::spawn(move || {
                    let latest_version = net_util::Updater::global().lock().unwrap().get_latest_version().unwrap_or("unknown".to_string());
                    let current_version = option_env!("CARGO_PKG_VERSION").unwrap_or("unknown");

                    setting_win_clone.upgrade_in_event_loop(move |setting_window| {
                        setting_window.set_current_version(current_version.into());
                        setting_window.set_latest_version(latest_version.into());
                        setting_window.set_update_state(1);
                        setting_window.set_block(false);
                    }).unwrap();
                });
            });

            let setting_win_clone = setting_win.as_weak();
            setting_win.on_update(move || {
                let setting_window = setting_win_clone.unwrap();
                setting_window.set_block(true);
                setting_window.set_update_state(0);

                let setting_win_clone = setting_win_clone.clone();
                std::thread::spawn(move || {
                    net_util::Updater::global().lock().unwrap().update_software().unwrap_or_else(
                        |_| {
                            setting_win_clone.upgrade_in_event_loop(move |setting_window| {
                                setting_window.set_block(false);
                                setting_window.set_update_state(2);
                            }).unwrap();
                        }
                    );
                });
            });
        }

        Setting {
            setting_win
        }
    }
}

slint::slint! {
    import { CheckBox, StandardListView, Palette, Button, ProgressIndicator } from "std-widgets.slint";
    import { BaseSettingPage, ScreenShotterSettingPage, SearchSettingPage } from "src/core/application/setting/UI/pages/pages.slint";
    import { SideBar } from "src/core/application/setting/UI/side_bar.slint";
    import { TitleBar } from "src/core/application/setting/UI/title_bar.slint";

    export component SettingWindow inherits Window {
        default-font-size: 14px;
        width: 500px;
        height: 400px;
        title: @tr("设置");
        icon: @image-url("assets/logo.png");
        no-frame: true;
        background: transparent;

        callback minimize <=> title_bar.minimize;
        callback close <=> title_bar.close;
        callback win_move;

        callback power_boot_changed(bool);
        callback theme_changed(int);
        callback check_update();
        callback update();

        in property <string> version;
        in-out property <int> update_state: 0;
        in-out property <string> current_version;
        in-out property <string> latest_version;
        in-out property <bool> power_boot;
        in-out property <int> theme;

        in-out property <bool> block;
        in-out property <float> progress: -1.0;

        touch := TouchArea {
            pointer-event(event) => {
                if (event.button == PointerEventButton.left && event.kind == PointerEventKind.down) {
                    win_move();
                }
            }

            Rectangle {
                height: (root.height) - 4px;
                width: (root.width) - 4px;

                background: Palette.background;
                border-color: Palette.background.brighter(1);
                border-width: 1px;
                border-radius: 14px;
                clip: true;
                
                VerticalLayout {

                    HorizontalLayout {
                        side-bar := SideBar {
                            model: [
                                @tr("Menu" => "基础"),
                                @tr("Menu" => "搜索"),
                                @tr("Menu" => "截图"),
                            ];
                        }

                        VerticalLayout {
                            title_bar := TitleBar {}

                            Rectangle {
                                if(side-bar.current-item == 0) : 
                                    BaseSettingPage {
                                        version: version;
                                        power_boot <=> root.power_boot;
                                        theme <=> root.theme;
                                        theme_changed(theme) => { root.theme_changed(theme); }
                                        power_boot_changed(power_boot) => { root.power_boot_changed(power_boot); }
                                        check_update() => { root.check_update(); }
                                    }
                                if(side-bar.current-item == 1) :
                                    SearchSettingPage {}
                                if(side-bar.current-item == 2) :
                                    ScreenShotterSettingPage {}
                            }
                        }
                    }

                    if block : ProgressIndicator {
                        width: 100%;
                        height: 4px;
                        indeterminate: root.progress < 0.0 ? true : false;
                        progress: root.progress < 0.0 ? 0.0 : root.progress;
                    }
                }


            }
        }

        if (update_state > 0) : Rectangle {
            height: root.height;
            width: root.width;
            background: Palette.background.with_alpha(0.5);

            in-out property <int> update_state <=> root.update_state;
            in-out property <string> current_version <=> root.current_version;
            in-out property <string> latest_version <=> root.latest_version;
            
            TouchArea {
                clicked() => { update_state = 0; }
                Rectangle {
                    height: 150px;
                    width: 200px;
                    background: Palette.background;
                    border-width: 1px;
                    border-radius: 14px;
                    border-color: Palette.background.brighter(1);
                    
                    if (update_state == 1 && current_version == latest_version) : VerticalLayout {
                        alignment: center;
                        spacing: 5px;
                        Text {
                            horizontal-alignment: center;
                            vertical-alignment: center;
                            text: @tr("当前已是最新版本");
                        }
                        HorizontalLayout {
                            padding-top: 10px;
                            alignment: center;
                            Button {
                                height: 30px;
                                width: 60px;
                                text: @tr("确定");
                                clicked() => { root.update_state = 0; }
                            }
                        }
                    }

                    if (update_state == 1 && current_version != latest_version && latest_version != "unknown"): VerticalLayout {
                        alignment: center;
                        spacing: 8px;
                        padding-top: 10px;
                        Text {
                            horizontal-alignment: center;
                            vertical-alignment: center;
                            text: @tr("当前版本: {}", current_version);
                        }
                        Text {
                            horizontal-alignment: center;
                            vertical-alignment: center;
                            text: @tr("最新版本: {}", latest_version);
                        }
                        Text {
                            horizontal-alignment: center;
                            vertical-alignment: center;
                            text: @tr("是否更新至最新版本？");
                        }

                        HorizontalLayout {
                            padding-top: 10px;
                            alignment: center;
                            spacing: 10px;
                            Button {
                                height: 28px;
                                width: 60px;
                                text: @tr("是");
                                clicked() => { root.update(); }
                            }
                            Button {
                                height: 28px;
                                width: 60px;
                                text: @tr("否");
                                clicked() => { root.update_state = 0; }
                            }
                        }
                    }

                    if (update_state == 1 && latest_version == "unknown"): VerticalLayout {
                        alignment: center;
                        spacing: 5px;
                        Text {
                            horizontal-alignment: center;
                            vertical-alignment: center;
                            text: @tr("获取版本信息失败");
                        }

                        Text {
                            horizontal-alignment: center;
                            vertical-alignment: center;
                            text: @tr("请检查网络环境");
                        }

                        HorizontalLayout {
                            padding-top: 10px;
                            alignment: center;
                            spacing: 10px;
                            Button {
                                height: 28px;
                                width: 60px;
                                text: @tr("确定");
                                clicked() => { root.update_state = 0; }
                            }
                        }
                    }

                    if (update_state == 2): VerticalLayout {
                        alignment: center;
                        spacing: 5px;
                        Text {
                            horizontal-alignment: center;
                            vertical-alignment: center;
                            text: @tr("更新失败");
                        }
                        Text {
                            horizontal-alignment: center;
                            vertical-alignment: center;
                            text: @tr("请检查网络环境");
                        }

                        HorizontalLayout {
                            padding-top: 10px;
                            alignment: center;
                            spacing: 10px;
                            Button {
                                height: 28px;
                                width: 60px;
                                text: @tr("确定");
                                clicked() => { root.update_state = 0; }
                            }
                        }
                    }
                }
            }
        }
    }
}