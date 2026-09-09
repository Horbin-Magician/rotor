use crate::services::{OperationId, RuntimeEvent};
use async_channel::{Receiver, Sender};
use image::RgbaImage;
use rotor_screenshot::{
    pin_store::{PinStore, StoredPin},
    shotter_record::ShotterConfig,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::{runtime::Runtime, sync::oneshot, task::JoinHandle};

pub struct RestoredPins {
    pub pins: Vec<StoredPin>,
    pub warnings: Vec<String>,
}

pub enum PinExportTarget {
    Clipboard,
    File(PathBuf),
}

pub enum PinEvent {
    Exported {
        id: OperationId,
        result: Result<(), String>,
    },
    Restored {
        id: OperationId,
        reveal: bool,
        result: Result<RestoredPins, String>,
    },
    Created {
        id: OperationId,
        result: Result<StoredPin, String>,
    },
    Updated {
        id: OperationId,
        pin_id: u32,
        result: Result<ShotterConfig, String>,
    },
    Deleted {
        id: OperationId,
        pin_id: u32,
        result: Result<(), String>,
    },
}

pub(crate) enum PinCommand {
    ExportFrame {
        id: OperationId,
        pin_id: Option<u32>,
        image: Arc<RgbaImage>,
        target: PinExportTarget,
    },
    CreateFromCapture {
        id: OperationId,
        image: Arc<RgbaImage>,
        config: ShotterConfig,
    },
    Export {
        id: OperationId,
        pin_id: Option<u32>,
        image: Arc<RgbaImage>,
        config: ShotterConfig,
        target: PinExportTarget,
    },
    Restore {
        id: OperationId,
        include_hidden: bool,
        excluded_ids: Vec<u32>,
    },
    Create {
        id: OperationId,
        image: Arc<RgbaImage>,
        config: ShotterConfig,
    },
    Update {
        id: OperationId,
        pin_id: u32,
        config: ShotterConfig,
    },
    Delete {
        id: OperationId,
        pin_id: u32,
    },
    Flush(oneshot::Sender<()>),
}

impl PinCommand {
    fn failure(&self, error: String) -> PinEvent {
        match self {
            Self::Export { id, .. } | Self::ExportFrame { id, .. } => PinEvent::Exported {
                id: *id,
                result: Err(error),
            },
            Self::Restore {
                id, include_hidden, ..
            } => PinEvent::Restored {
                id: *id,
                reveal: *include_hidden,
                result: Err(error),
            },
            Self::Create { id, .. } | Self::CreateFromCapture { id, .. } => PinEvent::Created {
                id: *id,
                result: Err(error),
            },
            Self::Update { id, pin_id, .. } => PinEvent::Updated {
                id: *id,
                pin_id: *pin_id,
                result: Err(error),
            },
            Self::Delete { id, pin_id } => PinEvent::Deleted {
                id: *id,
                pin_id: *pin_id,
                result: Err(error),
            },
            Self::Flush(_) => unreachable!("flush does not execute disk work"),
        }
    }
}

pub(crate) struct PinService {
    pub sender: Sender<PinCommand>,
    pub worker: Option<JoinHandle<()>>,
    pub final_updates: Arc<Mutex<Vec<(u32, ShotterConfig)>>>,
}

impl PinService {
    pub fn new(runtime: &Runtime, directory: PathBuf, events: Sender<RuntimeEvent>) -> Self {
        let (sender, receiver) = async_channel::bounded(8);
        let final_updates = Arc::new(Mutex::new(Vec::new()));
        let worker = runtime.spawn(run(directory, receiver, events, final_updates.clone()));
        Self {
            sender,
            worker: Some(worker),
            final_updates,
        }
    }
    pub fn submit(&self, command: PinCommand) -> Result<(), String> {
        self.sender
            .try_send(command)
            .map_err(|_| "pin persistence queue is busy or closed".into())
    }
}

async fn run(
    directory: PathBuf,
    commands: Receiver<PinCommand>,
    events: Sender<RuntimeEvent>,
    final_updates: Arc<Mutex<Vec<(u32, ShotterConfig)>>>,
) {
    let mut store = tokio::task::spawn_blocking(move || PinStore::load_from(&directory))
        .await
        .map_err(|error| error.to_string())
        .and_then(|result| result);
    while let Ok(command) = commands.recv().await {
        if let PinCommand::Flush(reply) = command {
            let _ = reply.send(());
            continue;
        }
        let failure = command.failure("Pin persistence worker failed".into());
        let task = tokio::task::spawn_blocking(move || {
            let event = execute(&mut store, command);
            (store, event)
        })
        .await;
        let (next, event) = match task {
            Ok(result) => result,
            Err(error) => {
                log::error!("Pin persistence worker failed: {error}");
                let _ = events.send(RuntimeEvent::Pin(failure)).await;
                store = Err(format!("Pin persistence worker failed: {error}"));
                continue;
            }
        };
        store = next;
        let _ = events.send(RuntimeEvent::Pin(event)).await;
    }
    let updates = std::mem::take(
        &mut *final_updates
            .lock()
            .unwrap_or_else(|error| error.into_inner()),
    );
    if !updates.is_empty() {
        let result = tokio::task::spawn_blocking(move || {
            store
                .as_mut()
                .map_err(|error| error.clone())?
                .update_existing_batch(updates)
        })
        .await;
        match result {
            Ok(Ok(warnings)) => {
                for warning in warnings {
                    log::warn!("Final pin update: {warning}");
                }
            }
            Ok(Err(error)) => log::error!("Final pin snapshot failed: {error}"),
            Err(error) => log::error!("Final pin snapshot task failed: {error}"),
        }
    }
}

fn export_frame(
    store: &mut Result<PinStore, String>,
    pin_id: Option<u32>,
    image: &RgbaImage,
    target: PinExportTarget,
) -> Result<(), String> {
    if image.width() == 0 || image.height() == 0 {
        return Err("Cannot export an empty pin frame".into());
    }
    match target {
        PinExportTarget::Clipboard => rotor_platform::clipboard::write_image(image)?,
        PinExportTarget::File(path) => {
            let bytes = rotor_screenshot::pin_store::png_bytes(image)?;
            rotor_common::persistence::atomic_write(&path, &bytes)
                .map_err(|error| error.to_string())?;
        }
    }
    if let Some(pin_id) = pin_id {
        store
            .as_mut()
            .map_err(|error| error.clone())?
            .delete(pin_id)
            .map_err(|error| format!("Image exported, but pin removal failed: {error}"))?;
    }
    Ok(())
}

fn execute(store: &mut Result<PinStore, String>, command: PinCommand) -> PinEvent {
    match command {
        PinCommand::ExportFrame {
            id,
            pin_id,
            image,
            target,
        } => PinEvent::Exported {
            id,
            result: export_frame(store, pin_id, &image, target),
        },
        PinCommand::CreateFromCapture { id, image, config } => PinEvent::Created {
            id,
            result: (|| {
                let (x, y, width, height) = config
                    .image_rect
                    .ok_or("Capture selection has no source rectangle")?;
                if width == 0
                    || height == 0
                    || x.checked_add(width)
                        .is_none_or(|right| right > image.width())
                    || y.checked_add(height)
                        .is_none_or(|bottom| bottom > image.height())
                {
                    return Err("Capture selection is outside the monitor image".into());
                }
                let cropped = Arc::new(
                    image::imageops::crop_imm(image.as_ref(), x, y, width, height).to_image(),
                );
                let pin_id = store
                    .as_mut()
                    .map_err(|error| error.clone())?
                    .create(&cropped, config.clone())?;
                Ok(StoredPin {
                    id: pin_id,
                    config,
                    image: cropped,
                })
            })(),
        },
        PinCommand::Export {
            id,
            pin_id,
            image,
            config,
            target,
        } => PinEvent::Exported {
            id,
            result: (|| {
                let image = rotor_screenshot::pin_store::crop_image(&image, &config)?;
                export_frame(store, pin_id, &image, target)
            })(),
        },
        PinCommand::Restore {
            id,
            include_hidden,
            excluded_ids,
        } => PinEvent::Restored {
            id,
            reveal: include_hidden,
            result: store.as_ref().map_err(Clone::clone).map(|store| {
                let (pins, warnings) = store.load_pins_for_restore(include_hidden, &excluded_ids);
                RestoredPins { pins, warnings }
            }),
        },
        PinCommand::Create { id, image, config } => PinEvent::Created {
            id,
            result: store
                .as_mut()
                .map_err(|error| error.clone())
                .and_then(|store| {
                    let pin_id = store.create(&image, config.clone())?;
                    Ok(StoredPin {
                        id: pin_id,
                        config,
                        image,
                    })
                }),
        },
        PinCommand::Update { id, pin_id, config } => PinEvent::Updated {
            id,
            pin_id,
            result: store
                .as_mut()
                .map_err(|error| error.clone())
                .and_then(|store| {
                    store.update(pin_id, config.clone())?;
                    Ok(config)
                }),
        },
        PinCommand::Delete { id, pin_id } => PinEvent::Deleted {
            id,
            pin_id,
            result: store
                .as_mut()
                .map_err(|error| error.clone())
                .and_then(|store| store.delete(pin_id)),
        },
        PinCommand::Flush(_) => unreachable!("flush is a queue barrier handled before disk work"),
    }
}
