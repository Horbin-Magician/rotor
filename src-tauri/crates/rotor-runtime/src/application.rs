use std::{
    collections::{HashMap, HashSet},
    sync::{mpsc, LazyLock, Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
};

use rotor_screenshot::ScreenShotter;
use rotor_searcher::{file_data::SearchResultItem, Searcher};
use rotor_translator::Translator;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

use crate::quick::Quick;
use crate::tray::Tray;

const SHORTCUT_TRIGGER_DEBOUNCE: Duration = Duration::from_millis(500);
const PRESSED_SHORTCUT_STALE_AFTER: Duration = Duration::from_secs(3);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutRegistrationNotice {
    pub key: String,
    pub shortcut: String,
    pub message: String,
}

#[derive(Clone, Copy)]
struct GlobalHotkeyDispatch {
    shortcut: Shortcut,
    state: ShortcutState,
}

pub fn handle_global_hotkey_event(_app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    let dispatch = GlobalHotkeyDispatch {
        shortcut: *shortcut,
        state: event.state(),
    };

    if GLOBAL_HOTKEY_DISPATCHER.send(dispatch).is_err() {
        log::error!("Global shortcut dispatcher is unavailable");
    }
}

fn dispatch_global_hotkey_event(dispatch: GlobalHotkeyDispatch) {
    let mut rotor_app = Application::lock_global();

    let shortcut = dispatch.shortcut;
    let shortcut_id = shortcut.id();
    if dispatch.state == ShortcutState::Released {
        rotor_app.pressed_shortcuts.remove(&shortcut_id);
        return;
    }

    if dispatch.state == ShortcutState::Pressed {
        if rotor_app.should_ignore_shortcut_press(shortcut_id, &shortcut) {
            return;
        }

        let mut handled = false;
        if rotor_app.screenshot_shortcut == Some(shortcut) {
            let result = rotor_app.screenshot.prepare_screenshot_session();
            rotor_app.finish_shortcut_trigger(shortcut_id);
            let flag = rotor_app.screenshot.flag().to_string();
            drop(rotor_app);

            match result {
                Ok(session) => session.capture_and_show_async(),
                Err(e) => log::error!("Module {flag} run error: {e}"),
            }
            return;
        }

        if rotor_app.search_shortcut == Some(shortcut) {
            let result = rotor_app.searcher.run();
            rotor_app.finish_shortcut_trigger(shortcut_id);
            result.unwrap_or_else(|e| {
                let flag = rotor_app.searcher.flag();
                log::error!("Module {flag} run error: {e}")
            });
            handled = true;
        }

        if !handled && rotor_app.translate_select_shortcut == Some(shortcut) {
            let result = rotor_app.translator.run_select();
            rotor_app.finish_shortcut_trigger(shortcut_id);
            result.unwrap_or_else(|e| {
                let flag = rotor_app.translator.flag();
                log::error!("Module {flag} run error: {e}")
            });
            handled = true;
        }

        if !handled && rotor_app.translate_input_shortcut == Some(shortcut) {
            let result = rotor_app.translator.run_input();
            rotor_app.finish_shortcut_trigger(shortcut_id);
            result.unwrap_or_else(|e| {
                let flag = rotor_app.translator.flag();
                log::error!("Module {flag} run error: {e}")
            });
            handled = true;
        }

        if !handled {
            match rotor_app.quick.run_by_shortcut(&shortcut) {
                Ok(true) => {
                    rotor_app.finish_shortcut_trigger(shortcut_id);
                    handled = true;
                }
                Ok(false) => {}
                Err(error) => {
                    let flag = rotor_app.quick.flag();
                    log::error!("Module {flag} run error: {error}");
                    rotor_app.finish_shortcut_trigger(shortcut_id);
                    handled = true;
                }
            }
        }

        if !handled {
            rotor_app.pressed_shortcuts.remove(&shortcut_id);
        }
    }
}

pub struct Application {
    pub app: Option<AppHandle>,
    pub tray: Tray,
    pub screenshot: ScreenShotter,
    pub searcher: Searcher,
    pub quick: Quick,
    pub translator: Translator,
    screenshot_shortcut: Option<Shortcut>,
    search_shortcut: Option<Shortcut>,
    translate_select_shortcut: Option<Shortcut>,
    translate_input_shortcut: Option<Shortcut>,
    pressed_shortcuts: HashSet<u32>,
    last_shortcut_triggers: HashMap<u32, Instant>,
    shortcut_registration_notices: Vec<ShortcutRegistrationNotice>,
}

impl Application {
    fn new() -> Application {
        Application {
            app: None,
            tray: Tray::new(),
            screenshot: ScreenShotter::new(),
            searcher: Searcher::new(
                Application::update_search_result,
                Some(Box::new(Application::update_search_index_state)),
            ),
            quick: Quick::new(),
            translator: Translator::new(),
            screenshot_shortcut: None,
            search_shortcut: None,
            translate_select_shortcut: None,
            translate_input_shortcut: None,
            pressed_shortcuts: HashSet::new(),
            last_shortcut_triggers: HashMap::new(),
            shortcut_registration_notices: Vec::new(),
        }
    }

    pub fn global() -> &'static Mutex<Application> {
        &INSTANCE
    }

    pub fn lock_global() -> MutexGuard<'static, Application> {
        INSTANCE.lock().unwrap_or_else(|poisoned| {
            log::error!("Application lock poisoned; recovering inner state");
            poisoned.into_inner()
        })
    }

    pub fn init(&mut self, app: tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        if let Err(e) = self.tray.init(&app) {
            let flag = self.tray.flag();
            log::error!("Module {flag} init error: {e}");
        }

        if let Err(e) = self.screenshot.init(&app) {
            let flag = self.screenshot.flag();
            log::error!("Module {flag} init error: {e}");
        }
        self.screenshot_shortcut = self.screenshot.get_shortcut();
        if let Some(shortcut) = self.screenshot_shortcut {
            if let Err(e) = app.global_shortcut().register(shortcut) {
                let flag = self.screenshot.flag();
                log::error!("Module {flag} shortcut registration error: {e}");
                self.notify_shortcut_registration_error("shortcut_screenshot", shortcut, e);
            }
        }

        if let Err(e) = self.searcher.init(&app) {
            let flag = self.searcher.flag();
            log::error!("Module {flag} init error: {e}");
        }
        self.search_shortcut = self.searcher.get_shortcut();
        if let Some(shortcut) = self.search_shortcut {
            if let Err(e) = app.global_shortcut().register(shortcut) {
                let flag = self.searcher.flag();
                log::error!("Module {flag} shortcut registration error: {e}");
                self.notify_shortcut_registration_error("shortcut_search", shortcut, e);
            }
        }

        if let Err(e) = self.translator.init(&app) {
            let flag = self.translator.flag();
            log::error!("Module {flag} init error: {e}");
        }
        self.translate_select_shortcut = self.translator.get_select_shortcut();
        if let Some(shortcut) = self.translate_select_shortcut {
            if let Err(e) = app.global_shortcut().register(shortcut) {
                let flag = self.translator.flag();
                log::error!("Module {flag} shortcut registration error: {e}");
                self.notify_shortcut_registration_error("shortcut_translate_select", shortcut, e);
            }
        }
        self.translate_input_shortcut = self.translator.get_input_shortcut();
        if let Some(shortcut) = self.translate_input_shortcut {
            if let Err(e) = app.global_shortcut().register(shortcut) {
                let flag = self.translator.flag();
                log::error!("Module {flag} shortcut registration error: {e}");
                self.notify_shortcut_registration_error("shortcut_translate_input", shortcut, e);
            }
        }

        self.quick.reload();
        for (action_id, shortcut) in self.quick.get_shortcuts() {
            if let Err(e) = app.global_shortcut().register(shortcut) {
                let flag = self.quick.flag();
                log::error!("Module {flag} shortcut registration error: {e}");
                self.notify_shortcut_registration_error(
                    &format!("quick_action_{action_id}"),
                    shortcut,
                    e,
                );
            }
        }
        if std::env::var("ROTOR_SIMULATE_SHORTCUT_CONFLICT").as_deref() == Ok("1") {
            self.shortcut_registration_notices
                .push(ShortcutRegistrationNotice {
                    key: "shortcut_screenshot".to_string(),
                    shortcut: "Ctrl+Shift+Y".to_string(),
                    message: "Simulated shortcut conflict".to_string(),
                });
        }
        if let Some(shortcut_notice) = self.shortcut_registration_notices.first().cloned() {
            Tray::show_setting_window(&app, Some(shortcut_notice));
        }

        self.app = Some(app);

        Ok(())
    }

    fn update_search_result(
        filename: String,
        update_result: Vec<SearchResultItem>,
        if_increase: bool,
    ) {
        let app_handle = {
            let app = Application::lock_global();
            app.app.clone()
        };

        if let Some(app_handle) = app_handle {
            if let Err(e) = app_handle.emit_to(
                "searcher",
                "update_result",
                (filename, update_result, if_increase),
            ) {
                log::warn!("Failed to emit search result update: {e}");
            }
        }
    }

    fn update_search_index_state(state: String) {
        let app_handle = {
            let app = Application::lock_global();
            app.app.clone()
        };

        if let Some(app_handle) = app_handle {
            if let Err(e) = app_handle.emit_to("searcher", "index-state-changed", state) {
                log::warn!("Failed to emit search index state change: {e}");
            }
        }
    }

    fn should_ignore_shortcut_press(&mut self, shortcut_id: u32, shortcut: &Shortcut) -> bool {
        let now = Instant::now();

        if self.pressed_shortcuts.contains(&shortcut_id) {
            if self
                .last_shortcut_triggers
                .get(&shortcut_id)
                .is_some_and(|last_trigger| {
                    now.duration_since(*last_trigger) < PRESSED_SHORTCUT_STALE_AFTER
                })
            {
                log::debug!("Ignoring repeated global shortcut press: {shortcut}");
                return true;
            }

            self.pressed_shortcuts.remove(&shortcut_id);
        }

        if self
            .last_shortcut_triggers
            .get(&shortcut_id)
            .is_some_and(|last_trigger| {
                now.duration_since(*last_trigger) < SHORTCUT_TRIGGER_DEBOUNCE
            })
        {
            log::debug!("Ignoring debounced global shortcut press: {shortcut}");
            return true;
        }

        self.pressed_shortcuts.insert(shortcut_id);
        false
    }

    fn finish_shortcut_trigger(&mut self, shortcut_id: u32) {
        self.last_shortcut_triggers
            .insert(shortcut_id, Instant::now());
    }

    pub fn take_shortcut_registration_notices(&mut self) -> Vec<ShortcutRegistrationNotice> {
        std::mem::take(&mut self.shortcut_registration_notices)
    }

    pub fn update_module_shortcut(&mut self, key: &str, shortcut: Shortcut) {
        match key {
            "shortcut_screenshot" => self.screenshot_shortcut = Some(shortcut),
            "shortcut_search" => self.search_shortcut = Some(shortcut),
            "shortcut_translate_select" => self.translate_select_shortcut = Some(shortcut),
            "shortcut_translate_input" => self.translate_input_shortcut = Some(shortcut),
            _ => {}
        }
    }

    fn notify_shortcut_registration_error(
        &mut self,
        key: &str,
        shortcut: Shortcut,
        error: tauri_plugin_global_shortcut::Error,
    ) {
        let notice = ShortcutRegistrationNotice {
            key: key.to_string(),
            shortcut: shortcut.to_string(),
            message: error.to_string(),
        };

        self.shortcut_registration_notices.push(notice.clone());

        let Some(app) = self.app.as_ref() else {
            return;
        };

        Tray::show_setting_window(app, Some(notice));
    }
}

static INSTANCE: LazyLock<Mutex<Application>> = LazyLock::new(|| Mutex::new(Application::new()));

static GLOBAL_HOTKEY_DISPATCHER: LazyLock<mpsc::Sender<GlobalHotkeyDispatch>> =
    LazyLock::new(|| {
        let (sender, receiver) = mpsc::channel::<GlobalHotkeyDispatch>();
        thread::Builder::new()
            .name("rotor-global-hotkey-dispatch".to_string())
            .spawn(move || {
                while let Ok(dispatch) = receiver.recv() {
                    dispatch_global_hotkey_event(dispatch);
                }
            })
            .unwrap_or_else(|error| panic!("Failed to start global shortcut dispatcher: {error}"));
        sender
    });
