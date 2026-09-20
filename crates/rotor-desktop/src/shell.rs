//! Shell state, the window registry and the window factories.
//!
//! Every window the shell opens is recorded here by role so runtime events
//! and commands can reach its view without downcasting at each call site.
use crate::{
    capture, pins, placement,
    system::{CommandBus, SystemServices},
};
use gpui_kit::{
    component::{Root, Theme, ThemeMode},
    *,
};
use rotor_common::{Config, Settings};
use rotor_runtime::{OperationId, Services};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WindowRole {
    Settings,
    Translator,
    Search,
    Mask { session: u64, monitor: u32 },
    Pin(u64),
}

pub(crate) enum WindowView {
    Settings(WeakEntity<rotor_ui::SettingsView>),
    Translator(WeakEntity<rotor_ui::TranslatorView>),
    Search(WeakEntity<rotor_ui::SearchView>),
    Mask(WeakEntity<rotor_ui::MaskView>),
    Pin(WeakEntity<rotor_ui::PinView>),
}

pub(crate) struct WindowSlot {
    pub(crate) window: AnyWindowHandle,
    pub(crate) view: WindowView,
    pub(crate) _appearance: Option<Subscription>,
}

pub(crate) struct ShellState {
    pub(crate) windows: HashMap<WindowRole, WindowSlot>,
    pub(crate) config: Config,
    pub(crate) services: Arc<Services>,
    pub(crate) commands: CommandBus,
    pub(crate) system: SystemServices,
    pub(crate) _task: Option<Task<()>>,
    pub(crate) _closed: Option<Subscription>,
    pub(crate) _quit: Option<Subscription>,
    pub(crate) pending_selection: Option<OperationId>,
    pub(crate) capture: capture::CaptureState,
    pub(crate) pins: pins::PinWindows,
    pub(crate) monitors: Vec<rotor_runtime::MonitorConfig>,
    pub(crate) index_rebuild: Option<Task<()>>,
}
impl Global for ShellState {}

impl ShellState {
    pub(crate) fn window(&self, role: WindowRole) -> Option<AnyWindowHandle> {
        self.windows.get(&role).map(|slot| slot.window)
    }

    pub(crate) fn settings(&self) -> Option<(AnyWindowHandle, WeakEntity<rotor_ui::SettingsView>)> {
        self.windows
            .get(&WindowRole::Settings)
            .and_then(|slot| match &slot.view {
                WindowView::Settings(view) => Some((slot.window, view.clone())),
                _ => None,
            })
    }

    pub(crate) fn translator(
        &self,
    ) -> Option<(AnyWindowHandle, WeakEntity<rotor_ui::TranslatorView>)> {
        self.windows
            .get(&WindowRole::Translator)
            .and_then(|slot| match &slot.view {
                WindowView::Translator(view) => Some((slot.window, view.clone())),
                _ => None,
            })
    }

    pub(crate) fn search(&self) -> Option<(AnyWindowHandle, WeakEntity<rotor_ui::SearchView>)> {
        self.windows
            .get(&WindowRole::Search)
            .and_then(|slot| match &slot.view {
                WindowView::Search(view) => Some((slot.window, view.clone())),
                _ => None,
            })
    }

    pub(crate) fn register(
        &mut self,
        role: WindowRole,
        window: &Window,
        view: WindowView,
        appearance: Option<Subscription>,
    ) {
        self.windows.insert(
            role,
            WindowSlot {
                window: window.window_handle(),
                view,
                _appearance: appearance,
            },
        );
    }
}

pub(crate) fn quit_in_progress(cx: &App) -> bool {
    cx.global::<ShellState>()
        .settings()
        .and_then(|(_, view)| view.upgrade())
        .is_some_and(|view| view.read(cx).waiting_to_quit())
}

pub(crate) fn request_quit(cx: &mut App) {
    if let Some((handle, view)) = cx.global::<ShellState>().settings()
        && handle
            .update(cx, |_, window, cx| {
                view.update(cx, |view, cx| view.request_quit(window, cx))
            })
            .is_ok_and(|result| result.is_ok())
    {
        return;
    }
    cx.quit();
}

/// Retain background warnings for the next settings open and update an already
/// open settings view without taking focus away from the user's current app.
pub(crate) fn publish_warning(message: String, cx: &mut App) {
    let view = {
        let state = cx.global_mut::<ShellState>();
        state.system.warning = Some(message.clone());
        state.settings().map(|(_, view)| view)
    };
    if let Some(view) = view {
        let _ = view.update(cx, |view, cx| view.show_message(message, cx));
    }
}

pub(crate) fn apply_theme(config: &Config, cx: &mut App) {
    match config.theme() {
        rotor_common::Theme::Light => Theme::change(ThemeMode::Light, None, cx),
        rotor_common::Theme::Dark => Theme::change(ThemeMode::Dark, None, cx),
        rotor_common::Theme::System => Theme::sync_system_appearance(None, cx),
    }
}

