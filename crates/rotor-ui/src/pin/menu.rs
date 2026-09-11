use super::*;
use gpui_kit::component::native_menu::NativeMenu;

actions!(pin_menu, [Annotate, Ocr, Minimize, Save, Close, Copy]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Command {
    Annotate,
    Ocr,
    Minimize,
    Save,
    Close,
    Copy,
}

const COMMANDS: [Command; 6] = [
    Command::Annotate,
    Command::Ocr,
    Command::Minimize,
    Command::Save,
    Command::Close,
    Command::Copy,
];

pub(super) const CONTEXT_COMMANDS: &[Command] = &[
    Command::Minimize,
    Command::Save,
    Command::Close,
    Command::Copy,
];

// Button widths, gaps and panel padding must agree with toolbar::button/panel.
fn layout(available: f32) -> (usize, f32) {
    let available = available.max(0.);
    if available >= 183. {
        return (0, 183.);
    }
    for hidden in 1..=6 {
        let width = 8. + (7 - hidden) as f32 * 29. - 2.;
        if width <= available || hidden == 6 {
            return (hidden, width.min(available));
        }
    }
    unreachable!()
}

impl PinView {
    fn command_info(&self, command: Command) -> (&'static str, toolbar::Glyph, String) {
        let (id, glyph, label, shortcut) = match command {
            Command::Annotate => (
                "pin-annotate",
                toolbar::Glyph::Pen,
                self.t("标注", "Annotate"),
                "",
            ),
            Command::Ocr => ("pin-ocr", toolbar::Glyph::Ocr, "OCR", ""),
            Command::Minimize => (
                "pin-minimize",
                toolbar::Glyph::Minimize,
                self.t("最小化", "Minimize"),
                "shortcut_pinwin_hide",
            ),
            Command::Save => (
                "pin-save",
                toolbar::Glyph::Save,
                self.t("保存", "Save"),
                "shortcut_pinwin_save",
            ),
            Command::Close => (
                "pin-close",
                toolbar::Glyph::Close,
                self.t("关闭", "Close"),
                "shortcut_pinwin_close",
            ),
            Command::Copy => (
                "pin-copy",
                toolbar::Glyph::Copy,
                self.t("复制", "Copy"),
                "shortcut_pinwin_copy",
            ),
        };
        (id, glyph, self.shortcut_hint(label, shortcut))
    }

    fn command_disabled(&self, command: Command) -> bool {
        let export_disabled = self.busy() || !self.canvas.ready() || self.crop_drag.is_some();
        match command {
            Command::Ocr => self.ocr.loading() || (!self.ocr.active && export_disabled),
            Command::Minimize | Command::Close => self.busy(),
            _ => export_disabled,
        }
    }

    pub(super) fn run_command(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // A menu can stay open while runtime events change availability.
        if self.command_disabled(command) {
            return;
        }
        match command {
            Command::Annotate => {
                self.clear_ocr(window);
                self.set_tool(annotation::Tool::Pen, window, cx);
            }
            Command::Ocr => self.toggle_ocr(window, cx),
            Command::Minimize => self.minimize(window, cx),
            Command::Save => self.save(window, cx),
            Command::Close => self.close(window, cx),
            Command::Copy => self.export(PinExportTarget::Clipboard, window, cx),
        }
    }

    pub(super) fn show_pin_menu(
        &self,
        commands: &[Command],
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut menu = NativeMenu::new();
        for &command in commands {
            let (_, _, label) = self.command_info(command);
            let action: Box<dyn Action> = match command {
                Command::Annotate => Box::new(Annotate),
                Command::Ocr => Box::new(Ocr),
                Command::Minimize => Box::new(Minimize),
                Command::Save => Box::new(Save),
                Command::Close => Box::new(Close),
                Command::Copy => Box::new(Copy),
            };
            menu = menu.menu_with_disabled(label, self.command_disabled(command), action);
        }
        // Native menus dispatch their action through the owning window's focus path.
        self.focus.focus(window, cx);
        menu.show(position, window, cx);
    }

    pub(super) fn pin_toolbar(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (hidden, width) = layout(window.viewport_size().width.as_f32() - 16.);
        let mut row = div().flex().items_center().justify_center().gap(px(2.));
        if hidden > 0 {
            row = row.child(
                toolbar::button("pin-more", toolbar::Glyph::More, cx)
                    .accessibility_label(self.t("更多功能", "More actions"))
                    .tooltip(self.t("更多功能", "More actions"))
                    .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        this.show_pin_menu(&COMMANDS[..hidden], event.position(), window, cx);
                    })),
            );
        }
        for (index, &command) in COMMANDS.iter().enumerate().skip(hidden) {
            if hidden == 0 && index == 2 {
                row = row.child(toolbar::separator());
            }
            let (id, glyph, label) = self.command_info(command);
            row = row.child(
                toolbar::button(id, glyph, cx)
                    .accessibility_label(label.clone())
                    .tooltip(label)
                    .selected(matches!(command, Command::Ocr) && self.ocr.active)
                    .loading(matches!(command, Command::Ocr) && self.ocr.loading())
                    .disabled(self.command_disabled(command))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.run_command(command, window, cx)
                    })),
            );
        }
        toolbar::panel("pin-toolbar", px(width), window).child(row)
    }
}

