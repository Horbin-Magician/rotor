use crate::RuntimeEvent;
use async_channel::Sender;
use rotor_updater::{CancellationToken, DownloadProgress, Release};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};
use tokio::runtime::Handle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdatePhase {
    Idle,
    Checking,
    Current,
    Available,
    Downloading,
    Ready,
    Installing,
    HandedOff,
    Failed,
}
#[derive(Clone, Debug)]
pub struct UpdateSnapshot {
    pub revision: u64,
    pub phase: UpdatePhase,
    pub release: Option<Arc<Release>>,
    pub path: Option<PathBuf>,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
}
impl Default for UpdateSnapshot {
    fn default() -> Self {
        Self {
            revision: 0,
            phase: UpdatePhase::Idle,
            release: None,
            path: None,
            downloaded: 0,
            total: None,
            error: None,
        }
    }
}
impl UpdateSnapshot {
    pub fn busy(&self) -> bool {
        matches!(
            self.phase,
            UpdatePhase::Checking
                | UpdatePhase::Downloading
                | UpdatePhase::Installing
                | UpdatePhase::HandedOff
        )
    }
}
pub(crate) struct UpdateService {
    runtime: Handle,
    state: Arc<Mutex<UpdateSnapshot>>,
    events: Sender<RuntimeEvent>,
    cancellation: Mutex<CancellationToken>,
    shutdown: CancellationToken,
    directory: PathBuf,
}
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|p| p.into_inner())
}
fn publish(
    state: &Mutex<UpdateSnapshot>,
    change: impl FnOnce(&mut UpdateSnapshot),
) -> Arc<UpdateSnapshot> {
    let mut state = lock(state);
    change(&mut state);
    state.revision += 1;
    Arc::new(state.clone())
}
impl UpdateService {
    pub fn new(runtime: Handle, events: Sender<RuntimeEvent>, directory: PathBuf) -> Self {
        Self {
            runtime,
            events,
            directory,
            state: Arc::default(),
            cancellation: Mutex::new(CancellationToken::new()),
            shutdown: CancellationToken::new(),
        }
    }
    pub fn snapshot(&self) -> Arc<UpdateSnapshot> {
        Arc::new(lock(&self.state).clone())
    }
    fn begin(&self, phase: UpdatePhase) -> Result<CancellationToken, String> {
        let mut cancellation = lock(&self.cancellation);
        if self.shutdown.is_cancelled() {
            return Err("Update service is stopped".into());
        }
        let mut state = lock(&self.state);
        if state.busy() {
            return Err("An update operation is already running".into());
        }
        if phase == UpdatePhase::Downloading && state.release.is_none() {
            return Err("Check for updates first".into());
        }
        *cancellation = self.shutdown.child_token();
        state.phase = phase;
        state.error = None;
        state.path = None;
        state.downloaded = 0;
        state.total = None;
        if phase == UpdatePhase::Checking {
            state.release = None;
        }
        state.revision += 1;
        let _ = self
            .events
            .try_send(RuntimeEvent::Update(Arc::new(state.clone())));
        Ok(cancellation.clone())
    }
    pub fn check(&self) -> Result<(), String> {
        let cancelled = self.begin(UpdatePhase::Checking)?;
        let state = self.state.clone();
        let events = self.events.clone();
        self.runtime.spawn(async move {
            let endpoints = (if rotor_common::native_app::PRODUCTION { rotor_updater::PRODUCTION_PREVIEW_ENDPOINTS } else { rotor_updater::PREVIEW_ENDPOINTS }).iter().map(|s| s.to_string()).collect::<Vec<_>>();
            let result = tokio::select! {
                _ = cancelled.cancelled() => Err("Update check cancelled".into()),
                result = rotor_updater::check(&endpoints, env!("CARGO_PKG_VERSION"), rotor_updater::target()) => result,
            };
            let snapshot = publish(&state, |state| match result {
                Ok(release) => { state.phase = if release.is_some() { UpdatePhase::Available } else { UpdatePhase::Current }; state.release = release.map(Arc::new); }
                Err(error) => { state.phase = UpdatePhase::Failed; state.error = Some(error); }
            });
            let _ = events.send(RuntimeEvent::Update(snapshot)).await;
        });
        Ok(())
    }
    pub fn download(&self) -> Result<(), String> {
        let cancelled = self.begin(UpdatePhase::Downloading)?;
        let release = lock(&self.state)
            .release
            .clone()
            .expect("begin validates release");
        let state = self.state.clone();
        let events = self.events.clone();
        let directory = self.directory.clone();
        self.runtime.spawn(async move {
            let result = rotor_updater::download(
                &release,
                &directory,
                &cancelled,
                |DownloadProgress { downloaded, total }| {
                    let snapshot = publish(&state, |state| {
                        state.downloaded = downloaded;
                        state.total = total;
                    });
                    let _ = events.try_send(RuntimeEvent::Update(snapshot));
                },
            )
            .await;
            let snapshot = publish(&state, |state| match result {
                Ok(path) => {
                    state.phase = UpdatePhase::Ready;
                    state.path = Some(path);
                }
                Err(error) => {
                    state.phase = UpdatePhase::Failed;
                    state.error = Some(error);
                }
            });
            let _ = events.send(RuntimeEvent::Update(snapshot)).await;
        });
        Ok(())
    }
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    pub fn install(&self, profile: PathBuf, flags: Vec<String>) -> Result<(), String> {
        let (path, release) = {
            let mut state = lock(&self.state);
            if self.shutdown.is_cancelled() {
                return Err("Update service is stopped".into());
            }
            if state.phase != UpdatePhase::Ready {
                return Err("Download and verify the update first".into());
            }
            let path = state.path.clone().ok_or("Update file is missing")?;
            let release = state.release.clone().ok_or("Update release is missing")?;
            state.phase = UpdatePhase::Installing;
            state.error = None;
            state.revision += 1;
            let _ = self
                .events
                .try_send(RuntimeEvent::Update(Arc::new(state.clone())));
            (path, release)
        };
        let state = self.state.clone();
        let events = self.events.clone();
        self.runtime.spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                #[cfg(target_os = "macos")]
                {
                    rotor_updater::launch_handoff(
                        &path,
                        &release.artifact.signature,
                        &release.version,
                        &profile,
                        &flags,
                    )
                }
                #[cfg(target_os = "windows")]
                {
                    rotor_updater::launch_verified_installer(
                        &path,
                        &release.artifact.signature,
                        |path| {
                            rotor_platform::installer::verify(
                                path,
                                rotor_common::native_app::PRODUCT_NAME,
                                &release.version,
                            )?;
                            rotor_platform::desktop::launch_update_installer(path, &profile, &flags)
                        },
                    )
                }
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|result| result);
            let snapshot = publish(&state, |state| match result {
                Ok(()) => state.phase = UpdatePhase::HandedOff,
                Err(error) => {
                    state.phase = UpdatePhase::Ready;
                    state.error = Some(error);
                }
            });
            let _ = events.send(RuntimeEvent::Update(snapshot)).await;
        });
        Ok(())
    }

    pub fn cancel(&self) {
        lock(&self.cancellation).cancel();
    }
    pub fn shutdown(&self) {
        self.shutdown.cancel();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serial_operations_require_a_release_and_stop_after_shutdown() {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (events, _) = async_channel::bounded(1);
        let service = UpdateService::new(runtime.handle().clone(), events, PathBuf::new());
        assert!(service.begin(UpdatePhase::Downloading).is_err());
        let token = service.begin(UpdatePhase::Checking).unwrap();
        assert!(service.begin(UpdatePhase::Checking).is_err());
        service.cancel();
        assert!(token.is_cancelled());
        // Cancellation completion precedes another operation.
        assert!(service.begin(UpdatePhase::Checking).is_err());
        publish(&service.state, |state| state.phase = UpdatePhase::Failed);
        let token = service.begin(UpdatePhase::Checking).unwrap();
        publish(&service.state, |state| {
            state.phase = UpdatePhase::Installing
        });
        assert!(service.begin(UpdatePhase::Checking).is_err());
        publish(&service.state, |state| state.phase = UpdatePhase::HandedOff);
        assert!(service.begin(UpdatePhase::Checking).is_err());
        service.shutdown();
        assert!(token.is_cancelled());
        publish(&service.state, |state| state.phase = UpdatePhase::Failed);
        assert!(service.begin(UpdatePhase::Checking).is_err());
    }
}
