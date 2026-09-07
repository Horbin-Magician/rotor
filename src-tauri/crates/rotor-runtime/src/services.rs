//! UI-independent use cases. Publishers wake a bounded receiver; the shell owns
//! window lookup and generation checks, never a worker or an IPC string router.
use crate::pins::{PinCommand, PinService};
use async_channel::{Receiver, Sender};
use image::{DynamicImage, RgbaImage};
use rotor_common::{Config, ConfigService, ResourceLocator};
use rotor_screenshot::{
    img_util::{self, TextResult},
    monitor::{self, MonitorConfig},
};
use rotor_searcher::{file_data::SearchIndexStatus, IndexState, QueryId, SearchBatch, Searcher};
use rotor_translator::engine::{self, EngineConfig, TranslateResult, TranslateStreamEvent};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex, MutexGuard,
    },
    time::Duration,
};
use tokio::{
    runtime::{Builder, Runtime},
    sync::{oneshot, Semaphore},
    task::JoinHandle,
};

const EVENT_CAPACITY: usize = 64;
const SETTINGS_CAPACITY: usize = 16;
const BACKGROUND_LIMIT: usize = 4;
static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OperationId(pub u64);

fn next_operation() -> OperationId {
    OperationId(NEXT_OPERATION.fetch_add(1, Ordering::Relaxed))
}

#[derive(Clone)]
pub struct CapturedMonitor {
    pub monitor: MonitorConfig,
    pub image: Arc<RgbaImage>,
}

pub struct CaptureBundle {
    pub monitors: Vec<CapturedMonitor>,
    pub windows: Vec<rotor_platform::sys_util::WindowRect>,
}

pub enum RuntimeEvent {
    Pin(crate::PinEvent),
    Search(SearchBatch),
    IndexState(IndexState),
    IndexStatus {
        id: OperationId,
        result: Result<SearchIndexStatus, String>,
    },
    SettingsSaved {
        id: OperationId,
        result: Result<Config, String>,
    },
    FileOpened {
        id: OperationId,
        result: Result<(), String>,
    },
    SelectionFinished {
        id: OperationId,
        result: Result<rotor_platform::clipboard::SelectedText, String>,
    },
    Translation {
        id: OperationId,
        event: TranslateStreamEvent,
    },
    TranslationFinished {
        id: OperationId,
        result: Result<TranslateResult, String>,
    },
    CaptureFinished {
        id: OperationId,
        result: Result<CaptureBundle, String>,
    },
    OcrFinished {
        id: OperationId,
        pin_id: u32,
        revision: u64,
        result: Result<Vec<TextResult>, String>,
    },
}

enum SettingsCommand {
    Save {
        id: OperationId,
        changes: Vec<(String, String)>,
    },
    Flush(oneshot::Sender<()>),
}

#[derive(Clone, Copy)]
pub struct ServiceOptions {
    pub index_files: bool,
}
impl Default for ServiceOptions {
    fn default() -> Self {
        Self { index_files: true }
    }
}

pub struct Services {
    runtime: Option<Runtime>,
    published_config: Arc<Mutex<Config>>,
    resources: Option<ResourceLocator>,
    events: Sender<RuntimeEvent>,
    settings: Sender<SettingsCommand>,
    settings_worker: Option<JoinHandle<()>>,
    pins: PinService,
    searcher: Option<Searcher>,
    translation: Mutex<Option<JoinHandle<()>>>,
    translation_id: Arc<AtomicU64>,
    selection: Mutex<Option<Arc<AtomicBool>>>,
    capture_id: Arc<AtomicU64>,
    background: Mutex<Vec<JoinHandle<()>>>,
    slots: Arc<Semaphore>,
    canvas_fonts: Arc<Mutex<Option<Arc<rotor_canvas::Renderer>>>>,
    stopped: Arc<AtomicBool>,
}

