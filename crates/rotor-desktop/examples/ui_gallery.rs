//! Open actual native views with a fresh temporary profile and indexing disabled.
//! Usage: cargo run -p rotor-desktop --example ui_gallery --release -- search|translation
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let page = std::env::args().nth(1).unwrap_or_else(|| "search".into());
    if !matches!(page.as_str(), "search" | "translation") {
        return Err("Choose search or translation".into());
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
            } else {
                size(px(392.), px(420.))
            };
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::centered(dimensions, cx)),
                titlebar: None,
                // A normal host keeps popup views targetable by screenshot tools.
                // The application itself retains its popup window lifecycle.
                kind: WindowKind::Normal,
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
