use gpui_kit::{prelude::*, *};
use rotor_common::{Config, Locale};
use rotor_runtime::{
    ExportFormat, ExportProgress, GifPreset, ScreenRecordLimit, ScreenRecordStatus,
};
use std::{
    path::PathBuf,
    rc::Rc,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

pub const SCREEN_RECORD_PANEL_WIDTH: f32 = 304.;
pub const SCREEN_RECORD_PANEL_HEIGHT: f32 = 416.;

const BACKGROUND: u32 = 0x141c25;
const WELL: u32 = 0x0b1118;
const LINE: u32 = 0x2a3644;
const TEXT: u32 = 0xe4edf5;
const MUTED: u32 = 0x93a4b5;
const ACCENT: u32 = 0x4ba3e3;
const RECORD: u32 = 0xe85d5d;
const SUCCESS: u32 = 0x6fd6a3;
const WARNING: u32 = 0xffc36b;
const LOCATOR: (f32, f32) = (44., 26.);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScreenRecordAction {
    Pause,
    Resume,
    Stop,
    Export {
        format: ExportFormat,
        path: PathBuf,
        gif_preset: GifPreset,
        remember_directory: bool,
    },
    CancelExport,
    OpenFile(PathBuf),
    RevealFile(PathBuf),
    Close,
}

pub type ScreenRecordCallback = Rc<dyn Fn(ScreenRecordAction, &mut Window, &mut App)>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Recording,
    Paused,
    Stopping,
    Ready,
    Exporting {
        format: ExportFormat,
        progress: ExportProgress,
    },
}

pub struct ScreenRecordView {
    locale: Locale,
    settings: Config,
    callback: ScreenRecordCallback,
    focus: FocusHandle,
    phase: Phase,
    monitor: u32,
    scale_factor: f32,
    locator: Option<(f32, f32, f32, f32)>,
    dimensions: (u32, u32),
    fps: u32,
    elapsed_ms: u64,
    frames: usize,
    unique_frames: usize,
    spool_bytes: u64,
    limit: Option<ScreenRecordLimit>,
    preview: Option<Arc<RenderImage>>,
    gif_preset: GifPreset,
    dialog_open: bool,
    last_export: Option<(ExportFormat, PathBuf)>,
    error: Option<String>,
}