impl Services {
    pub fn new(
        config: Arc<Mutex<ConfigService>>,
        resources: Option<ResourceLocator>,
        options: ServiceOptions,
    ) -> Result<(Self, Receiver<RuntimeEvent>), String> {
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(BACKGROUND_LIMIT + 2)
            .thread_name("rotor-worker")
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        let (events, receiver) = async_channel::bounded(EVENT_CAPACITY);
        let (settings, settings_receiver) = async_channel::bounded(SETTINGS_CAPACITY);
        let data_directory = lock(&config)
            .data_directory()
            .ok_or("configuration data directory unavailable")?
            .to_path_buf();
        let pins = PinService::new(&runtime, data_directory, events.clone());
        let published_config = Arc::new(Mutex::new(lock(&config).get_all()));
        let settings_worker = runtime.spawn(settings_loop(
            config,
            published_config.clone(),
            settings_receiver,
            events.clone(),
        ));
        let searcher = options.index_files.then(|| {
            let results = events.clone();
            let state_events = events.clone();
            Searcher::new(
                move |batch| {
                    let _ = results.send_blocking(RuntimeEvent::Search(batch));
                },
                Some(Box::new(move |state| {
                    let _ = state_events.send_blocking(RuntimeEvent::IndexState(state));
                })),
            )
        });
        Ok((
            Self {
                runtime: Some(runtime),
                published_config,
                resources,
                events,
                settings,
                settings_worker: Some(settings_worker),
                pins,
                searcher,
                translation: Mutex::new(None),
                translation_id: Arc::new(AtomicU64::new(0)),
                selection: Mutex::new(None),
                capture_id: Arc::new(AtomicU64::new(0)),
                background: Mutex::new(Vec::new()),
                slots: Arc::new(Semaphore::new(BACKGROUND_LIMIT)),
                canvas_fonts: Arc::new(Mutex::new(None)),
                stopped: Arc::new(AtomicBool::new(false)),
            },
            receiver,
        ))
    }

    pub fn settings(&self) -> Config {
        lock(&self.published_config).clone()
    }

