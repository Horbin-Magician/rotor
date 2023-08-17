
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
    import { Button, VerticalBox } from "std-widgets.slint";

    export component SettingWindow inherits Window {
        width: 800px;
        height: 600px;
        
        VerticalBox {
            Text {
                text: "Counter";
            }
            Button {
                text: "Increase value";
            }
        }
    }
}