/// Windows that follow the system re-sync when the OS appearance changes.
pub(crate) fn follows_system_theme(cx: &App) -> bool {
    cx.global::<ShellState>().config.theme() == rotor_common::Theme::System
}

pub(crate) fn show_settings(cx: &mut App) -> Result<(), String> {
    if let Some((view, warning)) = cx.global::<ShellState>().settings().and_then(|(_, view)| {
        cx.global::<ShellState>()
            .system
            .warning
            .clone()
            .map(|warning| (view, warning))
    }) {
        let _ = view.update(cx, |view, cx| view.show_message(warning, cx));
    }
    if let Some(handle) = cx.global::<ShellState>().window(WindowRole::Settings)
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return Ok(());
    }
    let state = cx.global::<ShellState>();
    let config = state.config.clone();
    let services = state.services.clone();
    let warning = state.system.warning.clone();
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(500.), px(400.)), cx)),
        window_min_size: Some(size(px(500.), px(400.))),
        titlebar: Some(TitlebarOptions {
            title: Some(rotor_ui::settings_title(&config).into()),
            appears_transparent: cfg!(any(target_os = "windows", target_os = "macos")),
            // Keep native AppKit controls in the reserved strip above the sidebar logo.
            traffic_light_position: cfg!(target_os = "macos").then_some(point(px(18.), px(12.))),
        }),
        app_id: Some(rotor_common::native_app::IDENTIFIER.into()),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| {
        let appearance = window.observe_window_appearance(|window, cx| {
            if follows_system_theme(cx) {
                Theme::sync_system_appearance(Some(window), cx);
            }
        });
        let view = cx.new(|cx| rotor_ui::SettingsView::new(config, services, window, cx));
        let closing = view.downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            closing
                .update(cx, |view, cx| view.request_close(window, cx))
                .is_err()
        });
        if let Some(warning) = warning {
            view.update(cx, |view, cx| view.show_message(warning, cx));
        }
        cx.global_mut::<ShellState>().register(
            WindowRole::Settings,
            window,
            WindowView::Settings(view.downgrade()),
            Some(appearance),
        );
        cx.new(|cx| Root::new(view, window, cx))
    })
    .map_err(|error| error.to_string())?;
    if let Err(error) = rotor_platform::desktop::set_dock_visible(true) {
        log::warn!("Application policy: {error}");
    }
    #[cfg(target_os = "macos")]
    {
        // Apply the icon after the accessory-to-regular transition creates the Dock tile.
        if let Err(error) = rotor_platform::desktop::set_application_icon(include_bytes!(
            "../../../assets/icons/icon.icns"
        )) {
            log::warn!("Application icon: {error}");
        }
        cx.activate(true);
    }
    Ok(())
}

pub(crate) fn show_translator(cx: &mut App) -> Result<(), String> {
    if let Some((handle, view)) = cx.global::<ShellState>().translator()
        && handle
            .update(cx, |_, window, cx| {
                window.activate_window();
                let _ = view.update(cx, |view, cx| view.begin_input(window, cx));
            })
            .is_ok()
    {
        return Ok(());
    }
    let services = cx.global::<ShellState>().services.clone();
    cx.open_window(
        placement::utility_options(size(px(392.), px(420.)), cx),
        |window, cx| {
            let appearance = window.observe_window_appearance(|window, cx| {
                if follows_system_theme(cx) {
                    Theme::sync_system_appearance(Some(window), cx);
                }
            });
            let view = cx.new(|cx| rotor_ui::TranslatorView::new(services, window, cx));
            cx.global_mut::<ShellState>().register(
                WindowRole::Translator,
                window,
                WindowView::Translator(view.downgrade()),
                Some(appearance),
            );
            cx.new(|cx| Root::new(view, window, cx))
        },
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn show_search(cx: &mut App) -> Result<(), String> {
    // macOS PopUp windows are nonactivating NSPanels. Making the panel key
    // alone can leave the input method attached to the previously active app.
    // Activate Rotor before both opening and reusing the text-entry window.
    #[cfg(target_os = "macos")]
    cx.activate(true);

    if let Some(handle) = cx.global::<ShellState>().window(WindowRole::Search)
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return Ok(());
    }
    let services = cx.global::<ShellState>().services.clone();
    cx.open_window(placement::search_options(cx), |window, cx| {
        if let Err(error) = raw_window_handle::HasWindowHandle::window_handle(window)
            .map_err(|error| error.to_string())
            .and_then(rotor_platform::overlay::configure_text_entry_panel)
        {
            log::warn!("Failed to configure search panel level: {error}");
        }
        let appearance = window.observe_window_appearance(|window, cx| {
            if follows_system_theme(cx) {
                Theme::sync_system_appearance(Some(window), cx);
            }
        });
        let view = cx.new(|cx| rotor_ui::SearchView::new(services, window, cx));
        cx.global_mut::<ShellState>().register(
            WindowRole::Search,
            window,
            WindowView::Search(view.downgrade()),
            Some(appearance),
        );
        cx.new(|cx| Root::new(view, window, cx))
    })
    .map_err(|error| error.to_string())?;
    Ok(())
}