    pub async fn render_canvas(
        &self,
        image: Arc<RgbaImage>,
        scene: rotor_canvas::Scene,
        output: rotor_canvas::ImageSize,
    ) -> Result<Arc<RgbaImage>, String> {
        self.ensure_running()?;
        let permit = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "canvas rendering service is closed")?;
        self.ensure_running()?;
        let resources = self.resources.clone();
        let fonts = self.canvas_fonts.clone();
        self.runtime()
            .spawn_blocking(move || {
                let _permit = permit;
                if scene.has_text() {
                    let renderer = {
                        let mut loaded = lock(&fonts);
                        if loaded.is_none() {
                            let path = resources
                                .as_ref()
                                .ok_or("Annotation resources are unavailable")?
                                .resolve(Path::new("fonts/NotoSansCJKsc-Regular.otf"))
                                .map_err(|error| error.to_string())?;
                            let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
                            *loaded =
                                Some(Arc::new(rotor_canvas::Renderer::with_font(bytes, true)?));
                        }
                        loaded.as_ref().unwrap().clone()
                    };
                    renderer.render(&image, &scene, output).map(Arc::new)
                } else {
                    rotor_canvas::Renderer::without_fonts()
                        .render(&image, &scene, output)
                        .map(Arc::new)
                }
            })
            .await
            .map_err(|error| error.to_string())?
    }

    pub fn restore_pins(&self) -> Result<OperationId, String> {
        self.ensure_running()?;
        let id = next_operation();
        self.pins.submit(PinCommand::Restore { id })?;
        Ok(id)
    }
    pub fn create_pin(
        &self,
        image: Arc<RgbaImage>,
        config: crate::ShotterConfig,
    ) -> Result<OperationId, String> {
        self.ensure_running()?;
        let id = next_operation();
        self.pins.submit(PinCommand::Create { id, image, config })?;
        Ok(id)
    }
    pub fn create_pin_from_capture(
        &self,
        image: Arc<RgbaImage>,
        config: crate::ShotterConfig,
    ) -> Result<OperationId, String> {
        self.ensure_running()?;
        let id = next_operation();
        self.pins
            .submit(PinCommand::CreateFromCapture { id, image, config })?;
        Ok(id)
    }
    pub fn update_pin(
        &self,
        pin_id: u32,
        config: crate::ShotterConfig,
    ) -> Result<OperationId, String> {
        self.ensure_running()?;
        let id = next_operation();
        self.pins
            .submit(PinCommand::Update { id, pin_id, config })?;
        Ok(id)
    }
    pub fn delete_pin(&self, pin_id: u32) -> Result<OperationId, String> {
        self.ensure_running()?;
        let id = next_operation();
        self.pins.submit(PinCommand::Delete { id, pin_id })?;
        Ok(id)
    }
    pub fn export_pin(
        &self,
        pin_id: Option<u32>,
        image: Arc<RgbaImage>,
        config: crate::ShotterConfig,
        target: crate::PinExportTarget,
    ) -> Result<OperationId, String> {
        self.ensure_running()?;
        let id = next_operation();
        self.pins.submit(PinCommand::Export {
            id,
            pin_id,
            image,
            config,
            target,
        })?;
        Ok(id)
    }

    pub fn export_pin_frame(
        &self,
        pin_id: Option<u32>,
        image: Arc<RgbaImage>,
        target: crate::PinExportTarget,
    ) -> Result<OperationId, String> {
        self.ensure_running()?;
        let id = next_operation();
        self.pins.submit(PinCommand::ExportFrame {
            id,
            pin_id,
            image,
            target,
        })?;
        Ok(id)
    }
    pub async fn flush_pins(&self) -> Result<(), String> {
        let (sender, receiver) = oneshot::channel();
        self.pins
            .sender
            .send(PinCommand::Flush(sender))
            .await
            .map_err(|_| "pin persistence queue is closed")?;
        receiver
            .await
            .map_err(|_| "pin persistence worker stopped".into())
    }

    /// Accepted patches execute serially, including when fields are edited fast.
    pub fn save_settings(&self, changes: Vec<(String, String)>) -> Result<OperationId, String> {
        self.ensure_running()?;
        let id = next_operation();
        self.settings
            .try_send(SettingsCommand::Save { id, changes })
            .map_err(|_| "configuration queue is busy or closed".to_string())?;
        Ok(id)
    }

    pub async fn flush_settings(&self) -> Result<(), String> {
        let (sender, receiver) = oneshot::channel();
        self.settings
            .send(SettingsCommand::Flush(sender))
            .await
            .map_err(|_| "configuration queue is closed".to_string())?;
        receiver
            .await
            .map_err(|_| "configuration worker stopped".to_string())
    }

    pub fn search(&self, query: String) -> Result<QueryId, String> {
        self.ensure_running()?;
        self.searcher
            .as_ref()
            .ok_or("file indexing is disabled")?
            .find(query)
            .map_err(|error| error.to_string())
    }

    pub fn update_search(&self) {
        if let Some(searcher) = &self.searcher {
            searcher.update();
        }
    }
    pub fn release_search(&self) {
        if let Some(searcher) = &self.searcher {
            searcher.release();
        }
    }
    pub fn rebuild_search(&self) {
        if let Some(searcher) = &self.searcher {
            searcher.rebuild_index();
        }
    }

    pub fn request_index_status(&self) -> Result<OperationId, String> {
        let reader = self
            .searcher
            .as_ref()
            .ok_or("file indexing is disabled")?
            .index_status_reader();
        self.spawn_job(
            move || Ok(reader.index_status()),
            None,
            |id, result| RuntimeEvent::IndexStatus { id, result },
        )
    }

    pub fn capture_selection(&self) -> Result<OperationId, String> {
        let mut selection = lock(&self.selection);
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_cancelled = cancelled.clone();
        let id = self.spawn_job(
            move || {
                rotor_platform::clipboard::capture_selected_text(|| {
                    worker_cancelled.load(Ordering::Acquire)
                })
            },
            None,
            |id, result| RuntimeEvent::SelectionFinished { id, result },
        )?;
        if let Some(previous) = selection.replace(cancelled) {
            previous.store(true, Ordering::Release);
        }
        Ok(id)
    }

    pub fn cancel_selection(&self) {
        if let Some(cancelled) = lock(&self.selection).take() {
            cancelled.store(true, Ordering::Release);
        }
    }

    pub fn open_file(&self, path: String, as_admin: bool) -> Result<OperationId, String> {
        self.spawn_job(
            move || {
                if as_admin {
                    rotor_platform::file_util::open_file_as_admin(path)
                } else {
                    rotor_platform::file_util::open_file(path)
                }
                .map_err(|error| error.to_string())
            },
            None,
            |id, result| RuntimeEvent::FileOpened { id, result },
        )
    }

    pub fn translate(&self, text: String) -> Result<OperationId, String> {
        self.ensure_running()?;
        if text.trim().is_empty() {
            return Err("translation text is empty".into());
        }
        let config = EngineConfig::from_config(&self.settings());
        let id = next_operation();
        let mut previous = lock(&self.translation);
        self.ensure_running()?;
        self.translation_id.store(id.0, Ordering::Release);
        if let Some(task) = previous.take() {
            task.abort();
        }
        let current = self.translation_id.clone();
        let events = self.events.clone();
        *previous = Some(self.runtime().spawn(async move {
            let progress_events = events.clone();
            let progress_id = current.clone();
            let result = engine::translate_with_config(&config, &text, move |event| {
                if progress_id.load(Ordering::Acquire) == id.0 {
                    // A bounded callback sink provides backpressure. The UI
                    // drains it independently of window visibility.
                    let _ = progress_events.send_blocking(RuntimeEvent::Translation { id, event });
                }
            })
            .await
            .map_err(|error| redact_error(error.to_string(), &config));
            if current.load(Ordering::Acquire) == id.0 {
                let _ = events
                    .send(RuntimeEvent::TranslationFinished { id, result })
                    .await;
            }
        }));
        Ok(id)
    }

    pub fn cancel_translation(&self) {
        let mut task = lock(&self.translation);
        self.translation_id
            .store(next_operation().0, Ordering::Release);
        if let Some(task) = task.take() {
            task.abort();
        }
    }

    pub fn cancel_translation_request(&self, id: OperationId) {
        let mut task = lock(&self.translation);
        if self.translation_id.load(Ordering::Acquire) == id.0 {
            self.translation_id
                .store(next_operation().0, Ordering::Release);
            if let Some(task) = task.take() {
                task.abort();
            }
        }
    }

    pub fn capture(&self) -> Result<OperationId, String> {
        self.spawn_job(
            capture_monitors,
            Some(self.capture_id.clone()),
            |id, result| RuntimeEvent::CaptureFinished { id, result },
        )
    }

    pub fn cancel_capture(&self) {
        self.capture_id.store(next_operation().0, Ordering::Release);
    }

    pub fn recognize_text(
        &self,
        pin_id: u32,
        revision: u64,
        image: Arc<RgbaImage>,
    ) -> Result<OperationId, String> {
        let root = self
            .resources
            .as_ref()
            .ok_or("OCR resources are unavailable")?
            .resolve(Path::new("model"))
            .map_err(|error| error.to_string())?;
        self.spawn_job(
            move || {
                img_util::img2text(
                    &root,
                    &DynamicImage::ImageRgba8(Arc::unwrap_or_clone(image)),
                )
                .map_err(|error| error.to_string())
            },
            None,
            move |id, result| RuntimeEvent::OcrFinished {
                id,
                pin_id,
                revision,
                result,
            },
        )
    }

    fn spawn_job<T, F, E>(
        &self,
        work: F,
        current: Option<Arc<AtomicU64>>,
        event: E,
    ) -> Result<OperationId, String>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, String> + Send + 'static,
        E: FnOnce(OperationId, Result<T, String>) -> RuntimeEvent + Send + 'static,
    {
        self.ensure_running()?;
        let permit = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| "background work queue is busy".to_string())?;
        let id = next_operation();
        if let Some(current) = &current {
            current.store(id.0, Ordering::Release);
        }
        let events = self.events.clone();
        let stopped = self.stopped.clone();
        let task = self.runtime().spawn(async move {
            let _permit = permit;
            if stopped.load(Ordering::Acquire) {
                return;
            }
            let result = tokio::task::spawn_blocking(work)
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result);
            if current
                .as_ref()
                .is_none_or(|current| current.load(Ordering::Acquire) == id.0)
            {
                let _ = events.send(event(id, result)).await;
            }
        });
        let mut tasks = lock(&self.background);
        tasks.retain(|task| !task.is_finished());
        tasks.push(task);
        Ok(id)
    }

    fn ensure_running(&self) -> Result<(), String> {
        if self.stopped.load(Ordering::Acquire) {
            Err("application services are stopped".into())
        } else {
            Ok(())
        }
    }

    fn runtime(&self) -> &Runtime {
        self.runtime
            .as_ref()
            .expect("runtime exists until service drop")
    }

    pub fn shutdown(&self) {
        self.shutdown_with_pin_updates(Vec::new());
    }

    pub fn shutdown_with_pin_updates(&self, updates: Vec<(u32, crate::ShotterConfig)>) {
        if self.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        *lock(&self.pins.final_updates) = updates;
        self.cancel_translation();
        self.slots.close();
        self.cancel_selection();
        self.cancel_capture();
        if let Some(searcher) = &self.searcher {
            searcher.shutdown();
        }
        self.settings.close();
        self.pins.sender.close();
        // Unblock callback publishers before waiting for accepted disk writes.
        self.events.close();
        for task in lock(&self.background).drain(..) {
            task.abort();
        }
    }
}

