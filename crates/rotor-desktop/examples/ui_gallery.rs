//! Open actual native views with a fresh temporary profile and indexing disabled.
//! Usage: cargo run -p rotor-desktop --example ui_gallery --release -- search|translation|pin
//! A bundled macOS launch can select the page with ROTOR_UI_GALLERY_PAGE.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use gpui_kit::{
    component::{Root, Theme, ThemeMode},
    *,
};
use rotor_common::{ConfigService, ResourceLocator, file_path};
use rotor_runtime::{ServiceOptions, Services};
use std::sync::{Arc, Mutex};

enum Preview {
    Search(WeakEntity<rotor_ui::SearchView>),
    Translation(WeakEntity<rotor_ui::TranslatorView>),
    Pin(WeakEntity<rotor_ui::PinView>),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let page = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("ROTOR_UI_GALLERY_PAGE").ok())
        .unwrap_or_else(|| "search".into());
    if !matches!(page.as_str(), "search" | "translation" | "pin") {
        return Err("Choose search, translation or pin".into());
    }
    let profile = tempfile::Builder::new()
        .prefix("rotor-ui-gallery-")
        .tempdir()?;
    file_path::initialize_data_directory(profile.path().to_path_buf())?;
    let mut config = ConfigService::load_from(profile.path())?;
    config.set_many([
        ("language".into(), "1".into()),
        ("theme".into(), "2".into()),
    ])?;
    let resources = ResourceLocator::from_root(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"),
    )?;
    let (services, events) = Services::new(
        Arc::new(Mutex::new(config)),
        Some(resources),
        ServiceOptions { index_files: false },
    )?;
    let services = Arc::new(services);
    let views = services.clone();
    gpui_kit::application()
        .with_assets(rotor_ui::UiAssets)
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(move |cx| {
            gpui_kit::init(cx);
            rotor_ui::configure_theme(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            let dimensions = if page == "search" {
                size(px(500.), px(50.))
            } else if page == "pin" {
                size(px(300.), px(200.))
            } else {
                size(px(392.), px(420.))
            };
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(dimensions, cx)),
                titlebar: None,
                app_owns_titlebar_drag: page == "pin",
                // Pins must exercise AppKit's real popup/titlebar hit testing.
                kind: if page == "pin" {
                    WindowKind::PopUp
                } else {
                    WindowKind::Normal
                },
                window_min_size: Some(size(px(1.), px(1.))),
                is_resizable: false,
                ..Default::default()
            };
            let mut preview = None;
            let handle = cx
                .open_window(options, |window, cx| {
                    window.set_window_title(&format!("Rotor · {page} · synthetic preview"));
                    if page == "search" {
                        let view = cx.new(|cx| rotor_ui::SearchView::new(views, window, cx));
                        preview = Some(Preview::Search(view.downgrade()));
                        cx.new(|cx| Root::new(view, window, cx))
                    } else if page == "pin" {
                        let view = cx.new(|cx| synthetic_pin(views, window, cx));
                        preview = Some(Preview::Pin(view.downgrade()));
                        cx.new(|cx| Root::new(view, window, cx))
                    } else {
                        let view = cx.new(|cx| rotor_ui::TranslatorView::new(views, window, cx));
                        preview = Some(Preview::Translation(view.downgrade()));
                        cx.new(|cx| Root::new(view, window, cx))
                    }
                })
                .expect("open native preview");
            let preview = preview.expect("created preview view");
            cx.spawn(async move |cx| {
                while let Ok(event) = events.recv().await {
                    if handle
                        .update(cx, |_, window, cx| match &preview {
                            Preview::Search(view) => {
                                let _ = view
                                    .update(cx, |view, cx| view.handle_event(&event, window, cx));
                            }
                            Preview::Pin(view) => {
                                let _ = view
                                    .update(cx, |view, cx| view.handle_event(&event, window, cx));
                            }
                            Preview::Translation(view) => {
                                let _ = view
                                    .update(cx, |view, cx| view.handle_event(&event, window, cx));
                            }
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
        });
    services.shutdown();
    Ok(())
}

fn synthetic_pin(
    services: Arc<Services>,
    window: &mut Window,
    cx: &mut Context<rotor_ui::PinView>,
) -> rotor_ui::PinView {
    use raw_window_handle::HasWindowHandle;
    use std::rc::Rc;
    let image = image::RgbaImage::from_fn(800, 600, |x, y| {
        if x % 40 < 2 || y % 40 < 2 {
            image::Rgba([20, 50, 90, 255])
        } else {
            image::Rgba([40 + (x / 4) as u8, 80 + (y / 4) as u8, 200, 255])
        }
    });
    #[cfg(target_os = "macos")]
    rotor_platform::overlay::enable_pin_minimization(
        HasWindowHandle::window_handle(window).unwrap(),
    )
    .unwrap();
    rotor_ui::PinView::new(
        services,
        rotor_ui::PinInit {
            image: rotor_ui::prepare_image(Arc::new(image)).unwrap(),
            config: rotor_runtime::ShotterConfig {
                annotations: Vec::new(),
                monitor_pos: (0, 0),
                monitor_size: (800, 600),
                rect: (100, 100, 600, 400),
                image_rect: (0, 0, 800, 600),
                offset: (0, 0),
                zoom_factor: 100,
                mask_label: "synthetic".into(),
                minimized: false,
            },
            id: None,
            pending: None,
            error: None,
            content_scale: 2.,
            position: Rc::new(|window| {
                #[cfg(target_os = "windows")]
                {
                    rotor_platform::overlay::client_origin(
                        HasWindowHandle::window_handle(window).ok()?,
                    )
                    .ok()
                }
                #[cfg(not(target_os = "windows"))]
                {
                    let origin = window.inner_window_bounds().get_bounds().origin;
                    let scale = window.scale_factor();
                    Some((
                        (origin.x.as_f32() * scale).round() as i32,
                        (origin.y.as_f32() * scale).round() as i32,
                    ))
                }
            }),
            minimized: Rc::new(|_| None),
            bounds: Rc::new(|window, bounds| {
                rotor_platform::overlay::set_client_bounds(
                    HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?,
                    bounds.x,
                    bounds.y,
                    bounds.width,
                    bounds.height,
                    window.scale_factor(),
                )
            }),
            pointer: Rc::new(|window, capture| {
                rotor_platform::overlay::pointer_capture(
                    HasWindowHandle::window_handle(window).map_err(|e| e.to_string())?,
                    capture,
                )
            }),
            cursor: Rc::new(|window| {
                rotor_platform::overlay::screen_cursor_position(window.scale_factor())
            }),
        },
        window,
        cx,
    )
}
