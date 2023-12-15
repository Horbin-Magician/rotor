use windows_sys::Win32::UI::WindowsAndMessaging;
use i_slint_backend_winit::WinitWindowAccessor;
use raw_window_handle::HasRawWindowHandle;


pub struct Toolbar {
    pub toolbar_window: ToolbarWindow,
}

impl Toolbar {
    pub fn new() -> Toolbar {
        let toolbar_window = ToolbarWindow::new().unwrap();
        Toolbar {
            toolbar_window,
        }
    }

    pub fn show(&self) {
        self.toolbar_window.show().unwrap();
    }
}


slint::slint! {
    export component ToolbarWindow inherits Window {
        height: 40px;
        background: red;
        no-frame: true;
        always-on-top: true;

        in-out property <length> win_width: 300px;
        width <=> win_width;
    }
}