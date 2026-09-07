mod fixture;
#[cfg(target_os = "windows")]
mod platform;
mod services;

use anyhow::Result;
use gpui_kit::{
    component::{
        Root,
        button::Button,
        input::{Input, InputState},
    },
    prelude::*,
    *,
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct State {
    image: Arc<RenderImage>,
    png: Arc<Vec<u8>>,
    ordinary: Option<AnyWindowHandle>,
    masks: Vec<AnyWindowHandle>,
    _services: services::Services,
}
impl Global for State {}

struct Probe {
    input: Entity<InputState>,
    label: String,
    overlay: bool,
    status: String,
}

impl Render for Probe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let image = cx.global::<State>().image.clone();
        let scale = window.scale_factor();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .size_full()
            .bg(if self.overlay {
                rgba(0x17203388)
            } else {
                rgba(0x172033ff)
            })
            .text_color(rgb(0xffffff))
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
                    .child(format!(
                        "{} · DPI {} · {:?}",
                        self.label,
                        window.scale_factor(),
                        window.bounds()
                    )),
            )
            .child(Input::new(&self.input))
            .child(img(image).w(px(640. / scale)).h(px(240. / scale)))
            .child(
                div()
                    .pl(px(16. / scale))
                    .font_family(if cfg!(target_os = "windows") {
                        "Microsoft YaHei"
                    } else {
                        "PingFang SC"
                    })
                    .text_size(px(28. / scale))
                    .child(fixture::TEXT),
            )
            .child("物理像素 1:1 · PNG 640×240；上图离屏文字，下行 GPUI 文字。检查颜色、alpha 与 1px 线条。")
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(Button::new("copy").label("复制 PNG").on_click(cx.listener(
                        |this, _, _, cx| {
                            this.status = match copy_png(cx) {
                                Ok(()) => "系统剪贴板图像回读一致：640×240 RGBA；外部应用粘贴仍需核验".into(),
                                Err(error) => format!("剪贴板验证失败：{error:#}"),
                            };
                            cx.notify();
                        },
                    )))
                    .child(Button::new("save").label("保存 PNG…").on_click(cx.listener(
                        |this, _, window, cx| {
                            let bytes = cx.global::<State>().png.clone();
                            let receiver = cx.prompt_for_new_path(
                                &std::env::temp_dir(),
                                Some("Rotor 中文 P0.png"),
                            );
                            this.status = "等待原生保存框".into();
                            cx.spawn_in(window, async move |view, cx| {
                                let result = async {
                                    let Some(path) = receiver.await?? else {
                                        return Ok::<_, anyhow::Error>(
                                            "已取消；窗口保留".to_string(),
                                        );
                                    };
                                    let saved = path.clone();
                                    cx.background_executor()
                                        .spawn(
                                            async move { std::fs::write(&saved, bytes.as_slice()) },
                                        )
                                        .await?;
                                    Ok(format!("已保存 {}", path.display()))
                                }
                                .await;
                                let _ = view.update(cx, |this, cx| {
                                    this.status = result
                                        .unwrap_or_else(|error| format!("保存失败：{error:#}"));
                                    cx.notify();
                                });
                            })
                            .detach();
                        },
                    )))
                    .child(
                        Button::new("masks")
                            .label("每屏遮罩")
                            .on_click(|_, _, cx| report(open_masks(cx))),
                    )
                    .child(
                        Button::new("close")
                            .label("关闭窗口")
                            .on_click(|_, window, _| window.remove_window()),
                    )
                    .child(
                        Button::new("quit")
                            .label("退出原型")
                            .on_click(|_, _, cx| cx.quit()),
                    ),
            )
            .child(self.status.clone())
    }
}

fn open(
    cx: &mut App,
    label: String,
    bounds: Bounds<Pixels>,
    display: Option<DisplayId>,
    overlay: bool,
) -> Result<AnyWindowHandle> {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        display_id: display,
        titlebar: if overlay {
            None
        } else {
            Some(TitlebarOptions {
                title: Some(label.clone().into()),
                ..Default::default()
            })
        },
        kind: if overlay {
            WindowKind::PopUp
        } else {
            WindowKind::Normal
        },
        window_background: if overlay {
            WindowBackgroundAppearance::Transparent
        } else {
            WindowBackgroundAppearance::Opaque
        },
        app_id: Some("cc.fluctus.rotor.gpui-probe".into()),
        is_resizable: !overlay,
        is_minimizable: !overlay,
        ..Default::default()
    };
    Ok(cx
        .open_window(options, |window, cx| {
            window.set_window_title(&label);
            let view = cx.new(|cx| Probe {
                input: cx.new(|cx| {
                    InputState::new(window, cx).placeholder("中文 IME：组合、选词、撤销、粘贴")
                }),
                label,
                overlay,
                status: "Ctrl+Alt+Shift+G 唤起普通窗口；关闭全部窗口后通过托盘恢复".into(),
            });
            cx.new(|cx| Root::new(view, window, cx).bg(rgba(0x00000000)))
        })?
        .into())
}

fn show(cx: &mut App) -> Result<()> {
    if let Some(handle) = cx.global::<State>().ordinary
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return Ok(());
    }
    let bounds = Bounds::centered(None, size(px(820.), px(560.)), cx);
    let handle = open(cx, "Rotor GPUI P0".into(), bounds, None, false)?;
    cx.global_mut::<State>().ordinary = Some(handle);
    Ok(())
}

