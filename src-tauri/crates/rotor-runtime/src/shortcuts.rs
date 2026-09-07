use global_hotkey::hotkey::{HotKey, Modifiers};
use rotor_common::Config;
use std::{collections::HashSet, str::FromStr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShortcutAction {
    Settings,
    Search,
    Capture,
    TranslateSelection,
    TranslateInput,
    Quick(String),
}
#[derive(Clone, Debug)]
pub struct ShortcutBinding {
    pub key: HotKey,
    pub action: ShortcutAction,
}

pub fn bindings(config: &Config, development: bool) -> Result<Vec<ShortcutBinding>, String> {
    let mut bindings = Vec::new();
    if development {
        bindings.push(ShortcutBinding {
            key: HotKey::from_str("Ctrl+Alt+Shift+G").map_err(|error| error.to_string())?,
            action: ShortcutAction::Settings,
        });
    }
    let configured = [
        ("shortcut_search", ShortcutAction::Search),
        ("shortcut_screenshot", ShortcutAction::Capture),
        (
            "shortcut_translate_select",
            ShortcutAction::TranslateSelection,
        ),
        ("shortcut_translate_input", ShortcutAction::TranslateInput),
    ];
    let key = |value: &str| -> Result<HotKey, String> {
        let parsed = HotKey::from_str(value).map_err(|error| error.to_string())?;
        Ok(if development {
            HotKey::new(Some(parsed.mods | Modifiers::ALT), parsed.key)
        } else {
            parsed
        })
    };
    for (setting, action) in configured {
        let value = config
            .get(setting)
            .ok_or_else(|| format!("Missing shortcut setting: {setting}"))?;
        if !value.trim().is_empty() {
            bindings.push(ShortcutBinding {
                key: key(value)?,
                action,
            });
        }
    }
    for action in crate::quick::actions_from_config(config)? {
        if action.enabled {
            bindings.push(ShortcutBinding {
                key: key(&action.shortcut)?,
                action: ShortcutAction::Quick(action.id),
            });
        }
    }
    let mut ids = HashSet::new();
    for binding in &bindings {
        if !ids.insert(binding.key.id()) {
            return Err(format!("Duplicate global shortcut: {}", binding.key));
        }
    }
    Ok(bindings)
}

pub trait HotkeyBackend {
    fn register(&mut self, key: HotKey) -> Result<(), String>;
    fn unregister(&mut self, key: HotKey) -> Result<(), String>;
}

#[cfg(test)]
mod scope_tests {
    #[test]
    fn production_bindings_do_not_add_the_development_settings_hotkey() {
        let directory = tempfile::tempdir().unwrap();
        let config = rotor_common::ConfigService::load_from(directory.path())
            .unwrap()
            .get_all();
        assert!(super::bindings(&config, true)
            .unwrap()
            .iter()
            .any(|binding| binding.action == super::ShortcutAction::Settings));
        assert!(!super::bindings(&config, false)
            .unwrap()
            .iter()
            .any(|binding| binding.action == super::ShortcutAction::Settings));
    }
}

#[derive(Default)]
pub struct ShortcutRecording {
    active: std::sync::atomic::AtomicBool,
    quiet_until: std::sync::Mutex<Option<std::time::Instant>>,
}
impl ShortcutRecording {
    pub fn set(&self, active: bool) {
        let previous = self
            .active
            .swap(active, std::sync::atomic::Ordering::AcqRel);
        if previous && !active {
            *self
                .quiet_until
                .lock()
                .unwrap_or_else(|error| error.into_inner()) =
                Some(std::time::Instant::now() + std::time::Duration::from_millis(500));
        }
    }
    pub fn active(&self) -> bool {
        self.active.load(std::sync::atomic::Ordering::Acquire)
    }
    pub fn quiet(&self, now: std::time::Instant) -> bool {
        self.quiet_until
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_some_and(|until| now < until)
    }
}

#[derive(Default)]
pub struct ShortcutDebounce {
    pressed: HashSet<u32>,
    last: std::collections::HashMap<u32, std::time::Instant>,
}
impl ShortcutDebounce {
    pub fn release(&mut self, id: u32) {
        self.pressed.remove(&id);
    }
    pub fn press(&mut self, id: u32, now: std::time::Instant) -> bool {
        let elapsed = self
            .last
            .get(&id)
            .map(|last| now.saturating_duration_since(*last));
        if self.pressed.contains(&id)
            && elapsed.is_some_and(|elapsed| elapsed < std::time::Duration::from_secs(3))
        {
            return false;
        }
        if elapsed.is_some_and(|elapsed| elapsed < std::time::Duration::from_millis(500)) {
            return false;
        }
        self.pressed.insert(id);
        self.last.insert(id, now);
        true
    }
}

/// Applied on the shell's main thread. Config is written only after this stage.
pub struct ShortcutTransaction {
    added: Vec<HotKey>,
    removed: Vec<HotKey>,
}
impl ShortcutTransaction {
    pub fn prepare(
        backend: &mut impl HotkeyBackend,
        old: &[HotKey],
        new: &[HotKey],
    ) -> Result<Self, String> {
        let old_ids: HashSet<_> = old.iter().map(HotKey::id).collect();
        let new_ids: HashSet<_> = new.iter().map(HotKey::id).collect();
        let mut transaction = Self {
            added: Vec::new(),
            removed: Vec::new(),
        };
        let change = (|| {
            for key in old.iter().filter(|key| !new_ids.contains(&key.id())) {
                backend.unregister(*key)?;
                transaction.removed.push(*key);
            }
            for key in new.iter().filter(|key| !old_ids.contains(&key.id())) {
                backend.register(*key)?;
                transaction.added.push(*key);
            }
            Ok::<_, String>(())
        })();
        if let Err(error) = change {
            return Err(match transaction.rollback(backend) {
                Ok(()) => error,
                Err(rollback) => format!("{error}; rollback: {rollback}"),
            });
        }
        Ok(transaction)
    }
    pub fn rollback(&self, backend: &mut impl HotkeyBackend) -> Result<(), String> {
        let mut errors = Vec::new();
        for key in self.added.iter().rev() {
            if let Err(error) = backend.unregister(*key) {
                errors.push(error);
            }
        }
        for key in &self.removed {
            if let Err(error) = backend.register(*key) {
                errors.push(error);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Registry {
        keys: HashSet<u32>,
        fail: Option<u32>,
    }
    impl HotkeyBackend for Registry {
        fn register(&mut self, key: HotKey) -> Result<(), String> {
            if self.fail == Some(key.id()) {
                return Err("shortcut occupied".into());
            }
            self.keys.insert(key.id());
            Ok(())
        }
        fn unregister(&mut self, key: HotKey) -> Result<(), String> {
            self.keys.remove(&key.id());
            Ok(())
        }
    }
    #[test]
    fn partial_registration_failure_restores_old_keys() {
        let a = HotKey::from_str("Ctrl+A").unwrap();
        let b = HotKey::from_str("Ctrl+B").unwrap();
        let c = HotKey::from_str("Ctrl+C").unwrap();
        let mut backend = Registry {
            keys: HashSet::from([a.id()]),
            fail: Some(c.id()),
        };
        assert!(ShortcutTransaction::prepare(&mut backend, &[a], &[b, c]).is_err());
        assert_eq!(backend.keys, HashSet::from([a.id()]));
    }
    #[test]
    fn disk_failure_can_roll_back_a_successfully_staged_change() {
        let a = HotKey::from_str("Ctrl+A").unwrap();
        let b = HotKey::from_str("Ctrl+B").unwrap();
        let mut backend = Registry {
            keys: HashSet::from([a.id()]),
            fail: None,
        };
        let transaction = ShortcutTransaction::prepare(&mut backend, &[a], &[b]).unwrap();
        assert_eq!(backend.keys, HashSet::from([b.id()]));
        transaction.rollback(&mut backend).unwrap();
        assert_eq!(backend.keys, HashSet::from([a.id()]));
    }

    #[test]
    fn debounce_preserves_release_and_recovers_a_stale_press() {
        let now = std::time::Instant::now();
        let mut state = ShortcutDebounce::default();
        assert!(state.press(1, now));
        assert!(!state.press(1, now + std::time::Duration::from_secs(1)));
        state.release(1);
        assert!(state.press(1, now + std::time::Duration::from_secs(1)));
        assert!(!state.press(1, now + std::time::Duration::from_secs(2)));
        assert!(state.press(1, now + std::time::Duration::from_secs(4)));
    }

    #[test]
    fn recording_exit_has_a_quiet_period_without_muting_ordinary_window_closes() {
        let state = ShortcutRecording::default();
        state.set(false);
        assert!(!state.quiet(std::time::Instant::now()));
        state.set(true);
        assert!(state.active());
        state.set(false);
        assert!(!state.active());
        assert!(state.quiet(std::time::Instant::now()));
        assert!(!state.quiet(std::time::Instant::now() + std::time::Duration::from_secs(1)));
    }
}
