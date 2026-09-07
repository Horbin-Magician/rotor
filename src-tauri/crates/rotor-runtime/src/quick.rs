use std::{collections::HashSet, error::Error, fmt, process::Command, str::FromStr};

use global_hotkey::hotkey::HotKey as Shortcut;
use rotor_common::{AppConfig, DEFAULT_QUICK_ACTIONS, DEFAULT_QUICK_ACTIONS_REVISION};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuickAction {
    pub id: String,
    pub name: String,
    pub shortcut: String,
    pub command: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

pub struct Quick {
    actions: Vec<QuickAction>,
}

impl Quick {
    pub fn flag(&self) -> &str {
        "quick"
    }

    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    pub fn reload(&mut self) {
        self.actions = load_actions_from_config();
    }

    pub fn actions(&self) -> Vec<QuickAction> {
        self.actions.clone()
    }

    pub fn set_actions(&mut self, actions: Vec<QuickAction>) {
        self.actions = actions;
    }

    pub fn get_shortcuts(&self) -> Vec<(String, Shortcut)> {
        parse_shortcuts(&self.actions).unwrap_or_else(|error| {
            log::warn!("Invalid quick action shortcuts: {error}");
            Vec::new()
        })
    }

    pub fn run_by_shortcut(&self, shortcut: &Shortcut) -> Result<bool, Box<dyn Error>> {
        let Some(action) = self.find_by_shortcut(shortcut) else {
            return Ok(false);
        };

        run_command(&action.command)?;
        Ok(true)
    }

    pub fn run_action(&self, id: &str) -> Result<(), Box<dyn Error>> {
        let Some(action) = self.actions.iter().find(|action| action.id == id) else {
            return Err(format!("Quick action `{id}` not found").into());
        };

        run_command(&action.command)
    }

    fn find_by_shortcut(&self, shortcut: &Shortcut) -> Option<&QuickAction> {
        self.actions
            .iter()
            .filter(|action| action.enabled)
            .find(|action| {
                Shortcut::from_str(&action.shortcut)
                    .map(|action_shortcut| action_shortcut == *shortcut)
                    .unwrap_or(false)
            })
    }
}

impl Default for Quick {
    fn default() -> Self {
        Self::new()
    }
}

pub fn normalize_actions(actions: Vec<QuickAction>) -> Result<Vec<QuickAction>, QuickActionError> {
    let mut ids = HashSet::new();
    let mut normalized = Vec::with_capacity(actions.len());

    for mut action in actions {
        action.id = action.id.trim().to_string();
        action.name = action.name.trim().to_string();
        action.shortcut = action.shortcut.trim().to_string();
        action.command = action.command.trim().to_string();

        if action.id.is_empty() {
            return Err(QuickActionError::InvalidAction(
                "Quick action id cannot be empty".to_string(),
            ));
        }
        if action.name.is_empty() {
            return Err(QuickActionError::InvalidAction(format!(
                "Quick action `{}` name cannot be empty",
                action.id
            )));
        }
        if action.enabled && action.command.is_empty() {
            return Err(QuickActionError::InvalidAction(format!(
                "Quick action `{}` command cannot be empty",
                action.name
            )));
        }
        if !ids.insert(action.id.clone()) {
            return Err(QuickActionError::InvalidAction(format!(
                "Duplicate quick action id `{}`",
                action.id
            )));
        }

        if action.enabled {
            action.shortcut = Shortcut::from_str(&action.shortcut)
                .map_err(|error| QuickActionError::InvalidShortcut {
                    id: action.id.clone(),
                    shortcut: action.shortcut.clone(),
                    message: error.to_string(),
                })?
                .to_string();
        }

        normalized.push(action);
    }

    parse_shortcuts(&normalized)?;
    Ok(normalized)
}

pub fn parse_shortcuts(
    actions: &[QuickAction],
) -> Result<Vec<(String, Shortcut)>, QuickActionError> {
    let mut shortcut_ids = HashSet::new();
    let mut shortcuts = Vec::new();

    for action in actions.iter().filter(|action| action.enabled) {
        let shortcut = Shortcut::from_str(&action.shortcut).map_err(|error| {
            QuickActionError::InvalidShortcut {
                id: action.id.clone(),
                shortcut: action.shortcut.clone(),
                message: error.to_string(),
            }
        })?;

        if !shortcut_ids.insert(shortcut.id()) {
            return Err(QuickActionError::DuplicateShortcut {
                id: action.id.clone(),
                shortcut: shortcut.to_string(),
            });
        }

        shortcuts.push((action.id.clone(), shortcut));
    }

    Ok(shortcuts)
}

#[derive(Debug)]
pub enum QuickActionError {
    InvalidAction(String),
    InvalidShortcut {
        id: String,
        shortcut: String,
        message: String,
    },
    DuplicateShortcut {
        id: String,
        shortcut: String,
    },
}

impl fmt::Display for QuickActionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuickActionError::InvalidAction(message) => write!(f, "{message}"),
            QuickActionError::InvalidShortcut {
                id,
                shortcut,
                message,
            } => write!(
                f,
                "Invalid shortcut `{shortcut}` for quick action `{id}`: {message}"
            ),
            QuickActionError::DuplicateShortcut { id, shortcut } => {
                write!(f, "Duplicate quick action shortcut `{shortcut}` for `{id}`")
            }
        }
    }
}

impl Error for QuickActionError {}