fn open_masks(cx: &mut App) -> Result<()> {
    for handle in std::mem::take(&mut cx.global_mut::<State>().masks) {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    }
    for display in cx.displays() {
        let bounds = display.bounds();
        eprintln!("display {:?}: {bounds:?}", display.id());
        let handle = open(
            cx,
            format!("Mask {:?}", display.id()),
            bounds,
            Some(display.id()),
            true,
        )?;
        #[cfg(target_os = "windows")]
        handle.update(cx, |_, window, _| platform::fit_mask_to_monitor(window))??;
        cx.global_mut::<State>().masks.push(handle);
    }
    Ok(())
}

fn report(result: Result<()>) {
    if let Err(error) = result {
        eprintln!("P0 operation failed: {error:#}");
    }
}

fn copy_png(cx: &mut App) -> Result<()> {
    let bytes = cx.global::<State>().png.as_ref().clone();
    let expected = image::load_from_memory(&bytes)?.into_rgba8();
    cx.write_to_clipboard(ClipboardItem::new_image(&Image::from_bytes(
        ImageFormat::Png,
        bytes,
    )));
    let clipboard = cx
        .read_from_clipboard()
        .ok_or_else(|| anyhow::anyhow!("clipboard unavailable"))?;
    let image = clipboard
        .into_entries()
        .find_map(|entry| match entry {
            ClipboardEntry::Image(image) => Some(image),
            _ => None,
        })
        .ok_or_else(|| anyhow::anyhow!("clipboard did not return an image"))?;
    let actual = image::load_from_memory(&image.bytes)?.into_rgba8();
    anyhow::ensure!(
        actual == expected,
        "clipboard image pixels differ from PNG source"
    );
    Ok(())
}

fn run() -> Result<()> {
    let rgba = fixture::annotated()?;
    let png = Arc::new(fixture::png(&rgba)?);
    let window_only = std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--window-only");
    if let Some(path) = std::env::args_os().nth(1)
        && !window_only
    {
        anyhow::ensure!(
            path == "--export-fixture",
            "usage: rotor-gpui-probe [--window-only | --export-fixture OUTPUT.png]"
        );
        let output = PathBuf::from(
            std::env::args_os()
                .nth(2)
                .ok_or_else(|| anyhow::anyhow!("missing PNG output path"))?,
        );
        std::fs::write(output, png.as_slice())?;
        return Ok(());
    }
    let image = Arc::new(RenderImage::new(vec![image::Frame::new(fixture::bgra(
        &rgba,
    ))]));
    let startup_failed = Arc::new(AtomicBool::new(false));
    let failure = startup_failed.clone();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .with_quit_mode(QuitMode::Explicit)
        .run(move |cx| {
            gpui_kit::init(cx);
            gpui_kit::component::Theme::change(gpui_kit::component::ThemeMode::Dark, None, cx);
            let (tx, rx) = async_channel::bounded(16);
            let quitting = Arc::new(AtomicBool::new(false));
            let services = match services::Services::new(tx, quitting.clone()) {
                Ok(services) => services,
                Err(error) => {
                    eprintln!("P0 system service startup failed: {error:#}");
                    failure.store(true, Ordering::Relaxed);
                    cx.quit();
                    return;
                }
            };
            cx.set_global(State {
                image,
                png,
                ordinary: None,
                masks: vec![],
                _services: services,
            });
            if let Err(error) = show(cx) {
                eprintln!("P0 ordinary window failed: {error:#}");
                failure.store(true, Ordering::Relaxed);
                cx.quit();
                return;
            }
            if !window_only {
                for i in 0..2 {
                    let bounds = Bounds::new(
                        point(px(80. + i as f32 * 100.), px(100. + i as f32 * 100.)),
                        size(px(820.), px(540.)),
                    );
                    if let Err(error) =
                        open(cx, format!("Transparent pin {}", i + 1), bounds, None, true)
                    {
                        eprintln!("pin failed: {error:#}");
                        failure.store(true, Ordering::Relaxed);
                    }
                }
                if let Err(error) = open_masks(cx) {
                    eprintln!("P0 mask failed: {error:#}");
                    failure.store(true, Ordering::Relaxed);
                }
            }
            cx.spawn(async move |cx| {
                while let Ok(event) = rx.recv().await {
                    let event = if quitting.load(Ordering::Relaxed) {
                        services::Event::Quit
                    } else {
                        event
                    };
                    cx.update(|cx| match event {
                        services::Event::Show => report(show(cx)),
                        services::Event::Masks => report(open_masks(cx)),
                        services::Event::Quit => cx.quit(),
                    });
                    if matches!(event, services::Event::Quit) {
                        break;
                    }
                }
            })
            .detach();
        });
    anyhow::ensure!(
        !startup_failed.load(Ordering::Relaxed),
        "P0 startup was incomplete; see diagnostics above"
    );
    Ok(())
}

fn main() {
    // Keep GPU/startup diagnostics available before any window can be opened.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("Rotor GPUI P0 panic: {info}\nCheck GPU driver and retain this log.")
    }));
    if let Err(error) = run() {
        eprintln!("Rotor GPUI P0 failed: {error:#}");
        std::process::exit(1);
    }
}