#[cfg(test)]
mod tests {
    use super::layout;

    #[test]
    fn narrowing_hides_a_prefix_and_reserves_the_more_button() {
        for (available, hidden, width) in [
            (200., 0, 183.),
            (183., 0, 183.),
            (182., 1, 180.),
            (180., 1, 180.),
            (179., 2, 151.),
            (151., 2, 151.),
            (150., 3, 122.),
            (121., 4, 93.),
            (92., 5, 64.),
            (63., 6, 35.),
            (20., 6, 20.),
            (0., 6, 0.),
        ] {
            assert_eq!(layout(available), (hidden, width));
        }
    }

    #[gpui::test]
    fn native_menu_actions_reach_the_pin_and_recheck_availability(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        use super::super::{PinInit, PinView};
        use gpui_kit::*;
        use std::{
            rc::Rc,
            sync::{Arc, Mutex},
        };
        let profile = tempfile::tempdir().unwrap();
        let (services, _events) = rotor_runtime::Services::new(
            Arc::new(Mutex::new(
                rotor_common::ConfigService::load_from(profile.path()).unwrap(),
            )),
            None,
            rotor_runtime::ServiceOptions { index_files: false },
        )
        .unwrap();
        cx.update(gpui_kit::component::init);
        let (pin, cx) = cx.add_window_view(|window, cx| {
            window.resize(size(px(400.), px(400.)));
            PinView::new(
                Arc::new(services),
                PinInit {
                    image: crate::prepare_image(Arc::new(image::RgbaImage::new(400, 400))).unwrap(),
                    config: rotor_runtime::ShotterConfig {
                        annotations: Vec::new(),
                        monitor_pos: (0, 0),
                        monitor_size: (400, 400),
                        rect: (0, 0, 400, 400),
                        image_rect: (0, 0, 400, 400),
                        offset: (0, 0),
                        zoom_factor: 100,
                        mask_label: "synthetic".into(),
                        minimized: false,
                    },
                    id: None,
                    pending: None,
                    error: None,
                    content_scale: 1.,
                    position: Rc::new(|_| Some((0, 0))),
                    minimized: Rc::new(|_| None),
                    bounds: Rc::new(|_, _| Ok(())),
                    pointer: Rc::new(|_, _| Ok(())),
                },
                window,
                cx,
            )
        });

        cx.simulate_resize(size(px(400.), px(400.)));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.update(|window, cx| window.dispatch_action(Box::new(super::Annotate), cx));
        cx.run_until_parked();
        pin.read_with(cx, |pin, _| assert!(pin.canvas.editing()));
        pin.update(cx, |pin, _| pin.dialog = true);
        cx.update(|window, cx| window.dispatch_action(Box::new(super::Minimize), cx));
        cx.run_until_parked();
        pin.read_with(cx, |pin, _| assert!(!pin.record.minimized));
    }

    #[test]
    fn full_menu_excludes_annotation_and_ocr_but_overflow_keeps_them() {
        use super::{COMMANDS, CONTEXT_COMMANDS, Command};
        assert_eq!(CONTEXT_COMMANDS, &COMMANDS[2..]);
        assert_eq!(&COMMANDS[..2], &[Command::Annotate, Command::Ocr]);
    }
}