impl ScreenRecordView {
    pub fn new(
        locale: Locale,
        settings: Config,
        fps: u32,
        callback: ScreenRecordCallback,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            locale,
            settings,
            callback,
            focus: cx.focus_handle(),
            phase: Phase::Recording,
            monitor: 0,
            scale_factor: 1.,
            locator: None,
            dimensions: (0, 0),
            fps,
            elapsed_ms: 0,
            frames: 0,
            unique_frames: 0,
            spool_bytes: 0,
            limit: None,
            preview: None,
            gif_preset: GifPreset::Standard,
            dialog_open: false,
            last_export: None,
            error: None,
        }
    }

    pub fn set_source(
        &mut self,
        monitor: u32,
        screen: (u32, u32),
        scale_factor: f32,
        rect: rotor_canvas::ImageRect,
    ) {
        self.monitor = monitor;
        self.scale_factor = scale_factor;
        self.dimensions = (rect.width, rect.height);
        let (w, h) = (screen.0.max(1) as f32, screen.1.max(1) as f32);
        self.locator = Some((
            rect.x as f32 / w * LOCATOR.0,
            rect.y as f32 / h * LOCATOR.1,
            (rect.width as f32 / w * LOCATOR.0).max(2.),
            (rect.height as f32 / h * LOCATOR.1).max(2.),
        ));
    }

    pub fn set_settings(&mut self, settings: Config) {
        self.settings = settings;
    }

    pub fn preview(&mut self, image: Arc<RenderImage>, cx: &mut Context<Self>) {
        self.preview = Some(image);
        cx.notify();
    }

    pub fn progress(
        &mut self,
        elapsed_ms: u64,
        frames: usize,
        unique_frames: usize,
        spool_bytes: u64,
        status: ScreenRecordStatus,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.phase, Phase::Recording | Phase::Paused) {
            return;
        }
        self.elapsed_ms = elapsed_ms;
        self.frames = frames;
        self.unique_frames = unique_frames;
        self.spool_bytes = spool_bytes;
        self.phase = match status {
            ScreenRecordStatus::Recording => Phase::Recording,
            ScreenRecordStatus::Paused => Phase::Paused,
        };
        cx.notify();
    }

    pub fn stopped(
        &mut self,
        duration_ms: u64,
        frames: usize,
        unique_frames: usize,
        spool_bytes: u64,
        limit: Option<ScreenRecordLimit>,
        cx: &mut Context<Self>,
    ) {
        self.elapsed_ms = duration_ms;
        self.frames = frames;
        self.unique_frames = unique_frames;
        self.spool_bytes = spool_bytes;
        self.limit = limit;
        self.phase = Phase::Ready;
        self.error = None;
        cx.notify();
    }

    pub fn export_progress(
        &mut self,
        format: ExportFormat,
        progress: ExportProgress,
        cx: &mut Context<Self>,
    ) {
        if matches!(self.phase, Phase::Exporting { .. }) {
            self.phase = Phase::Exporting { format, progress };
            cx.notify();
        }
    }

    pub fn export_finished(
        &mut self,
        format: ExportFormat,
        path: PathBuf,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        self.phase = Phase::Ready;
        match result {
            Ok(()) => {
                self.last_export = Some((format, path));
                self.error = None;
            }
            Err(error) if error == "Export cancelled" => {
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error);
            }
        }
        cx.notify();
    }

    pub fn is_active_recording(&self) -> bool {
        matches!(self.phase, Phase::Recording | Phase::Paused)
    }

    /// Stop from the panel, Enter, or pressing the global capture shortcut again.
    pub fn stop(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !matches!(self.phase, Phase::Recording | Phase::Paused) {
            return false;
        }
        self.phase = Phase::Stopping;
        (self.callback)(ScreenRecordAction::Stop, window, cx);
        cx.notify();
        true
    }

    fn toggle_pause(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.phase {
            Phase::Recording => {
                self.phase = Phase::Paused;
                (self.callback)(ScreenRecordAction::Pause, window, cx);
                cx.notify();
            }
            Phase::Paused => {
                self.phase = Phase::Recording;
                (self.callback)(ScreenRecordAction::Resume, window, cx);
                cx.notify();
            }
            _ => {}
        }
    }

    fn request_export(
        &mut self,
        format: ExportFormat,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.phase != Phase::Ready || self.dialog_open {
            return;
        }
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let ext = format.extension();
        let default_name = format!("Rotor_Record_{stamp}.{ext}");
        let configured = self
            .settings
            .get("save_path")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        let remember_directory = self
            .settings
            .get("if_auto_change_save_path")
            .is_none_or(|value| value != "false");

        if self
            .settings
            .get("if_ask_save_path")
            .is_some_and(|value| value == "false")
            && let Some(directory) = configured.as_ref()
        {
            self.start_export(format, directory.join(default_name), false, window, cx);
            return;
        }

        let directory = configured.or_else(std::env::home_dir).unwrap_or_default();
        let receiver = cx.prompt_for_new_path(&directory, Some(&default_name));
        self.dialog_open = true;
        self.error = None;
        cx.notify();
        cx.spawn_in(window, async move |view, cx| {
            let result = receiver.await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.dialog_open = false;
                match result {
                    Ok(Ok(Some(path))) => {
                        let path = ensure_extension(path, ext);
                        this.start_export(format, path, remember_directory, window, cx);
                    }
                    Ok(Ok(None)) => {}
                    Ok(Err(error)) => this.error = Some(error.to_string()),
                    Err(error) => this.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn start_export(
        &mut self,
        format: ExportFormat,
        path: PathBuf,
        remember_directory: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.phase != Phase::Ready {
            return;
        }
        self.error = None;
        self.phase = Phase::Exporting {
            format,
            progress: ExportProgress {
                completed: 0,
                total: self.unique_frames.max(1),
            },
        };
        (self.callback)(
            ScreenRecordAction::Export {
                format,
                path,
                gif_preset: self.gif_preset,
                remember_directory,
            },
            window,
            cx,
        );
        cx.notify();
    }

    fn cancel_export(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.phase, Phase::Exporting { .. }) {
            return;
        }
        (self.callback)(ScreenRecordAction::CancelExport, window, cx);
        cx.notify();
    }

    fn badge(&self) -> (&'static str, u32) {
        let pick = |zh, en| self.locale.pick(zh, en);
        match self.phase {
            Phase::Recording => (pick("录制中", "Recording"), RECORD),
            Phase::Paused => (pick("已暂停", "Paused"), WARNING),
            Phase::Stopping => (pick("正在停止", "Stopping"), ACCENT),
            Phase::Ready if self.last_export.is_some() => (pick("已导出", "Exported"), SUCCESS),
            Phase::Ready => (pick("准备导出", "Ready"), SUCCESS),
            Phase::Exporting {
                format: ExportFormat::Mp4,
                ..
            } => (pick("导出 MP4", "Exporting MP4"), ACCENT),
            Phase::Exporting {
                format: ExportFormat::Gif,
                ..
            } => (pick("导出 GIF", "Exporting GIF"), ACCENT),
        }
    }

    fn hint(&self) -> String {
        let pick = |zh, en| self.locale.pick(zh, en);
        if let Some(error) = &self.error {
            return error.clone();
        }
        match self.phase {
            Phase::Recording => pick(
                "正在录制选区画面。点击停止、按 Enter 或再次按截图快捷键即可结束并选择导出格式。",
                "Recording the selected region. Click Stop, press Enter, or press the capture shortcut again to export.",
            )
            .into(),
            Phase::Paused => pick(
                "录制已暂停，暂停期间不会计入时长。按空格继续录制，或点击停止进入导出。",
                "Recording paused; paused time is excluded. Press Space to resume or Stop to export.",
            )
            .into(),
            Phase::Stopping => pick("正在整理录制帧…", "Finalizing recorded frames…").into(),
            Phase::Ready => {
                if let Some((format, path)) = &self.last_export {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("output");
                    return match self.locale {
                        Locale::Chinese => format!(
                            "已导出 {}：{name}。可直接打开，或继续导出另一种格式。",
                            format.extension().to_uppercase()
                        ),
                        Locale::English => format!(
                            "Saved {}: {name}. Open it, or export another format from this clip.",
                            format.extension().to_uppercase()
                        ),
                    };
                }
                if let Some(limit) = self.limit {
                    return match limit {
                        ScreenRecordLimit::StorageBudget => pick(
                            "已达到 512 MiB 临时缓存上限并自动停止。现在可将已录制内容导出为 MP4 或 GIF。",
                            "Reached the 512 MiB temporary spool limit and stopped automatically. You can export the recorded clip as MP4 or GIF.",
                        )
                        .into(),
                        ScreenRecordLimit::DurationLimit => pick(
                            "已达到 5 分钟录制时长上限并自动停止。现在可导出为 MP4 或 GIF。",
                            "Reached the 5-minute duration limit and stopped automatically. You can export as MP4 or GIF.",
                        )
                        .into(),
                    };
                }
                let (gw, gh) = self.gif_preset.target_dimensions(
                    self.dimensions.0,
                    self.dimensions.1,
                    self.scale_factor,
                );
                match self.locale {
                    Locale::Chinese => format!(
                        "录制完成，可分别导出 MP4（原画 H.264）或 GIF（{}×{} · {} fps）。",
                        gw,
                        gh,
                        self.gif_preset.max_fps()
                    ),
                    Locale::English => format!(
                        "Clip ready. Export as native H.264 MP4 or animated GIF ({}×{} · {} fps).",
                        gw,
                        gh,
                        self.gif_preset.max_fps()
                    ),
                }
            }
            Phase::Exporting { format, progress } => {
                let pct = (progress.fraction() * 100.).round() as u32;
                match self.locale {
                    Locale::Chinese => format!(
                        "正在编码 {}（{}/{} 帧，{pct}%）…",
                        format.extension().to_uppercase(),
                        progress.completed,
                        progress.total
                    ),
                    Locale::English => format!(
                        "Encoding {} ({}/{} frames, {pct}%)…",
                        format.extension().to_uppercase(),
                        progress.completed,
                        progress.total
                    ),
                }
            }
        }
    }
}

fn ensure_extension(path: PathBuf, ext: &str) -> PathBuf {
    if path
        .extension()
        .is_some_and(|existing| existing.eq_ignore_ascii_case(ext))
    {
        path
    } else {
        let mut os = path.into_os_string();
        os.push(".");
        os.push(ext);
        PathBuf::from(os)
    }
}

fn format_duration(ms: u64) -> String {
    let total_tenths = ms / 100;
    let tenths = total_tenths % 10;
    let total_secs = total_tenths / 10;
    let secs = total_secs % 60;
    let mins = total_secs / 60;
    format!("{mins:02}:{secs:02}.{tenths}")
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 * 1024 {
        format!("{:.0} KB", (bytes as f64 / 1024.).max(1.))
    } else {
        format!("{:.1} MB", bytes as f64 / (1024. * 1024.))
    }
}

fn key_hint(key: &'static str, color: u32) -> Div {
    div()
        .px(px(4.))
        .rounded(px(3.))
        .border_1()
        .border_color(rgba(color << 8 | 0x66))
        .text_size(px(10.))
        .text_color(rgba(color << 8 | 0xcc))
        .child(key)
}

#[derive(Clone, Copy)]
enum ButtonTone {
    Primary,
    Danger,
    Secondary,
}

fn button(id: &'static str, tone: ButtonTone) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(5.))
        .h(px(30.))
        .px(px(10.))
        .rounded(px(6.))
        .cursor_pointer()
        .text_size(px(12.))
        .when(matches!(tone, ButtonTone::Primary), |button| {
            button
                .bg(rgb(0x2f7fc1))
                .text_color(rgb(0xffffff))
                .hover(|style| style.bg(rgb(0x3a8fd4)))
        })
        .when(matches!(tone, ButtonTone::Danger), |button| {
            button
                .bg(rgb(0xc84343))
                .text_color(rgb(0xffffff))
                .hover(|style| style.bg(rgb(0xdc5252)))
        })
        .when(matches!(tone, ButtonTone::Secondary), |button| {
            button
                .border_1()
                .border_color(rgb(LINE))
                .text_color(rgb(TEXT))
                .hover(|style| style.bg(rgb(0x1e2935)))
        })
}

impl Render for ScreenRecordView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let locale = self.locale;
        let pick = move |zh, en| locale.pick(zh, en);
        let (badge, badge_color) = self.badge();
        let hint_color = if self.error.is_some() || self.limit.is_some() {
            WARNING
        } else if self.last_export.is_some() && self.phase == Phase::Ready {
            SUCCESS
        } else {
            0xc3d0dc
        };

        let preview = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(148.))
            .overflow_hidden()
            .rounded(px(6.))
            .bg(rgb(WELL))
            .border_1()
            .border_color(rgb(LINE))
            .when(self.preview.is_none(), |well| {
                well.text_size(px(12.))
                    .text_color(rgb(MUTED))
                    .child(pick("等待画面…", "Waiting for frames…"))
            })
            .children(
                self.preview
                    .clone()
                    .map(|image| img(image).w_full().h_full().object_fit(ObjectFit::Contain)),
            );

        let stats =
            div()
                .flex()
                .flex_none()
                .items_center()
                .gap(px(10.))
                .children(self.locator.map(|(x, y, width, height)| {
                    let color = if matches!(self.phase, Phase::Recording) {
                        RECORD
                    } else {
                        ACCENT
                    };
                    div()
                        .relative()
                        .flex_none()
                        .w(px(LOCATOR.0))
                        .h(px(LOCATOR.1))
                        .rounded(px(3.))
                        .bg(rgb(WELL))
                        .border_1()
                        .border_color(rgb(LINE))
                        .child(
                            div()
                                .absolute()
                                .left(px(x))
                                .top(px(y))
                                .w(px(width))
                                .h(px(height))
                                .rounded(px(1.))
                                .border_1()
                                .border_color(rgb(color))
                                .bg(rgba(color << 8 | 0x40)),
                        )
                }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .child(
                            div()
                                .flex()
                                .items_baseline()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .text_size(px(16.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(format_duration(self.elapsed_ms)),
                                )
                                .child(div().text_size(px(11.)).text_color(rgb(MUTED)).child(
                                    format!("{} × {}", self.dimensions.0, self.dimensions.1),
                                )),
                        )
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(rgb(MUTED))
                                .child(format!(
                                    "{} {} · {} fps · {}",
                                    self.frames,
                                    pick("帧", "frames"),
                                    self.fps,
                                    format_bytes(self.spool_bytes),
                                )),
                        ),
                );

        let extra_row = match self.phase {
            Phase::Ready => {
                let preset_bar = div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child(pick("GIF 规格", "GIF quality")),
                    )
                    .child(
                        div()
                            .flex()
                            .p(px(2.))
                            .gap(px(2.))
                            .rounded(px(6.))
                            .bg(rgb(WELL))
                            .children([GifPreset::Standard, GifPreset::High].map(|preset| {
                                let selected = self.gif_preset == preset;
                                let id = match preset {
                                    GifPreset::Standard => "gif-preset-standard",
                                    GifPreset::High => "gif-preset-high",
                                };
                                div()
                                    .id(id)
                                    .debug_selector(move || id.into())
                                    .px(px(8.))
                                    .py(px(2.))
                                    .rounded(px(4.))
                                    .text_size(px(11.))
                                    .cursor_pointer()
                                    .when(selected, |tab| {
                                        tab.bg(rgb(0x243242)).text_color(rgb(TEXT))
                                    })
                                    .when(!selected, |tab| {
                                        tab.text_color(rgb(MUTED))
                                            .hover(|s| s.text_color(rgb(TEXT)))
                                    })
                                    .child(match preset {
                                        GifPreset::Standard => pick("标准 12fps", "Standard 12fps"),
                                        GifPreset::High => pick("高清 15fps", "High 15fps"),
                                    })
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.gif_preset = preset;
                                        cx.notify();
                                    }))
                            })),
                    );
                Some(preset_bar.into_any_element())
            }
            Phase::Exporting { progress, .. } => {
                let pct = progress.fraction();
                let bar = div()
                    .flex_none()
                    .w_full()
                    .h(px(6.))
                    .rounded_full()
                    .overflow_hidden()
                    .bg(rgb(WELL))
                    .border_1()
                    .border_color(rgb(LINE))
                    .child(div().h_full().w(relative(pct)).bg(rgb(ACCENT)));
                Some(bar.into_any_element())
            }
            _ => None,
        };

        let actions = match self.phase {
            Phase::Recording | Phase::Paused | Phase::Stopping => {
                let stopping = self.phase == Phase::Stopping;
                let paused = self.phase == Phase::Paused;
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        button("cancel-screen-record", ButtonTone::Secondary)
                            .child(pick("取消", "Cancel"))
                            .on_click(cx.listener(|this, _, window, cx| {
                                (this.callback)(ScreenRecordAction::Close, window, cx)
                            })),
                    )
                    .child(
                        button("pause-screen-record", ButtonTone::Secondary)
                            .flex_1()
                            .when(stopping, |b| b.opacity(0.5).cursor_default())
                            .child(if paused {
                                pick("继续", "Resume")
                            } else {
                                pick("暂停", "Pause")
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_pause(window, cx);
                            })),
                    )
                    .child(
                        button("stop-screen-record", ButtonTone::Danger)
                            .flex_1()
                            .when(stopping, |b| b.opacity(0.5).cursor_default())
                            .child(pick("停止", "Stop"))
                            .child(key_hint("↵", 0xffffff))
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.stop(window, cx);
                            })),
                    )
                    .into_any_element()
            }
            Phase::Ready => {
                let disabled = self.dialog_open;
                let mut stack = div().flex().flex_col().flex_none().gap(px(6.)).child(
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(
                            button("export-mp4", ButtonTone::Primary)
                                .flex_1()
                                .when(disabled, |b| b.opacity(0.5).cursor_default())
                                .child(pick("导出 MP4", "Export MP4"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.request_export(ExportFormat::Mp4, window, cx);
                                })),
                        )
                        .child(
                            button("export-gif", ButtonTone::Primary)
                                .flex_1()
                                .when(disabled, |b| b.opacity(0.5).cursor_default())
                                .child(pick("导出 GIF", "Export GIF"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.request_export(ExportFormat::Gif, window, cx);
                                })),
                        ),
                );
                let mut footer = div().flex().gap(px(6.));
                if let Some((_, path)) = self.last_export.clone() {
                    let open_path = path.clone();
                    let reveal_path = path;
                    footer = footer
                        .child(
                            button("open-exported-record", ButtonTone::Secondary)
                                .flex_1()
                                .child(pick("打开", "Open"))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    (this.callback)(
                                        ScreenRecordAction::OpenFile(open_path.clone()),
                                        window,
                                        cx,
                                    )
                                })),
                        )
                        .child(
                            button("reveal-exported-record", ButtonTone::Secondary)
                                .flex_1()
                                .child(pick("显示文件", "Reveal"))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    (this.callback)(
                                        ScreenRecordAction::RevealFile(reveal_path.clone()),
                                        window,
                                        cx,
                                    )
                                })),
                        );
                }
                footer = footer.child(
                    button("close-screen-record", ButtonTone::Secondary)
                        .when(self.last_export.is_none(), |b| b.flex_1())
                        .child(pick("关闭", "Close"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            (this.callback)(ScreenRecordAction::Close, window, cx)
                        })),
                );
                stack = stack.child(footer);
                stack.into_any_element()
            }
            Phase::Exporting { .. } => div()
                .flex()
                .flex_none()
                .child(
                    button("cancel-export-record", ButtonTone::Secondary)
                        .flex_1()
                        .child(pick("取消导出", "Cancel export"))
                        .child(key_hint("Esc", MUTED))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.cancel_export(window, cx);
                        })),
                )
                .into_any_element(),
        };

        div()
            .id("screen-record-control")
            .track_focus(&self.focus)
            .size_full()
            .bg(rgb(BACKGROUND))
            .border_1()
            .border_color(rgb(LINE))
            .text_color(rgb(TEXT))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(10.))
            .text_size(px(13.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.focus.focus(window, cx)),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(pick("录屏", "Screen record")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .px(px(8.))
                            .py(px(2.))
                            .rounded_full()
                            .bg(rgba(badge_color << 8 | 0x22))
                            .text_size(px(11.))
                            .text_color(rgb(badge_color))
                            .child(div().size(px(6.)).rounded_full().bg(rgb(badge_color)))
                            .child(badge),
                    ),
            )
            .child(preview)
            .child(stats)
            .children(extra_row)
            .child(
                div()
                    .id("screen-record-status")
                    .debug_selector(|| "screen-record-status".into())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .text_size(px(12.))
                    .line_height(px(17.))
                    .text_color(rgb(hint_color))
                    .child(self.hint()),
            )
            .child(actions)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.is_held || this.dialog_open {
                    return;
                }
                match event.keystroke.key.as_str() {
                    "escape" => {
                        if matches!(this.phase, Phase::Exporting { .. }) {
                            this.cancel_export(window, cx);
                        } else {
                            (this.callback)(ScreenRecordAction::Close, window, cx);
                        }
                    }
                    "enter" => {
                        if !this.stop(window, cx) {
                            return;
                        }
                    }
                    "space" => {
                        if matches!(this.phase, Phase::Recording | Phase::Paused) {
                            this.toggle_pause(window, cx);
                        } else {
                            return;
                        }
                    }
                    _ => return,
                }
                cx.stop_propagation();
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Config, ExportFormat, ExportProgress, GifPreset, Locale, PathBuf, Rc,
        SCREEN_RECORD_PANEL_HEIGHT, SCREEN_RECORD_PANEL_WIDTH, ScreenRecordAction,
        ScreenRecordView, ensure_extension, format_duration,
    };
    use gpui_kit::{MouseButton, px, size};
    use std::cell::RefCell;

    #[test]
    fn duration_and_extension_helpers_format_cleanly() {
        assert_eq!(format_duration(0), "00:00.0");
        assert_eq!(format_duration(12_450), "00:12.4");
        assert_eq!(format_duration(65_900), "01:05.9");
        assert_eq!(
            ensure_extension(PathBuf::from("demo"), "mp4"),
            PathBuf::from("demo.mp4")
        );
        assert_eq!(
            ensure_extension(PathBuf::from("demo.MP4"), "mp4"),
            PathBuf::from("demo.MP4")
        );
        assert_eq!(
            ensure_extension(PathBuf::from("demo.mp4"), "gif"),
            PathBuf::from("demo.mp4.gif")
        );
    }

    #[gpui::test]
    fn recording_panel_transitions_from_recording_to_export_and_open(
        cx: &mut gpui::TestAppContext,
    ) {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let recorded = actions.clone();
        let mut settings = Config::new();
        settings.insert("save_path".into(), "/tmp".into());
        settings.insert("if_ask_save_path".into(), "false".into());

        let (view, cx) = cx.add_window_view(|window, cx| {
            let mut view = ScreenRecordView::new(
                Locale::English,
                settings,
                24,
                Rc::new(move |action, _, _| recorded.borrow_mut().push(action)),
                cx,
            );
            view.set_source(
                1,
                (1920, 1080),
                2.,
                rotor_canvas::ImageRect {
                    x: 100,
                    y: 100,
                    width: 800,
                    height: 600,
                },
            );
            view.focus.focus(window, cx);
            view
        });
        cx.simulate_resize(size(
            px(SCREEN_RECORD_PANEL_WIDTH),
            px(SCREEN_RECORD_PANEL_HEIGHT),
        ));
        cx.update(|window, cx| window.draw(cx).clear(cx));

        // Space toggles pause and resume while recording.
        cx.simulate_keystrokes("space");
        assert_eq!(actions.borrow().as_slice(), [ScreenRecordAction::Pause]);
        cx.simulate_keystrokes("space");
        assert_eq!(
            actions.borrow().as_slice(),
            [ScreenRecordAction::Pause, ScreenRecordAction::Resume]
        );

        // Enter stops recording.
        cx.simulate_keystrokes("enter");
        assert_eq!(actions.borrow().last(), Some(&ScreenRecordAction::Stop));

        // Worker delivers Stopped -> panel shows Export MP4 / GIF controls.
        view.update(cx, |view, cx| {
            view.stopped(2500, 60, 40, 2 * 1024 * 1024, None, cx);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));

        for selector in ["export-mp4", "export-gif", "close-screen-record"] {
            let bounds = cx.debug_bounds(selector).unwrap();
            assert!(bounds.origin.x >= px(0.) && bounds.origin.y >= px(0.));
            assert!(bounds.right() <= px(SCREEN_RECORD_PANEL_WIDTH));
            assert!(bounds.bottom() <= px(SCREEN_RECORD_PANEL_HEIGHT));
        }

        // Click Export GIF -> dispatches Export with Standard preset (auto-save path).
        let gif_btn = cx.debug_bounds("export-gif").unwrap().center();
        cx.simulate_mouse_down(gif_btn, MouseButton::Left, Default::default());
        cx.simulate_mouse_up(gif_btn, MouseButton::Left, Default::default());
        assert!(matches!(
            actions.borrow().last(),
            Some(ScreenRecordAction::Export {
                format: ExportFormat::Gif,
                gif_preset: GifPreset::Standard,
                ..
            })
        ));

        // Progress and completion enable Open and Reveal buttons while keeping export available.
        let out_path = PathBuf::from("/tmp/Rotor_Record_1.gif");
        view.update(cx, |view, cx| {
            view.export_progress(
                ExportFormat::Gif,
                ExportProgress {
                    completed: 20,
                    total: 40,
                },
                cx,
            );
            view.export_finished(ExportFormat::Gif, out_path.clone(), Ok(()), cx);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let open_btn = cx.debug_bounds("open-exported-record").unwrap().center();
        cx.simulate_mouse_down(open_btn, MouseButton::Left, Default::default());
        cx.simulate_mouse_up(open_btn, MouseButton::Left, Default::default());
        assert_eq!(
            actions.borrow().last(),
            Some(&ScreenRecordAction::OpenFile(out_path))
        );
    }
}
