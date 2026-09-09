use async_channel::{Receiver, Sender};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use rotor_runtime::{
    OperationId,
    shortcuts::{
        self, HotkeyBackend, ShortcutAction, ShortcutBinding, ShortcutDebounce, ShortcutTransaction,
    },
};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::Instant,
};
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    ShowSettings,
    ShowTranslator,
    ShowSearch,
    SelectText,
    Capture,
    ShowPins,
    Quit,
    Shortcut {
        key: u32,
        generation: u64,
        pressed_at: Instant,
    },
    RecordedShortcut {
        key: u32,
        generation: u64,
    },
}
const CONTROLS: [(u8, Command); 7] = [
    (1, Command::Quit),
    (2, Command::Capture),
    (4, Command::ShowPins),
    (8, Command::ShowSearch),
    (16, Command::ShowTranslator),
    (32, Command::SelectText),
    (64, Command::ShowSettings),
];

#[derive(Clone)]
pub struct CommandBus {
    wake: Sender<()>,
    pending: Arc<AtomicU8>,
    shortcuts: Arc<Mutex<VecDeque<Command>>>,
}
impl CommandBus {
    pub fn new() -> (Self, Receiver<()>) {
        let (wake, receiver) = async_channel::bounded(1);
        (
            Self {
                wake,
                pending: Arc::new(AtomicU8::new(0)),
                shortcuts: Arc::new(Mutex::new(VecDeque::new())),
            },
            receiver,
        )
    }
    pub fn request(&self, command: Command) {
        if let Some((bit, _)) = CONTROLS.iter().find(|(_, value)| *value == command) {
            self.pending.fetch_or(*bit, Ordering::Release);
        } else {
            let mut queued = self
                .shortcuts
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if queued.len() == 32 {
                eprintln!("Shortcut dispatch queue is full");
                return;
            }
            queued.push_back(command);
        }
        let _ = self.wake.try_send(());
    }
    pub fn take(&self) -> Option<Command> {
        let pending = self.pending.swap(0, Ordering::AcqRel);
        let mut chosen = None;
        for (bit, command) in CONTROLS {
            if pending & bit == 0 {
                continue;
            }
            if chosen.is_none() {
                chosen = Some(command);
            } else {
                self.pending.fetch_or(bit, Ordering::Release);
            }
        }
        let mut queued = self
            .shortcuts
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if chosen == Some(Command::Quit) {
            queued.clear();
            self.pending.store(0, Ordering::Release);
            return chosen;
        }
        if chosen.is_none() {
            chosen = queued.pop_front();
        }
        if !queued.is_empty() || self.pending.load(Ordering::Acquire) != 0 {
            let _ = self.wake.try_send(());
        }
        chosen
    }
    pub fn close(&self) {
        self.wake.close();
    }
}
struct NativeBackend {
    manager: GlobalHotKeyManager,
    enabled: bool,
}
impl HotkeyBackend for NativeBackend {
    fn register(&mut self, key: HotKey) -> Result<(), String> {
        if self.enabled {
            self.manager
                .register(key)
                .map_err(|error| format!("{key}: {error}"))?;
        }
        Ok(())
    }
    fn unregister(&mut self, key: HotKey) -> Result<(), String> {
        if self.enabled {
            self.manager
                .unregister(key)
                .map_err(|error| format!("{key}: {error}"))?;
        }
        Ok(())
    }
}
struct ActiveBindings {
    generation: u64,
    bindings: Vec<ShortcutBinding>,
}
impl ActiveBindings {
    fn retain_registered(&mut self, registered: &[HotKey]) {
        // Invalidate queued events from before an aborted transaction. If a
        // rollback could not restore a key, do not dispatch its old action.
        self.generation = self.generation.wrapping_add(1);
        self.bindings
            .retain(|binding| registered.iter().any(|key| key.id() == binding.key.id()));
    }
}
struct Pending {
    id: OperationId,
    transaction: ShortcutTransaction,
    bindings: Vec<ShortcutBinding>,
}
pub struct SystemServices {
    tray: TrayIcon,
    backend: NativeBackend,
    registered: Vec<HotKey>,
    active: Arc<Mutex<ActiveBindings>>,
    paused: Arc<AtomicBool>,
    pending: Option<Pending>,
    development: bool,
    pub warning: Option<String>,
}
impl SystemServices {
    pub fn new(
        commands: CommandBus,
        config: &rotor_common::Config,
        enable_hotkeys: bool,
        development: bool,
        recording: Arc<shortcuts::ShortcutRecording>,
    ) -> Result<Self, String> {
        let image = image::load_from_memory(include_bytes!("../../../assets/icons/32x32.png"))
            .map_err(|error| error.to_string())?
            .into_rgba8();
        let (width, height) = image.dimensions();
        let icon = tray_icon::Icon::from_rgba(image.into_raw(), width, height)
            .map_err(|error| error.to_string())?;
        let tray = TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip(rotor_common::native_app::PRODUCT_NAME)
            .build()
            .map_err(|error| error.to_string())?;
        let mut backend = NativeBackend {
            manager: GlobalHotKeyManager::new().map_err(|error| error.to_string())?,
            enabled: enable_hotkeys,
        };
        let mut warnings = Vec::new();
        let planned = shortcuts::bindings(config, development).unwrap_or_else(|error| {
            warnings.push(error);
            vec![ShortcutBinding {
                key: "Ctrl+Alt+Shift+G".parse().expect("fixed settings shortcut"),
                action: ShortcutAction::Settings,
            }]
        });
        let mut registered = Vec::new();
        let mut bindings = Vec::new();
        for binding in planned {
            match backend.register(binding.key) {
                Ok(()) => {
                    registered.push(binding.key);
                    bindings.push(binding);
                }
                Err(error) => warnings.push(error),
            }
        }
        let active = Arc::new(Mutex::new(ActiveBindings {
            generation: 1,
            bindings,
        }));
        let paused = Arc::new(AtomicBool::new(false));
        let callback_active = active.clone();
        let callback_paused = paused.clone();
        let debounce = Mutex::new(ShortcutDebounce::default());
        let dispatch = commands.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |event: GlobalHotKeyEvent| {
            let pressed_at = Instant::now();
            let mut debounce = debounce.lock().unwrap_or_else(|error| error.into_inner());
            if event.state == HotKeyState::Released {
                debounce.release(event.id);
                return;
            }
            if callback_paused.load(Ordering::Acquire) {
                return;
            }
            let active = callback_active
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let recording_now = recording.active();
            if !active
                .bindings
                .iter()
                .any(|binding| binding.key.id() == event.id)
                || (!recording_now
                    && (recording.quiet(Instant::now())
                        || !debounce.press(event.id, Instant::now())))
            {
                return;
            }
            dispatch.request(if recording_now {
                Command::RecordedShortcut {
                    key: event.id,
                    generation: active.generation,
                }
            } else {
                Command::Shortcut {
                    key: event.id,
                    generation: active.generation,
                    pressed_at,
                }
            });
        }));
        let mut system = Self {
            tray,
            backend,
            registered,
            active,
            paused,
            pending: None,
            development,
            warning: (!warnings.is_empty()).then(|| warnings.join("\n")),
        };
        system.update_menu(commands, config)?;
        Ok(system)
    }
    pub fn resolve(&self, key: u32, generation: u64) -> Option<ShortcutAction> {
        if self.paused.load(Ordering::Acquire) {
            return None;
        }
        let active = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if active.generation != generation {
            return None;
        }
        active
            .bindings
            .iter()
            .find(|binding| binding.key.id() == key)
            .map(|binding| binding.action.clone())
    }
    pub fn shortcut_label(&self, key: u32, generation: u64) -> Option<String> {
        let active = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if active.generation != generation {
            return None;
        }
        active
            .bindings
            .iter()
            .find(|binding| binding.key.id() == key)
            .map(|binding| binding.key.to_string())
    }
    pub fn prepare(
        &mut self,
        id: OperationId,
        config: &rotor_common::Config,
    ) -> Result<(), String> {
        if self.pending.is_some() {
            return Err("Another shortcut transaction is active".into());
        }
        let bindings = shortcuts::bindings(config, self.development)?;
        let desired: Vec<_> = bindings.iter().map(|binding| binding.key).collect();
        self.paused.store(true, Ordering::Release);
        match ShortcutTransaction::prepare(&mut self.backend, &mut self.registered, &desired) {
            Ok(transaction) => {
                self.pending = Some(Pending {
                    id,
                    transaction,
                    bindings,
                });
                Ok(())
            }
            Err(error) => {
                self.reconcile_active_bindings();
                self.paused.store(false, Ordering::Release);
                self.warning = Some(error.clone());
                Err(error)
            }
        }
    }
    pub fn finish(&mut self, id: OperationId, committed: bool) -> Result<(), String> {
        if self.pending.as_ref().is_none_or(|pending| pending.id != id) {
            return Err("Shortcut transaction no longer exists".into());
        }
        let pending = self.pending.take().unwrap();
        let result = if committed {
            let mut active = self
                .active
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            active.generation = active.generation.wrapping_add(1);
            active.bindings = pending.bindings;
            Ok(())
        } else {
            let result = pending
                .transaction
                .rollback(&mut self.backend, &mut self.registered);
            self.reconcile_active_bindings();
            result
        };
        self.paused.store(false, Ordering::Release);
        if let Err(error) = &result {
            self.warning = Some(error.clone());
        }
        result
    }
    fn reconcile_active_bindings(&mut self) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        active.retain_registered(&self.registered);
    }
    pub fn update_menu(
        &mut self,
        commands: CommandBus,
        config: &rotor_common::Config,
    ) -> Result<(), String> {
        let menu = Menu::new();
        let chinese = rotor_common::i18n::language_for_config(config) == "zh-CN";
        let items: Vec<_> = [
            ("设置", "Settings", Command::ShowSettings),
            ("退出", "Quit", Command::Quit),
        ]
        .into_iter()
        .map(|(zh, en, command)| {
            (
                MenuItem::new(if chinese { zh } else { en }, true, None),
                command,
            )
        })
        .collect();
        for (item, _) in &items {
            menu.append(item).map_err(|error| error.to_string())?;
        }
        let actions: Vec<_> = items
            .iter()
            .map(|(item, command)| (item.id().clone(), *command))
            .collect();
        #[cfg(target_os = "windows")]
        let menu = crate::tray_menu::NativeMenu::new(menu, config)?;
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if let Some((_, command)) = actions.iter().find(|(id, _)| *id == event.id) {
                commands.request(*command);
            }
        }));
        self.tray.set_menu(Some(Box::new(menu)));
        Ok(())
    }
    pub fn stop_events(&mut self) {
        self.paused.store(true, Ordering::Release);
        for key in self.registered.drain(..) {
            let _ = self.backend.unregister(key);
        }
        self.pending = None;
        GlobalHotKeyEvent::set_event_handler(None::<fn(GlobalHotKeyEvent)>);
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
    }
}
impl Drop for SystemServices {
    fn drop(&mut self) {
        self.stop_events();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rollback_invalidates_old_events_and_keeps_only_registered_old_actions() {
        let retained: HotKey = "Ctrl+A".parse().unwrap();
        let missing: HotKey = "Ctrl+B".parse().unwrap();
        let leftover: HotKey = "Ctrl+C".parse().unwrap();
        let mut active = ActiveBindings {
            generation: 7,
            bindings: vec![
                ShortcutBinding {
                    key: retained,
                    action: ShortcutAction::Search,
                },
                ShortcutBinding {
                    key: missing,
                    action: ShortcutAction::Capture,
                },
            ],
        };
        active.retain_registered(&[retained, leftover]);
        assert_eq!(active.generation, 8);
        assert_eq!(active.bindings.len(), 1);
        assert_eq!(active.bindings[0].key, retained);
        assert_eq!(active.bindings[0].action, ShortcutAction::Search);
    }
    #[test]
    fn exit_is_not_lost_when_activation_wakeups_are_coalesced() {
        let (bus, receiver) = CommandBus::new();
        for _ in 0..100 {
            bus.request(Command::ShowSettings);
        }
        bus.request(Command::Shortcut {
            key: 1,
            generation: 1,
            pressed_at: Instant::now(),
        });
        bus.request(Command::Quit);
        assert_eq!(receiver.len(), 1);
        assert_eq!(bus.take(), Some(Command::Quit));
        assert!(bus.take().is_none());
    }
    #[test]
    fn distinct_window_requests_survive_a_single_wakeup() {
        let (bus, receiver) = CommandBus::new();
        bus.request(Command::ShowSettings);
        bus.request(Command::ShowTranslator);
        receiver.try_recv().unwrap();
        assert_eq!(bus.take(), Some(Command::ShowTranslator));
        receiver.try_recv().unwrap();
        assert_eq!(bus.take(), Some(Command::ShowSettings));
    }
    #[test]
    fn shortcut_payloads_keep_generation_and_order() {
        let (bus, _) = CommandBus::new();
        let first = Command::Shortcut {
            key: 4,
            generation: 7,
            pressed_at: Instant::now(),
        };
        let second = Command::Shortcut {
            key: 5,
            generation: 8,
            pressed_at: Instant::now(),
        };
        bus.request(first);
        bus.request(second);
        assert_eq!(bus.take(), Some(first));
        assert_eq!(bus.take(), Some(second));
    }
}
