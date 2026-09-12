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
        Arc, Mutex, MutexGuard, Weak,
    },
    time::Duration,
};
use tokio::{
    runtime::{Builder, Runtime},
    sync::{oneshot, Semaphore},
    task::JoinHandle,
};

mod settings_worker;
pub use settings_worker::SettingsCoordination;
use settings_worker::{
    merge_settings_changes, settings_loop, SettingsCommand, SettingsPatch, SettingsTail,
};

mod capture_worker;
use capture_worker::{CaptureRequest, CaptureWorker};

const EVENT_CAPACITY: usize = 64;
const SETTINGS_CAPACITY: usize = 16;
const BACKGROUND_LIMIT: usize = 4;
static NEXT_OPERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OperationId(pub u64);

fn next_operation() -> OperationId {
    OperationId(NEXT_OPERATION.fetch_add(1, Ordering::Relaxed))
}

pub struct CapturedMonitor {
    pub monitor: MonitorConfig,
    pub image: monitor::BgraCapture,
}

pub struct CaptureBundle {
    pub monitors: Vec<CapturedMonitor>,
    pub windows: Vec<rotor_platform::sys_util::WindowRect>,
}

pub enum RuntimeEvent {
    Update(Arc<crate::UpdateSnapshot>),
    Overview {
        id: OperationId,
        result: Result<Overview, String>,
    },
    StartupChanged {
        id: OperationId,
        result: Result<bool, String>,
    },
    SettingsCoordination(SettingsCoordination),
    QuickFinished {
        id: OperationId,
        action_id: String,
        result: Result<(), String>,
    },
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
    /// The capture topology is known; the shell can prepare hidden windows while
    /// pixels are captured. This is not a ready frame or permission to show it.
    CapturePreparing {
        id: OperationId,
        monitors: Vec<MonitorConfig>,
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

pub struct Overview {
    pub version: &'static str,
    pub platform: &'static str,
    pub architecture: &'static str,
    pub data_directory: String,
    pub resident_bytes: Result<u64, String>,
    pub permissions: Vec<rotor_platform::sys_util::PermissionStatus>,
    pub autostart: Result<bool, String>,
    pub ocr_loaded: Option<bool>,
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
    updates: crate::updates::UpdateService,
    runtime: Option<Runtime>,
    published_config: Arc<Mutex<Config>>,
    resources: Option<ResourceLocator>,
    events: Sender<RuntimeEvent>,
    settings: Sender<SettingsCommand>,
    settings_tail: SettingsTail,
    settings_worker: Option<JoinHandle<()>>,
    pins: PinService,
    searcher: Option<Searcher>,
    translation: Mutex<Option<JoinHandle<()>>>,
    translation_id: Arc<AtomicU64>,
    selection: Mutex<Option<Arc<AtomicBool>>>,
    capture_id: Arc<AtomicU64>,
    capture_worker: CaptureWorker,
    background: Mutex<Vec<JoinHandle<()>>>,
    slots: Arc<Semaphore>,
    canvas_fonts: Arc<Mutex<Option<Arc<rotor_canvas::Renderer>>>>,
    coordinate_shortcuts: Arc<AtomicBool>,
    development_shortcuts: AtomicBool,
    shortcut_recording: Arc<crate::shortcuts::ShortcutRecording>,
    startup_warning: Option<String>,
    data_directory: std::path::PathBuf,
    startup_flags: Mutex<Vec<String>>,
    stopped: Arc<AtomicBool>,
}

impl Services {
    pub fn new(
        config: Arc<Mutex<ConfigService>>,
        resources: Option<ResourceLocator>,
        options: ServiceOptions,
    ) -> Result<(Self, Receiver<RuntimeEvent>), String> {
        let startup_warning = crate::quick::actions_from_config(&lock(&config).get_all()).err();
        let runtime = Builder::new_multi_thread()
            .worker_threads(2)
            .max_blocking_threads(BACKGROUND_LIMIT + 2)
            .thread_name("rotor-worker")
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        let (events, receiver) = async_channel::bounded(EVENT_CAPACITY);
        let capture_id = Arc::new(AtomicU64::new(0));
        let capture_worker = CaptureWorker::new(events.clone(), capture_id.clone())?;
        let (settings, settings_receiver) = async_channel::bounded(SETTINGS_CAPACITY);
        let data_directory = lock(&config)
            .data_directory()
            .ok_or("configuration data directory unavailable")?
            .to_path_buf();
        let pins = PinService::new(&runtime, data_directory.clone(), events.clone());
        let published_config = Arc::new(Mutex::new(lock(&config).get_all()));
        let coordinate_shortcuts = Arc::new(AtomicBool::new(false));
        let settings_worker = runtime.spawn(settings_loop(
            config,
            published_config.clone(),
            settings_receiver,
            events.clone(),
            coordinate_shortcuts.clone(),
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
                updates: crate::updates::UpdateService::new(
                    runtime.handle().clone(),
                    events.clone(),
                    data_directory.join("updates"),
                ),
                runtime: Some(runtime),
                published_config,
                resources,
                events,
                settings,
                settings_tail: Mutex::new(None),
                settings_worker: Some(settings_worker),
                pins,
                searcher,
                translation: Mutex::new(None),
                translation_id: Arc::new(AtomicU64::new(0)),
                selection: Mutex::new(None),
                capture_id,
                capture_worker,
                background: Mutex::new(Vec::new()),
                slots: Arc::new(Semaphore::new(BACKGROUND_LIMIT)),
                canvas_fonts: Arc::new(Mutex::new(None)),
                coordinate_shortcuts,
                development_shortcuts: AtomicBool::new(true),
                shortcut_recording: Arc::new(crate::shortcuts::ShortcutRecording::default()),
                startup_warning,
                data_directory,
                startup_flags: Mutex::new(Vec::new()),
                stopped: Arc::new(AtomicBool::new(false)),
            },
            receiver,
        ))
    }

    pub fn update_snapshot(&self) -> Arc<crate::UpdateSnapshot> {
        self.updates.snapshot()
    }
    pub fn check_updates(&self) -> Result<(), String> {
        self.updates.check()
    }
    pub fn download_update(&self) -> Result<(), String> {
        self.updates.download()
    }
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    pub fn install_update(&self) -> Result<(), String> {
        self.updates.install(
            self.data_directory.clone(),
            lock(&self.startup_flags).clone(),
        )
    }
    pub fn cancel_update(&self) {
        self.updates.cancel();
    }

    pub fn settings(&self) -> Config {
        lock(&self.published_config).clone()
    }
    pub fn startup_warning(&self) -> Option<String> {
        self.startup_warning.clone()
    }
    pub fn configure_startup_flags(&self, flags: Vec<String>) {
        *lock(&self.startup_flags) = flags;
    }
    fn startup_parameters(&self) -> Result<(std::path::PathBuf, Vec<String>), String> {
        let executable = std::env::current_exe().map_err(|error| error.to_string())?;
        let mut args = vec![
            "--background".into(),
            "--data-dir".into(),
            self.data_directory
                .to_str()
                .ok_or("Data directory is not Unicode")?
                .into(),
        ];
        if let Some(resources) = &self.resources {
            args.extend([
                "--resource-dir".into(),
                resources
                    .root()
                    .to_str()
                    .ok_or("Resource directory is not Unicode")?
                    .into(),
            ]);
        }
        args.extend(lock(&self.startup_flags).iter().cloned());
        Ok((executable, args))
    }
    pub fn request_overview(&self) -> Result<OperationId, String> {
        let (executable, args) = self.startup_parameters()?;
        let data_directory = self.data_directory.display().to_string();
        self.spawn_job(
            move || {
                Ok(Overview {
                    version: env!("CARGO_PKG_VERSION"),
                    platform: std::env::consts::OS,
                    architecture: std::env::consts::ARCH,
                    data_directory,
                    resident_bytes: rotor_platform::sys_util::get_memory_usage()
                        .map(|memory| memory.resident_bytes)
                        .map_err(|error| error.to_string()),
                    permissions: rotor_platform::sys_util::get_permission_statuses(),
                    autostart: rotor_platform::startup::enabled(&executable, &args),
                    ocr_loaded: rotor_screenshot::img_util::ocr_cache_loaded(),
                })
            },
            None,
            |id, result| RuntimeEvent::Overview { id, result },
        )
    }
    pub fn set_autostart(&self, enabled: bool) -> Result<OperationId, String> {
        let (executable, args) = self.startup_parameters()?;
        self.spawn_job(
            move || {
                rotor_platform::startup::set_enabled(enabled, &executable, &args)?;
                rotor_platform::startup::enabled(&executable, &args)
            },
            None,
            |id, result| RuntimeEvent::StartupChanged { id, result },
        )
    }
    pub fn open_url(&self, url: String) -> Result<OperationId, String> {
        self.spawn_job(
            move || rotor_platform::desktop::open_url(&url),
            None,
            |id, result| RuntimeEvent::FileOpened { id, result },
        )
    }
    pub fn coordinate_shortcuts(&self, development: bool) {
        self.development_shortcuts
            .store(development, Ordering::Release);
        self.coordinate_shortcuts.store(true, Ordering::Release);
    }
    pub fn shortcut_recording_flag(&self) -> Arc<crate::shortcuts::ShortcutRecording> {
        self.shortcut_recording.clone()
    }
    pub fn set_shortcut_recording(&self, recording: bool) {
        self.shortcut_recording.set(recording);
    }
    pub fn is_shortcut_recording(&self) -> bool {
        self.shortcut_recording.active()
    }
    pub fn quick_actions(&self) -> Result<Vec<crate::QuickAction>, String> {
        crate::quick::actions_from_config(&self.settings())
    }
    pub fn save_quick_actions(
        &self,
        actions: Vec<crate::QuickAction>,
    ) -> Result<OperationId, String> {
        let actions =
            crate::quick::normalize_actions(actions).map_err(|error| error.to_string())?;
        self.save_settings(vec![(
            "quick_actions".into(),
            serde_json::to_string(&actions).map_err(|error| error.to_string())?,
        )])
    }
    pub fn run_quick_action(&self, action_id: String) -> Result<OperationId, String> {
        let action = self
            .quick_actions()?
            .into_iter()
            .find(|action| action.id == action_id && action.enabled)
            .ok_or("Quick action is missing or disabled")?;
        self.spawn_job(
            move || crate::quick::run_command(&action.command).map_err(|error| error.to_string()),
            None,
            move |id, result| RuntimeEvent::QuickFinished {
                id,
                action_id,
                result,
            },
        )
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
        let fonts = self.canvas_fonts.clone();
        self.runtime()
            .spawn_blocking(move || {
                let _permit = permit;
                if scene.has_text() {
                    let renderer = {
                        let mut loaded = lock(&fonts);
                        if loaded.is_none() {
                            *loaded = Some(Arc::new(rotor_canvas::Renderer::with_system_fonts()?));
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
        self.restore_pins_matching(false, Vec::new())
    }

    pub fn restore_hidden_pins(&self, excluded_ids: Vec<u32>) -> Result<OperationId, String> {
        self.restore_pins_matching(true, excluded_ids)
    }

    fn restore_pins_matching(
        &self,
        include_hidden: bool,
        excluded_ids: Vec<u32>,
    ) -> Result<OperationId, String> {
        self.ensure_running()?;
        let id = next_operation();
        self.pins.submit(PinCommand::Restore {
            id,
            include_hidden,
            excluded_ids,
        })?;
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
        let mut tail = lock(&self.settings_tail);
        self.ensure_running()?;
        let id = next_operation();
        self.settings
            .try_send(SettingsCommand::Save { id, changes })
            .map_err(|_| "configuration queue is busy or closed".to_string())?;
        // Later automatic edits must queue after this explicit transaction.
        *tail = None;
        Ok(id)
    }

    /// Merge only into the last unconsumed automatic batch. A returned ID may
    /// be shared by multiple fields; its SettingsSaved snapshot covers them all.
    pub fn save_settings_coalesced(
        &self,
        changes: Vec<(String, String)>,
    ) -> Result<OperationId, String> {
        let mut tail = lock(&self.settings_tail);
        self.ensure_running()?;
        if let Some(batch) = tail.as_ref().and_then(Weak::upgrade) {
            if let Some(patch) = lock(&batch).as_mut() {
                merge_settings_changes(&mut patch.changes, changes)?;
                return Ok(patch.id);
            }
        }
        let mut merged = Vec::new();
        merge_settings_changes(&mut merged, changes)?;
        let id = next_operation();
        let batch = Arc::new(Mutex::new(Some(SettingsPatch {
            id,
            changes: merged,
        })));
        self.settings
            .try_send(SettingsCommand::Coalesced(batch.clone()))
            .map_err(|_| "configuration queue is busy or closed".to_string())?;
        *tail = Some(Arc::downgrade(&batch));
        Ok(id)
    }

    pub async fn flush_settings(&self) -> Result<(), String> {
        *lock(&self.settings_tail) = None;
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
            .map_err(|error| config.redact_error(error.to_string()));
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
        self.capture_after_overlay_change(true, std::time::Instant::now())
    }

    pub fn capture_after_overlay_change(
        &self,
        settle: bool,
        started: std::time::Instant,
    ) -> Result<OperationId, String> {
        self.ensure_running()?;
        let id = next_operation();
        self.capture_worker.submit(
            CaptureRequest {
                id,
                settle,
                submitted: started,
            },
            &self.capture_id,
        )?;
        Ok(id)
    }

    pub async fn detect_capture_rectangles<T: crate::CapturePixels>(
        &self,
        image: Arc<T>,
    ) -> Result<Vec<rotor_canvas::ImageRect>, String> {
        self.ensure_running()?;
        let permit = self
            .slots
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| "capture detection is stopped")?;
        self.ensure_running()?;
        self.runtime()
            .spawn_blocking(move || {
                let _permit = permit;
                Ok(img_util::detect_pixels(image.as_ref())?
                    .into_iter()
                    .map(|(x, y, width, height)| rotor_canvas::ImageRect {
                        x,
                        y,
                        width,
                        height,
                    })
                    .collect())
            })
            .await
            .map_err(|error| error.to_string())?
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
        let _tail = lock(&self.settings_tail);
        if self.stopped.swap(true, Ordering::AcqRel) {
            return;
        }
        self.updates.shutdown();
        *lock(&self.pins.final_updates) = updates;
        self.cancel_translation();
        self.slots.close();
        self.cancel_selection();
        self.cancel_capture();
        self.capture_worker.stop();
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

#[cfg(test)]
mod settings_queue_tests;
#[cfg(test)]
mod translation_tests;

#[cfg(test)]
mod tests;