impl Drop for Services {
    fn drop(&mut self) {
        self.shutdown();
        if let Some(runtime) = self.runtime.take() {
            if let Some(worker) = self.settings_worker.take() {
                let flushed = runtime
                    .block_on(async { tokio::time::timeout(Duration::from_secs(2), worker).await });
                if !matches!(flushed, Ok(Ok(()))) {
                    log::error!("Configuration worker did not finish during shutdown");
                }
            }
            if let Some(worker) = self.pins.worker.take() {
                let flushed = runtime
                    .block_on(async { tokio::time::timeout(Duration::from_secs(2), worker).await });
                if !matches!(flushed, Ok(Ok(()))) {
                    log::error!("Pin persistence worker did not finish during shutdown");
                }
            }
            runtime.shutdown_timeout(Duration::from_secs(2));
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

async fn settings_loop(
    config: Arc<Mutex<ConfigService>>,
    published: Arc<Mutex<Config>>,
    receiver: Receiver<SettingsCommand>,
    events: Sender<RuntimeEvent>,
) {
    while let Ok(command) = receiver.recv().await {
        match command {
            SettingsCommand::Save { id, changes } => {
                let config = config.clone();
                let result = tokio::task::spawn_blocking(move || {
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
                let _ = events
                    .send(RuntimeEvent::SettingsSaved { id, result })
                    .await;
            }
            SettingsCommand::Flush(sender) => {
                let _ = sender.send(());
            }
        }
    }
}

fn capture_monitors() -> Result<CaptureBundle, String> {
    rotor_platform::overlay::settle_desktop()?;
    let before =
        monitor::sorted_configs(monitor::current_configs().map_err(|error| error.to_string())?);
    let windows = rotor_platform::sys_util::get_all_window_rect().unwrap_or_else(|error| {
        log::warn!("Capture window rectangles: {error}");
        Vec::new()
    });
    let mut images = rotor_screenshot::capture_images()?;
    let after =
        monitor::sorted_configs(monitor::current_configs().map_err(|error| error.to_string())?);
    if before != after {
        return Err("display topology changed during capture; retry screenshot".into());
    }
    let monitors = before
        .into_iter()
        .map(|monitor| {
            let image = images
                .remove(&monitor::mask_label(monitor.id))
                .ok_or("monitor image is missing")?;
            if image.width() != monitor.width || image.height() != monitor.height {
                return Err("capture dimensions differ from monitor dimensions".into());
            }
            Ok(CapturedMonitor {
                monitor,
                image: Arc::new(image),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(CaptureBundle { monitors, windows })
}

fn redact_error(message: String, config: &EngineConfig) -> String {
    config.redact_error(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };

    fn create(config: ConfigService) -> (Services, Receiver<RuntimeEvent>) {
        Services::new(
            Arc::new(Mutex::new(config)),
            None,
            ServiceOptions { index_files: false },
        )
        .unwrap()
    }

    #[test]
    fn shutdown_drains_accepted_configuration_writes_in_order() {
        let directory = tempfile::tempdir().unwrap();
        let (services, _events) = create(ConfigService::load_from(directory.path()).unwrap());
        services
            .save_settings(vec![("theme".into(), "1".into())])
            .unwrap();
        services
            .save_settings(vec![
                ("theme".into(), "2".into()),
                ("unknown".into(), "kept".into()),
            ])
            .unwrap();
        services.shutdown();
        assert!(services.save_settings(vec![]).is_err());
        drop(services);
        let config = ConfigService::load_from(directory.path()).unwrap();
        assert_eq!(config.get_user("theme").map(String::as_str), Some("2"));
        assert_eq!(config.get_user("unknown").map(String::as_str), Some("kept"));
    }

    fn pin_config() -> crate::ShotterConfig {
        crate::ShotterConfig {
            monitor_pos: (0, 0),
            monitor_size: (1920, 1080),
            rect: (0, 0, 2, 3),
            image_rect: Some((0, 0, 2, 3)),
            offset: (0, 0),
            zoom_factor: 100,
            mask_label: "ssmask-1".into(),
            minimized: false,
        }
    }

    #[test]
    fn shutdown_drains_accepted_pin_creation_without_an_event_consumer() {
        let directory = tempfile::tempdir().unwrap();
        let (services, _events) = create(ConfigService::load_from(directory.path()).unwrap());
        services
            .create_pin(Arc::new(RgbaImage::new(2, 3)), pin_config())
            .unwrap();
        services.shutdown();
        assert!(services.restore_pins().is_err());
        drop(services);
        let store = rotor_screenshot::pin_store::PinStore::load_from(directory.path()).unwrap();
        let (pins, warnings) = store.load_pins();
        assert!(warnings.is_empty());
        assert_eq!(pins.len(), 1);
    }

    #[test]
    fn pin_update_and_delete_follow_submission_order() {
        let directory = tempfile::tempdir().unwrap();
        let (services, events) = create(ConfigService::load_from(directory.path()).unwrap());
        let request = services
            .create_pin(Arc::new(RgbaImage::new(2, 3)), pin_config())
            .unwrap();
        let created = services.runtime().block_on(async {
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .unwrap()
                .unwrap()
        });
        let RuntimeEvent::Pin(crate::PinEvent::Created { id, result }) = created else {
            panic!("expected created pin");
        };
        assert_eq!(id, request);
        let pin = result.unwrap();
        let mut config = pin.config;
        config.offset = (5, -8);
        let updated = services.update_pin(pin.id, config).unwrap();
        let deleted = services.delete_pin(pin.id).unwrap();
        services.runtime().block_on(services.flush_pins()).unwrap();
        assert!(
            matches!(events.try_recv().unwrap(), RuntimeEvent::Pin(crate::PinEvent::Updated { id, result: Ok(_), .. }) if id == updated)
        );
        assert!(
            matches!(events.try_recv().unwrap(), RuntimeEvent::Pin(crate::PinEvent::Deleted { id, result: Ok(()), .. }) if id == deleted)
        );
        assert!(
            rotor_screenshot::pin_store::PinStore::load_from(directory.path())
                .unwrap()
                .load_pins()
                .0
                .is_empty()
        );
    }

    #[test]
    fn pin_export_failure_keeps_record_and_success_removes_it() {
        let directory = tempfile::tempdir().unwrap();
        let (services, events) = create(ConfigService::load_from(directory.path()).unwrap());
        let image = Arc::new(RgbaImage::from_pixel(2, 3, image::Rgba([7, 8, 9, 128])));
        services.create_pin(image.clone(), pin_config()).unwrap();
        let receive = || {
            services.runtime().block_on(async {
                tokio::time::timeout(Duration::from_secs(3), events.recv())
                    .await
                    .unwrap()
                    .unwrap()
            })
        };
        let RuntimeEvent::Pin(crate::PinEvent::Created {
            result: Ok(pin), ..
        }) = receive()
        else {
            panic!("expected created pin");
        };
        let invalid = directory.path().join("directory.png");
        std::fs::create_dir(&invalid).unwrap();
        let failed = services
            .export_pin(
                Some(pin.id),
                image.clone(),
                pin_config(),
                crate::PinExportTarget::File(invalid),
            )
            .unwrap();
        assert!(
            matches!(receive(), RuntimeEvent::Pin(crate::PinEvent::Exported { id, result: Err(_) }) if id == failed)
        );
        assert_eq!(
            rotor_screenshot::pin_store::PinStore::load_from(directory.path())
                .unwrap()
                .load_pins()
                .0
                .len(),
            1
        );
        let output = directory.path().join("export.png");
        let saved = services
            .export_pin(
                Some(pin.id),
                image.clone(),
                pin_config(),
                crate::PinExportTarget::File(output.clone()),
            )
            .unwrap();
        assert!(
            matches!(receive(), RuntimeEvent::Pin(crate::PinEvent::Exported { id, result: Ok(()) }) if id == saved)
        );
        assert_eq!(image::open(output).unwrap().into_rgba8(), *image);
        assert!(
            rotor_screenshot::pin_store::PinStore::load_from(directory.path())
                .unwrap()
                .load_pins()
                .0
                .is_empty()
        );
    }

    #[test]
    fn confirmed_capture_is_cropped_and_persisted_before_shutdown() {
        let directory = tempfile::tempdir().unwrap();
        let (services, _events) = create(ConfigService::load_from(directory.path()).unwrap());
        let image = Arc::new(RgbaImage::from_fn(4, 4, |x, y| {
            image::Rgba([x as u8, y as u8, 42, 255])
        }));
        let mut config = pin_config();
        config.monitor_size = (4, 4);
        config.rect = (1, 1, 2, 2);
        config.image_rect = Some((1, 1, 2, 2));
        services
            .create_pin_from_capture(image.clone(), config)
            .unwrap();
        services.shutdown();
        drop(services);
        let (pins, warnings) = rotor_screenshot::pin_store::PinStore::load_from(directory.path())
            .unwrap()
            .load_pins();
        assert!(warnings.is_empty());
        assert_eq!(pins.len(), 1);
        assert_eq!(
            *pins[0].image,
            image::imageops::crop_imm(image.as_ref(), 1, 1, 2, 2).to_image()
        );
        assert_eq!(pins[0].config.image_rect, Some((1, 1, 2, 2)));
    }

    #[test]
    fn exported_canvas_frame_keeps_its_viewport_size_and_pixels() {
        let directory = tempfile::tempdir().unwrap();
        let (services, events) = create(ConfigService::load_from(directory.path()).unwrap());
        let source = Arc::new(RgbaImage::from_pixel(2, 3, image::Rgba([20, 40, 80, 128])));
        services.create_pin(source.clone(), pin_config()).unwrap();
        let receive = || {
            services.runtime().block_on(async {
                tokio::time::timeout(Duration::from_secs(3), events.recv())
                    .await
                    .unwrap()
                    .unwrap()
            })
        };
        let RuntimeEvent::Pin(crate::PinEvent::Created {
            result: Ok(pin), ..
        }) = receive()
        else {
            panic!("expected created pin");
        };
        let scene = rotor_canvas::Document::new(
            rotor_canvas::ImageSize {
                width: 2,
                height: 3,
            },
            rotor_canvas::ImageRect {
                x: 0,
                y: 0,
                width: 2,
                height: 3,
            },
        )
        .unwrap();
        let frame = services
            .runtime()
            .block_on(services.render_canvas(
                source,
                scene.scene().clone(),
                rotor_canvas::ImageSize {
                    width: 4,
                    height: 6,
                },
            ))
            .unwrap();
        let path = directory.path().join("scaled.png");
        let id = services
            .export_pin_frame(
                Some(pin.id),
                frame.clone(),
                crate::PinExportTarget::File(path.clone()),
            )
            .unwrap();
        assert!(
            matches!(receive(), RuntimeEvent::Pin(crate::PinEvent::Exported { id: returned, result: Ok(()) }) if returned == id)
        );
        let image = image::open(path).unwrap().into_rgba8();
        assert_eq!(image.dimensions(), (4, 6));
        assert_eq!(image, *frame);
    }

    #[test]
    fn shutdown_wakes_canvas_requests_waiting_for_capacity() {
        use std::{
            future::Future,
            task::{Context, Waker},
        };
        let directory = tempfile::tempdir().unwrap();
        let (services, _events) = create(ConfigService::load_from(directory.path()).unwrap());
        let _permits: Vec<_> = (0..BACKGROUND_LIMIT)
            .map(|_| services.slots.clone().try_acquire_owned().unwrap())
            .collect();
        let scene = rotor_canvas::Document::new(
            rotor_canvas::ImageSize {
                width: 2,
                height: 3,
            },
            rotor_canvas::ImageRect {
                x: 0,
                y: 0,
                width: 2,
                height: 3,
            },
        )
        .unwrap();
        let mut request = Box::pin(services.render_canvas(
            Arc::new(RgbaImage::new(2, 3)),
            scene.scene().clone(),
            scene.scene().size,
        ));
        assert!(request
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending());
        services.shutdown();
        assert!(services.runtime().block_on(request).is_err());
    }

    #[test]
    fn final_pin_snapshot_bypasses_full_command_and_event_queues() {
        let directory = tempfile::tempdir().unwrap();
        let (services, events) = create(ConfigService::load_from(directory.path()).unwrap());
        services
            .create_pin(Arc::new(RgbaImage::new(2, 3)), pin_config())
            .unwrap();
        let created = services.runtime().block_on(async {
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .unwrap()
                .unwrap()
        });
        let RuntimeEvent::Pin(crate::PinEvent::Created {
            result: Ok(pin), ..
        }) = created
        else {
            panic!("expected created pin");
        };
        let queued = EVENT_CAPACITY + services.pins.sender.capacity().unwrap() + 1;
        services.runtime().block_on(async {
            tokio::time::timeout(Duration::from_secs(5), async {
                for _ in 0..queued {
                    services
                        .pins
                        .sender
                        .send(PinCommand::Restore {
                            id: next_operation(),
                        })
                        .await
                        .unwrap();
                }
            })
            .await
            .unwrap();
        });
        assert!(services.pins.sender.is_full());
        assert!(events.is_full());
        let mut latest = pin.config;
        latest.offset = (123, -321);
        services.shutdown_with_pin_updates(vec![(pin.id, latest)]);
        drop(services);
        let (pins, warnings) = rotor_screenshot::pin_store::PinStore::load_from(directory.path())
            .unwrap()
            .load_pins();
        assert!(warnings.is_empty());
        assert_eq!(pins[0].config.offset, (123, -321));
    }

    #[test]
    fn saturated_background_capacity_rejects_capture_without_superseding() {
        let directory = tempfile::tempdir().unwrap();
        let (services, _events) = create(ConfigService::load_from(directory.path()).unwrap());
        let _permit = services
            .slots
            .clone()
            .try_acquire_many_owned(BACKGROUND_LIMIT as u32)
            .unwrap();
        assert!(services.capture().is_err());
        assert_eq!(services.capture_id.load(Ordering::Acquire), 0);
    }

    #[test]
    fn translation_uses_isolated_configuration_and_keeps_request_identity() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("test HTTP server: {error}"),
                }
            };
            // Windows accepted sockets can inherit the listener's nonblocking
            // mode. Use bounded blocking I/O for the HTTP fixture itself.
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0; 1024];
            while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                let count = stream.read(&mut buffer).unwrap();
                assert!(count > 0);
                request.extend_from_slice(&buffer[..count]);
            }
            assert!(String::from_utf8_lossy(&request).contains("text=hello%20world"));
            let body = "{\"translated\":\"你好\"}";
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
        });
        let directory = tempfile::tempdir().unwrap();
        let mut config = ConfigService::load_from(directory.path()).unwrap();
        config
            .set_many([
                ("translator_engine".into(), "custom".into()),
                (
                    "translator_custom_url".into(),
                    format!("http://{address}/?text={{text}}"),
                ),
            ])
            .unwrap();
        let (services, events) = create(config);
        let id = services.translate("hello world".into()).unwrap();
        let event = services.runtime().block_on(async {
            tokio::time::timeout(Duration::from_secs(3), events.recv())
                .await
                .unwrap()
                .unwrap()
        });
        match event {
            RuntimeEvent::TranslationFinished {
                id: returned,
                result,
            } => {
                assert_eq!(returned, id);
                assert_eq!(result.unwrap().translated, "你好");
            }
            _ => panic!("expected completed custom translation"),
        }
        server.join().unwrap();
    }
}
