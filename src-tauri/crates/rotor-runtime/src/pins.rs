use crate::services::{OperationId, RuntimeEvent};
use async_channel::{Receiver, Sender};
use image::RgbaImage;
use rotor_screenshot::{
    pin_store::{PinStore, StoredPin},
    shotter_record::ShotterConfig,
};
use std::{path::PathBuf, sync::Arc};
use tokio::{runtime::Runtime, sync::oneshot, task::JoinHandle};

pub struct RestoredPins {
    pub pins: Vec<StoredPin>,
    pub warnings: Vec<String>,
}

pub enum PinEvent {
    Restored {
        id: OperationId,
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
    Restore {
        id: OperationId,
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
            Self::Restore { id } => PinEvent::Restored {
                id: *id,
                result: Err(error),
            },
            Self::Create { id, .. } => PinEvent::Created {
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
}

impl PinService {
    pub fn new(runtime: &Runtime, directory: PathBuf, events: Sender<RuntimeEvent>) -> Self {
        let (sender, receiver) = async_channel::bounded(8);
        let worker = runtime.spawn(run(directory, receiver, events));
        Self {
            sender,
            worker: Some(worker),
        }
    }
    pub fn submit(&self, command: PinCommand) -> Result<(), String> {
        self.sender
            .try_send(command)
            .map_err(|_| "pin persistence queue is busy or closed".into())
    }
}

async fn run(directory: PathBuf, commands: Receiver<PinCommand>, events: Sender<RuntimeEvent>) {
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
}

fn execute(store: &mut Result<PinStore, String>, command: PinCommand) -> PinEvent {
    match command {
        PinCommand::Restore { id } => PinEvent::Restored {
            id,
            result: store.as_ref().map_err(Clone::clone).map(|store| {
                let (pins, warnings) = store.load_pins();
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
