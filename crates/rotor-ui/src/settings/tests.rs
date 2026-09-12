use super::{Section, SettingsView};
use gpui::AppContext;
use gpui_kit::component::Root;
use rotor_common::ConfigService;
use rotor_runtime::{ServiceOptions, Services};
use std::sync::{Arc, Mutex};

#[test]
fn settings_pages_render_with_windows_main_thread_stack() {
    // The Windows executable reserves 1 MiB for its main thread. Rust's test
    // threads normally have more room and can hide oversized debug render frames.
    std::thread::Builder::new()
        .name("settings-main-stack".into())
        .stack_size(1024 * 1024)
        .spawn(|| {
            let profile = tempfile::tempdir().unwrap();
            let config = ConfigService::load_from(profile.path()).unwrap();
            let (services, _events) = Services::new(
                Arc::new(Mutex::new(config)),
                None,
                ServiceOptions { index_files: false },
            )
            .unwrap();
            let services = Arc::new(services);
            let mut cx = gpui::TestAppContext::single();
            cx.update(gpui_kit::component::init);
            let mut settings = None;
            let (_, cx) = cx.add_window_view(|window, cx| {
                let view = cx
                    .new(|cx| SettingsView::new(services.settings(), services.clone(), window, cx));
                settings = Some(view.clone());
                Root::new(view, window, cx)
            });
            let view = settings.unwrap();
            cx.update(|window, cx| window.draw(cx).clear(cx));
            let refresh_bounds = cx.debug_bounds("overview-refresh-content").unwrap();
            assert_eq!(
                refresh_bounds.size,
                gpui::size(gpui::px(24.), gpui::px(24.))
            );
            view.update(cx, |view, cx| {
                view.overview_refresh_started = Some(std::time::Instant::now());
                cx.notify();
            });
            cx.update(|window, cx| window.draw(cx).clear(cx));
            for _ in 0..3 {
                for section in [
                    Section::General,
                    Section::AiProvider,
                    Section::Overview,
                    Section::Pin,
                    Section::Search,
                    Section::Translation,
                    Section::Quick,
                ] {
                    view.update(cx, |view, cx| {
                        view.section = section;
                        cx.notify();
                    });
                    cx.update(|window, cx| window.draw(cx).clear(cx));
                }
            }
            for provider in rotor_common::ai_provider::PROVIDERS {
                view.update(cx, |view, cx| {
                    view.section = Section::AiProvider;
                    view.config.insert("ai_provider".into(), (*provider).into());
                    let visible: Vec<_> = view
                        .fields
                        .iter()
                        .filter(|field| view.field_visible(field))
                        .collect();
                    assert_eq!(visible.len(), 4);
                    assert!(
                        visible
                            .iter()
                            .all(|field| field.key.starts_with(&format!("ai_{provider}_")))
                    );
                    cx.notify();
                });
                cx.update(|window, cx| window.draw(cx).clear(cx));
            }
            view.update(cx, |view, cx| view.check_ai_test_result_identity(cx));
            for engine in ["google", "ai", "deepseek", "custom"] {
                view.update(cx, |view, cx| {
                    view.section = Section::Translation;
                    view.config
                        .insert("translator_engine".into(), engine.into());
                    cx.notify();
                });
                cx.update(|window, cx| window.draw(cx).clear(cx));
            }
            cx.update(|window, cx| {
                view.update(cx, |view, cx| {
                    view.section = Section::Quick;
                    view.actions.push(super::actions::ActionFields::new(
                        rotor_runtime::QuickAction {
                            id: "synthetic-action".into(),
                            name: "Synthetic".into(),
                            shortcut: "shift+control+KeyA".into(),
                            command: "echo synthetic".into(),
                            enabled: true,
                        },
                        window,
                        cx,
                    ));
                    view.editing_action = Some("synthetic-action".into());
                    cx.notify();
                });
            });
            cx.update(|window, cx| window.draw(cx).clear(cx));
            // Exercise populated rows as well as expanded editors. The header
            // and row builders share the same limited Windows render stack.
            for editing in [false, true, false] {
                view.update(cx, |view, cx| {
                    view.editing_action = editing.then(|| "synthetic-action".into());
                    cx.notify();
                });
                cx.update(|window, cx| window.draw(cx).clear(cx));
            }
            services.shutdown();
        })
        .unwrap()
        .join()
        .unwrap();
}
