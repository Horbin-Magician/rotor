//! Verify synchronous native prepainting with a hidden window and synthetic pixels.
//! No desktop capture, profile, hotkeys or visible windows are used.
//! cargo run -p rotor-desktop --example capture_paint_smoke --locked
use gpui_kit::*;
use raw_window_handle::HasWindowHandle;
use std::{cell::Cell, process::ExitCode, rc::Rc, sync::Arc};

struct Frame {
    image: Arc<RenderImage>,
    generation: u32,
    painted: Rc<Cell<u32>>,
}

impl Render for Frame {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let generation = self.generation;
        let painted = self.painted.clone();
        div()
            .size_full()
            .child(img(self.image.clone()).size_full())
            .child(canvas(
                |_, _, _| (),
                move |_, _, _, _| painted.set(generation),
            ))
    }
}

fn main() -> ExitCode {
    let passed = Rc::new(Cell::new(false));
    let result = passed.clone();
    gpui_kit::application().run(move |cx| {
        let painted = Rc::new(Cell::new(u32::MAX));
        let observed = painted.clone();
        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(64.), px(64.)), cx)),
                    titlebar: None,
                    kind: WindowKind::PopUp,
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| Frame {
                        image: synthetic_image(0),
                        generation: 0,
                        painted,
                    })
                },
            )
            .expect("create hidden capture fixture");
        cx.spawn(async move |cx| {
            // Replace a painted placeholder twice. Each native call must paint
            // the new image before returning, without a display-link tick.
            for generation in 0..3 {
                let raw = handle
                    .update(cx, |view, window, _| {
                        view.image = synthetic_image(generation);
                        view.generation = generation;
                        window.refresh();
                        HasWindowHandle::window_handle(window).unwrap().as_raw()
                    })
                    .expect("update hidden fixture");
                if generation > 0 {
                    assert_eq!(observed.get(), generation - 1, "refresh alone is deferred");
                }
                rotor_platform::overlay::paint_hidden_window(raw).expect("prepaint hidden fixture");
                assert_eq!(
                    observed.get(),
                    generation,
                    "new frame must already be painted"
                );
            }
            passed.set(true);
            println!("Hidden capture frames painted synchronously: passed");
            cx.update(|cx| {
                handle
                    .update(cx, |_, window, _| window.remove_window())
                    .unwrap();
                cx.quit();
            });
        })
        .detach();
    });
    if result.get() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn synthetic_image(generation: u32) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(vec![image::Frame::new(
        image::RgbaImage::from_pixel(64, 64, image::Rgba([0, (generation * 100) as u8, 0, 255])),
    )]))
}
