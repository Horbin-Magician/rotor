//! Ordered settings persistence and shortcut coordination.
use super::{lock, OperationId, RuntimeEvent};
use async_channel::{Receiver, Sender};
use rotor_common::{Config, ConfigService};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, Weak,
};
use tokio::sync::oneshot;

pub(super) enum SettingsCommand {
    Save {
        id: OperationId,
        changes: Vec<(String, String)>,
    },
    Coalesced(Arc<Mutex<Option<SettingsPatch>>>),
    Flush(oneshot::Sender<()>),
}

pub(super) struct SettingsPatch {
    pub id: OperationId,
    pub changes: Vec<(String, String)>,
}

pub(super) type SettingsTail = Mutex<Option<Weak<Mutex<Option<SettingsPatch>>>>>;

pub enum SettingsCoordination {
    Prepare {
        id: OperationId,
        candidate: Config,
        reply: oneshot::Sender<Result<(), String>>,
    },
    Finish {
        id: OperationId,
        committed: bool,
        reply: oneshot::Sender<Result<(), String>>,
    },
}

pub(super) async fn settings_loop(
    config: Arc<Mutex<ConfigService>>,
    published: Arc<Mutex<Config>>,
    receiver: Receiver<SettingsCommand>,
    events: Sender<RuntimeEvent>,
    coordinate_shortcuts: Arc<AtomicBool>,
) {
    while let Ok(command) = receiver.recv().await {
        let command = match command {
            SettingsCommand::Coalesced(batch) => {
                let Some(patch) = lock(&batch).take() else {
                    continue;
                };
                SettingsCommand::Save {
                    id: patch.id,
                    changes: patch.changes,
                }
            }
            other => other,
        };
        match command {
            SettingsCommand::Save { id, mut changes } => {
                let normalized = normalize_shortcut_changes(&mut changes, &lock(&published));
                if let Err(error) = normalized {
                    let _ = events
                        .send(RuntimeEvent::SettingsSaved {
                            id,
                            result: Err(error),
                        })
                        .await;
                    continue;
                }
                let coordinated = coordinate_shortcuts.load(Ordering::Acquire)
                    && changes.iter().any(|(key, _)| {
                        (key.starts_with("shortcut_") && !key.starts_with("shortcut_pinwin_"))
                            || key == "quick_actions"
                    });
                if let Some((_, json)) = changes.iter_mut().find(|(key, _)| key == "quick_actions")
                {
                    let normalized = serde_json::from_str::<Vec<crate::QuickAction>>(json)
                        .map_err(|error| error.to_string())
                        .and_then(|actions| {
                            crate::quick::normalize_actions(actions)
                                .map_err(|error| error.to_string())
                        })
                        .and_then(|actions| {
                            serde_json::to_string(&actions).map_err(|error| error.to_string())
                        });
                    match normalized {
                        Ok(value) => {
                            *json = value;
                        }
                        Err(error) => {
                            let _ = events
                                .send(RuntimeEvent::SettingsSaved {
                                    id,
                                    result: Err(error),
                                })
                                .await;
                            continue;
                        }
                    }
                }
                if coordinated {
                    let mut candidate = lock(&published).clone();
                    candidate.extend(changes.clone());
                    let (reply, response) = oneshot::channel();
                    let result = if events
                        .send(RuntimeEvent::SettingsCoordination(
                            SettingsCoordination::Prepare {
                                id,
                                candidate,
                                reply,
                            },
                        ))
                        .await
                        .is_ok()
                    {
                        response
                            .await
                            .unwrap_or_else(|_| Err("Shortcut coordinator stopped".into()))
                    } else {
                        Err("Shortcut coordinator is unavailable".into())
                    };
                    if let Err(error) = result {
                        let _ = events
                            .send(RuntimeEvent::SettingsSaved {
                                id,
                                result: Err(error),
                            })
                            .await;
                        continue;
                    }
                }
                let config = config.clone();
                let mut result = tokio::task::spawn_blocking(move || {
                    let mut config = lock(&config);
                    config
                        .set_many(changes)
                        .map_err(|error| error.to_string())?;
                    Ok(config.get_all())
                })
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result);
                if let Ok(snapshot) = &result {
                    *lock(&published) = snapshot.clone();
                }
                if coordinated {
                    let (reply, response) = oneshot::channel();
                    let finished = if events
                        .send(RuntimeEvent::SettingsCoordination(
                            SettingsCoordination::Finish {
                                id,
                                committed: result.is_ok(),
                                reply,
                            },
                        ))
                        .await
                        .is_ok()
                    {
                        response
                            .await
                            .unwrap_or_else(|_| Err("Shortcut coordinator stopped".into()))
                    } else {
                        Err("Shortcut coordinator is unavailable".into())
                    };
                    if let Err(error) = finished {
                        result = Err(match result {
                            Ok(_) => error,
                            Err(original) => format!("{original}; {error}"),
                        });
                    }
                }
                let _ = events
                    .send(RuntimeEvent::SettingsSaved { id, result })
                    .await;
            }
            SettingsCommand::Flush(sender) => {
                let _ = sender.send(());
            }
            SettingsCommand::Coalesced(_) => unreachable!("coalesced write was resolved above"),
        }
    }
}

pub(super) fn normalize_shortcut_changes(
    changes: &mut [(String, String)],
    config: &Config,
) -> Result<(), String> {
    use std::str::FromStr;
    let chinese = rotor_common::i18n::language_for_config(config) == "zh-CN";
    for (key, value) in changes {
        let (zh, en) = match key.as_str() {
            "shortcut_search" => ("搜索快捷键", "Search shortcut"),
            "shortcut_screenshot" => ("截图快捷键", "Screenshot shortcut"),
            "shortcut_translate_select" => ("划词翻译快捷键", "Selection translation shortcut"),
            "shortcut_translate_input" => ("输入翻译快捷键", "Input translation shortcut"),
            "shortcut_pinwin_save" => ("贴图保存", "Save pinned image"),
            "shortcut_pinwin_close" => ("贴图关闭", "Close pinned image"),
            "shortcut_pinwin_copy" => ("贴图复制", "Copy pinned image"),
            "shortcut_pinwin_hide" => ("贴图最小化", "Minimize pinned image"),
            _ => continue,
        };
        let normalized = value.trim();
        if !normalized.is_empty() && global_hotkey::hotkey::HotKey::from_str(normalized).is_err() {
            return Err(if chinese {
                format!("“{zh}”格式不正确，请重新录制或输入完整按键组合。")
            } else {
                format!("Invalid shortcut for “{en}”. Record it again or enter a complete key combination.")
            });
        }
        *value = normalized.into();
    }
    Ok(())
}

pub(super) fn merge_settings_changes(
    current: &mut Vec<(String, String)>,
    incoming: Vec<(String, String)>,
) -> Result<(), String> {
    // Bound the number of fields in one automatic batch without rejecting a
    // replacement of an existing field. Validate before changing accepted data.
    let keys = current
        .iter()
        .chain(&incoming)
        .map(|(key, _)| key)
        .collect::<std::collections::HashSet<_>>();
    if keys.len() > 64 {
        return Err("too many fields in automatic settings batch".into());
    }
    for (key, value) in incoming {
        if let Some((_, previous)) = current.iter_mut().find(|(existing, _)| existing == &key) {
            *previous = value;
        } else {
            current.push((key, value));
        }
    }
    Ok(())
}