fn load_actions_from_config() -> Vec<QuickAction> {
    let mut config = AppConfig::lock_global();
    let actions = config.get("quick_actions").cloned();

    let Some(actions) = actions else {
        return Vec::new();
    };

    let mut should_save_actions = false;
    let mut actions = match serde_json::from_str::<Vec<QuickAction>>(&actions) {
        Ok(actions) => actions,
        Err(error) => {
            log::warn!("Invalid quick actions config: {error}");
            should_save_actions = true;
            default_actions()
        }
    };

    let should_update_revision = config
        .get_user("quick_actions_revision")
        .map(String::as_str)
        != Some(DEFAULT_QUICK_ACTIONS_REVISION);
    if should_update_revision {
        append_missing_default_actions(&mut actions);
        should_save_actions = true;
    }

    if should_save_actions {
        match serde_json::to_string(&actions) {
            Ok(serialized) => {
                if let Err(error) = config.set_many([
                    ("quick_actions".to_string(), serialized),
                    (
                        "quick_actions_revision".to_string(),
                        DEFAULT_QUICK_ACTIONS_REVISION.to_string(),
                    ),
                ]) {
                    log::warn!("Failed to save migrated quick actions: {error}");
                }
            }
            Err(error) => {
                log::warn!("Failed to serialize migrated quick actions: {error}");
            }
        }
    }

    actions
}

pub fn default_actions() -> Vec<QuickAction> {
    serde_json::from_str::<Vec<QuickAction>>(DEFAULT_QUICK_ACTIONS).unwrap_or_else(|error| {
        log::warn!("Invalid default quick actions config: {error}");
        Vec::new()
    })
}

pub fn migrate_actions(
    json: &str,
    revision: Option<&str>,
) -> Result<Option<Vec<QuickAction>>, String> {
    if revision == Some(DEFAULT_QUICK_ACTIONS_REVISION) {
        return Ok(None);
    }
    let mut actions = normalize_actions(
        serde_json::from_str(json).map_err(|error| format!("Invalid quick actions: {error}"))?,
    )
    .map_err(|error| error.to_string())?;
    let used: HashSet<_> = parse_shortcuts(&actions)
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|(_, key)| key.id())
        .collect();
    for default in default_actions() {
        if actions.iter().any(|action| action.id == default.id) {
            continue;
        }
        let key = Shortcut::from_str(&default.shortcut).map_err(|error| error.to_string())?;
        if !used.contains(&key.id()) {
            actions.push(default);
        }
    }
    Ok(Some(actions))
}

fn append_missing_default_actions(actions: &mut Vec<QuickAction>) {
    for default_action in default_actions() {
        if actions.iter().all(|action| action.id != default_action.id) {
            actions.push(default_action);
        }
    }
}

pub fn actions_from_config(config: &rotor_common::Config) -> Result<Vec<QuickAction>, String> {
    let actions = config
        .get("quick_actions")
        .map(String::as_str)
        .unwrap_or(DEFAULT_QUICK_ACTIONS);
    normalize_actions(
        serde_json::from_str(actions).map_err(|error| format!("Invalid quick actions: {error}"))?,
    )
    .map_err(|error| error.to_string())
}

pub fn run_command(command: &str) -> Result<(), Box<dyn Error>> {
    let command = command.trim();
    if command.is_empty() {
        return Err("Quick action command is empty".into());
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;

        Command::new("cmd")
            .args(["/C", command])
            .creation_flags(0x08000000)
            .spawn()?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("sh").args(["-lc", command]).spawn()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(id: &str, shortcut: &str, enabled: bool) -> QuickAction {
        QuickAction {
            id: id.into(),
            name: " Test ".into(),
            command: " echo test ".into(),
            shortcut: shortcut.into(),
            enabled,
        }
    }

    #[test]
    fn revision_migration_keeps_custom_commands_and_occupied_default_shortcuts() {
        let default = default_actions().remove(0);
        let custom = QuickAction {
            id: "custom".into(),
            name: "Mine".into(),
            shortcut: default.shortcut,
            command: "custom command".into(),
            enabled: true,
        };
        let json = serde_json::to_string(&vec![custom]).unwrap();
        let migrated = migrate_actions(&json, Some("1")).unwrap().unwrap();
        assert_eq!(
            migrated
                .iter()
                .find(|action| action.id == "custom")
                .unwrap()
                .command,
            "custom command"
        );
        assert!(!migrated.iter().any(|action| action.id == default.id));
        assert!(migrate_actions(&json, Some(DEFAULT_QUICK_ACTIONS_REVISION))
            .unwrap()
            .is_none());
    }

    #[test]
    fn equivalent_enabled_shortcuts_conflict_after_normalization() {
        let actions = vec![
            action("one", "Ctrl+Shift+T", true),
            action("two", "Shift+Ctrl+T", true),
        ];
        assert!(matches!(
            normalize_actions(actions),
            Err(QuickActionError::DuplicateShortcut { .. })
        ));
    }

    #[test]
    fn disabled_actions_keep_unregistered_shortcuts_and_normalize_text() {
        let actions = normalize_actions(vec![
            action(" one ", "Ctrl+Shift+T", true),
            action("two", "not-a-shortcut", false),
        ])
        .unwrap();
        assert_eq!(actions[0].id, "one");
        assert_eq!(actions[0].name, "Test");
        assert_eq!(actions[0].command, "echo test");
        assert_eq!(parse_shortcuts(&actions).unwrap().len(), 1);
        assert_eq!(actions[1].shortcut, "not-a-shortcut");
    }
}
