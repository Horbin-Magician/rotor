use gpui_kit::{prelude::*, *};
use rotor_common::{Config, Locale};
use rotor_runtime::{
    ExportFormat, ExportProgress, GifPreset, ScreenRecordLimit, ScreenRecordStatus,
};
use std::{path::PathBuf, rc::Rc, sync::Arc};

pub const SCREEN_RECORD_PANEL_WIDTH: f32 = 280.;
pub const SCREEN_RECORD_PANEL_HEIGHT: f32 = 48.;
pub const SCREEN_RECORD_REVIEW_WIDTH: f32 = 320.;
pub const SCREEN_RECORD_REVIEW_HEIGHT: f32 = 184.;

const BACKGROUND: u32 = 0x141c25;
const WELL: u32 = 0x0b1118;
const LINE: u32 = 0x2a3644;
const TEXT: u32 = 0xe4edf5;
const MUTED: u32 = 0x93a4b5;
const ACCENT: u32 = 0x4ba3e3;
const RECORD: u32 = 0xe85d5d;
const SUCCESS: u32 = 0x6fd6a3;
const WARNING: u32 = 0xffc36b;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScreenRecordAction {
    Move,
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
    scale_factor: f32,
    dimensions: (u32, u32),
    elapsed_ms: u64,
    unique_frames: usize,
    limit: Option<ScreenRecordLimit>,
    preview: Option<Arc<RenderImage>>,
    gif_preset: GifPreset,
    format: ExportFormat,
    confirm_discard: bool,
    cancelling_export: bool,
    pending_status: Option<ScreenRecordStatus>,
    dialog_open: bool,
    last_export: Option<(ExportFormat, PathBuf)>,
    error: Option<String>,
}

impl ScreenRecordView {
    pub fn new(
        locale: Locale,
        settings: Config,
        callback: ScreenRecordCallback,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            locale,
            settings,
            callback,
            focus: cx.focus_handle(),
            phase: Phase::Recording,
            scale_factor: 1.,
            dimensions: (0, 0),
            elapsed_ms: 0,
            unique_frames: 0,
            limit: None,
            preview: None,
            gif_preset: GifPreset::Standard,
            format: ExportFormat::Mp4,
            confirm_discard: false,
            cancelling_export: false,
            pending_status: None,
            dialog_open: false,
            last_export: None,
            error: None,
        }
    }

    pub fn set_source(&mut self, scale_factor: f32, rect: rotor_canvas::ImageRect) {
        self.scale_factor = scale_factor;
        self.dimensions = (rect.width, rect.height);
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
    }

    /// Closing an active recording stops it first; unsaved clips require an
    /// explicit discard. Native close and keyboard close use the same path.
    pub fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog_open {
            return;
        }
        match self.phase {
            Phase::Recording | Phase::Paused => {
                self.stop(window, cx);
            }
            Phase::Stopping => {}
            Phase::Exporting { .. } => self.cancel_export(window, cx),
            Phase::Ready if self.last_export.is_some() => {
                (self.callback)(ScreenRecordAction::Close, window, cx);
            }
            Phase::Ready => {
                self.confirm_discard = true;
                cx.notify();
            }
        }
    }

    /// Content height requested by the review; native resizing belongs to the shell.
    pub fn review_height(&self) -> Option<f32> {
        match self.phase {
            Phase::Recording | Phase::Paused | Phase::Stopping => None,
            Phase::Exporting { .. } => Some(150.),
            Phase::Ready if self.confirm_discard => Some(144.),
            Phase::Ready => Some(
                SCREEN_RECORD_REVIEW_HEIGHT
                    + if self.format == ExportFormat::Gif {
                        36.
                    } else {
                        0.
                    }
                    + if self.last_export.is_some() { 38. } else { 0. },
            ),
        }
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
        unique_frames: usize,
        status: ScreenRecordStatus,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.phase, Phase::Recording | Phase::Paused) {
            return;
        }
        self.elapsed_ms = elapsed_ms;
        self.unique_frames = unique_frames;
        if self.pending_status.is_some_and(|pending| pending != status) {
            cx.notify();
            return;
        }
        self.pending_status = None;
        self.phase = match status {
            ScreenRecordStatus::Recording => Phase::Recording,
            ScreenRecordStatus::Paused => Phase::Paused,
        };
        cx.notify();
    }

    pub fn stopped(
        &mut self,
        duration_ms: u64,
        unique_frames: usize,
        limit: Option<ScreenRecordLimit>,
        cx: &mut Context<Self>,
    ) {
        self.elapsed_ms = duration_ms;
        self.unique_frames = unique_frames;
        self.limit = limit;
        self.pending_status = None;
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
        self.cancelling_export = false;
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
        if self.pending_status.is_some() {
            return;
        }
        match self.phase {
            Phase::Recording => {
                self.phase = Phase::Paused;
                self.pending_status = Some(ScreenRecordStatus::Paused);
                (self.callback)(ScreenRecordAction::Pause, window, cx);
                cx.notify();
            }
            Phase::Paused => {
                self.phase = Phase::Recording;
                self.pending_status = Some(ScreenRecordStatus::Recording);
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
        if self.phase != Phase::Ready || self.dialog_open || self.confirm_discard {
            return;
        }
        let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S_%3f");
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
        if !matches!(self.phase, Phase::Exporting { .. }) || self.cancelling_export {
            return;
        }
        self.cancelling_export = true;
        (self.callback)(ScreenRecordAction::CancelExport, window, cx);
        cx.notify();
    }

    fn badge(&self) -> (&'static str, u32) {
        let pick = |zh, en| self.locale.pick(zh, en);
        match self.phase {
            Phase::Recording => (pick("录制中", "Recording"), RECORD),
            Phase::Paused => (pick("已暂停", "Paused"), WARNING),
            Phase::Stopping => (pick("正在停止", "Stopping"), ACCENT),
            Phase::Ready if self.last_export.is_some() => (pick("已保存", "Saved"), SUCCESS),
            Phase::Ready => (pick("待保存", "Ready to save"), SUCCESS),
            Phase::Exporting {
                format: ExportFormat::Mp4,
                ..
            } => (pick("保存中", "Saving"), ACCENT),
            Phase::Exporting {
                format: ExportFormat::Gif,
                ..
            } => (pick("保存中", "Saving"), ACCENT),
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if event.is_held || self.dialog_open || event.keystroke.modifiers != Default::default() {
            return;
        }
        match event.keystroke.key.as_str() {
            "escape" => {
                if self.confirm_discard {
                    self.confirm_discard = false;
                    cx.notify();
                } else {
                    self.request_close(window, cx);
                }
            }
            "enter" => {
                if self.confirm_discard {
                    self.confirm_discard = false;
                    cx.notify();
                } else if self.phase == Phase::Ready {
                    self.request_export(self.format, window, cx);
                } else if !self.stop(window, cx) {
                    return;
                }
            }
            "space" => {
                if matches!(self.phase, Phase::Recording | Phase::Paused) {
                    self.toggle_pause(window, cx);
                } else {
                    return;
                }
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    fn recording_bar(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let locale = self.locale;
        let pick = move |zh, en| locale.pick(zh, en);
        let paused = self.phase == Phase::Paused;
        let stopping = self.phase == Phase::Stopping;
        let (status, color) = self.badge();
        let pause_hint = if paused {
            pick("继续录制 · 空格", "Resume · Space")
        } else {
            pick("暂停录制 · 空格", "Pause · Space")
        };
        let stop_hint = pick("停止录制 · Enter / Esc", "Stop recording · Enter / Esc");
        let drag_hint = pick(
            "拖动控制条 · 仅录画面，无声音",
            "Drag to move · Recording without audio",
        );
        // The native window owns the sole outer outline and corner mask.
        div()
            .id("screen-record-control")
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .items_center()
            .gap(px(8.))
            .pl(px(12.))
            .pr(px(7.))
            // Native window blur supplies the frost; the flat translucent tint
            // keeps controls legible without highlights or simulated refraction.
            .bg(rgba(0x18222e99))
            .text_color(rgb(0xf4f7fb))
            .text_size(px(12.))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.focus.focus(window, cx)),
            )
            .on_key_down(cx.listener(Self::on_key_down))
            .child(
                div()
                    .id("screen-record-drag")
                    .debug_selector(|| "screen-record-drag".into())
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .items_center()
                    .gap(px(8.))
                    .cursor(CursorStyle::OpenHand)
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(drag_hint).build(window, cx)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            (this.callback)(ScreenRecordAction::Move, window, cx);
                        }),
                    )
                    .child(div().size(px(7.)).flex_none().rounded_full().bg(rgb(color)))
                    .child(
                        div()
                            .id("screen-record-status")
                            .debug_selector(|| "screen-record-status".into())
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if stopping {
                                pick("整理中", "Finishing").to_owned()
                            } else {
                                format!(
                                    "{:02}:{:02}",
                                    self.elapsed_ms / 60_000,
                                    self.elapsed_ms / 1000 % 60
                                )
                            })
                            .tooltip(move |window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(status).build(window, cx)
                            }),
                    ),
            )
            .child(div().w(px(1.)).h(px(18.)).flex_none().bg(rgba(0xffffff26)))
            .child(
                recording_button("pause-screen-record")
                    .when(stopping || self.pending_status.is_some(), |b| {
                        b.opacity(0.5).cursor_default()
                    })
                    .child(if paused {
                        pick("继续", "Resume")
                    } else {
                        pick("暂停", "Pause")
                    })
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(pause_hint).build(window, cx)
                    })
                    .on_click(cx.listener(|this, _, window, cx| this.toggle_pause(window, cx))),
            )
            .child(
                recording_button("stop-screen-record")
                    .bg(rgba(0xffffff14))
                    .when(stopping, |b| b.opacity(0.5).cursor_default())
                    .child(div().size(px(8.)).rounded(px(2.)).bg(rgb(0xff737b)))
                    .child(pick("停止", "Stop"))
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(stop_hint).build(window, cx)
                    })
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.stop(window, cx);
                    })),
            )
    }

    fn hint(&self) -> String {
        let pick = |zh, en| self.locale.pick(zh, en);
        if self.confirm_discard {
            return pick(
                "录屏尚未保存，确定丢弃？",
                "Discard this unsaved recording?",
            )
            .into();
        }
        if let Some(error) = &self.error {
            return error.clone();
        }
        match self.phase {
            Phase::Recording => pick(
                "无声音 · 再按截图快捷键停止",
                "No audio. Capture shortcut stops.",
            )
            .into(),
            Phase::Paused => pick("暂停期间不计入录制时长", "Paused time is excluded").into(),
            Phase::Stopping => pick("正在整理录屏…", "Finishing recording…").into(),
            Phase::Ready => {
                if let Some((_, path)) = &self.last_export {
                    return path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                }
                if let Some(limit) = self.limit {
                    return match limit {
                        ScreenRecordLimit::StorageBudget => pick(
                            "缓存已满，录屏已保留，请保存。",
                            "Storage limit reached. Your clip is ready to save.",
                        ),
                        ScreenRecordLimit::DurationLimit => pick(
                            "已录满 5 分钟，请保存。",
                            "5-minute limit reached. Save your recording.",
                        ),
                    }
                    .into();
                }
                match self.format {
                    ExportFormat::Mp4 => {
                        pick("原画视频，无声音", "Full-resolution video, without audio").into()
                    }
                    ExportFormat::Gif => {
                        let (w, h) = self.gif_preset.target_dimensions(
                            self.dimensions.0,
                            self.dimensions.1,
                            self.scale_factor,
                        );
                        format!("{w} × {h} · {} fps", self.gif_preset.max_fps())
                    }
                }
            }
            Phase::Exporting { progress, .. } => {
                if self.cancelling_export {
                    return pick(
                        "正在取消，录屏将保留…",
                        "Cancelling; your recording will be kept…",
                    )
                    .into();
                }
                let pct = (progress.fraction() * 100.).round() as u32;
                format!("{} {pct}%", pick("正在保存", "Saving"))
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
        .h(px(32.))
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

fn recording_button(id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .debug_selector(move || id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .w(px(66.))
        .h(px(32.))
        .rounded(px(6.))
        .cursor_pointer()
        .hover(|style| style.bg(rgba(0xffffff1f)))
}

impl Render for ScreenRecordView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if matches!(
            self.phase,
            Phase::Recording | Phase::Paused | Phase::Stopping
        ) {
            return self.recording_bar(cx).into_any_element();
        }
        let locale = self.locale;
        let pick = move |zh, en| locale.pick(zh, en);
        let (badge, badge_color) = self.badge();
        let hint_color = if self.error.is_some()
            || self.confirm_discard
            || (self.limit.is_some() && self.last_export.is_none())
        {
            WARNING
        } else if self.last_export.is_some() && self.phase == Phase::Ready {
            SUCCESS
        } else {
            0xc3d0dc
        };

        // A small still frame is useful context, but an absent preview must
        // never reserve a large empty well after recording has already stopped.
        let summary = div()
            .flex()
            .items_center()
            .flex_1()
            .min_w_0()
            .gap(px(8.))
            .children(self.preview.clone().map(|image| {
                div()
                    .w(px(48.))
                    .h(px(34.))
                    .flex_none()
                    .rounded(px(4.))
                    .overflow_hidden()
                    .bg(rgb(WELL))
                    .child(img(image).size_full().object_fit(ObjectFit::Contain))
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(pick("录屏", "Recording")),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child(format!(
                                "{} · {} × {}",
                                format_duration(self.elapsed_ms),
                                self.dimensions.0,
                                self.dimensions.1
                            )),
                    ),
            );

        let extra_row =
            match self.phase {
                Phase::Ready if !self.confirm_discard => {
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
                                .child(pick("GIF 画质", "GIF quality")),
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
                                            GifPreset::Standard => pick("标准", "Standard"),
                                            GifPreset::High => pick("高清", "High"),
                                        })
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            if this.dialog_open || this.confirm_discard {
                                                return;
                                            }
                                            this.gif_preset = preset;
                                            cx.notify();
                                        }))
                                })),
                        );
                    let formats = div().flex().gap(px(6.)).children(
                        [ExportFormat::Mp4, ExportFormat::Gif].map(|format| {
                            let selected = self.format == format;
                            let id = match format {
                                ExportFormat::Mp4 => "format-mp4",
                                ExportFormat::Gif => "format-gif",
                            };
                            button(id, ButtonTone::Secondary)
                                .when(selected, |tab| {
                                    tab.border_color(rgb(ACCENT)).bg(rgba(ACCENT << 8 | 0x22))
                                })
                                .flex_1()
                                .child(match format {
                                    ExportFormat::Mp4 => pick("MP4 视频", "MP4 video"),
                                    ExportFormat::Gif => pick("GIF 动图", "GIF animation"),
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if !this.dialog_open && !this.confirm_discard {
                                        this.format = format;
                                        cx.notify();
                                    }
                                }))
                        }),
                    );
                    Some(
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .gap(px(8.))
                            .child(formats)
                            .when(self.format == ExportFormat::Gif, |stack| {
                                stack.child(preset_bar)
                            })
                            .into_any_element(),
                    )
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
                unreachable!("recording uses the compact bar")
            }
            Phase::Ready if self.confirm_discard => div()
                .flex()
                .flex_none()
                .gap(px(6.))
                .child(
                    button("keep-screen-record", ButtonTone::Primary)
                        .flex_1()
                        .child(pick("返回保存", "Keep recording"))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.confirm_discard = false;
                            cx.notify();
                        })),
                )
                .child(
                    button("discard-screen-record", ButtonTone::Danger)
                        .child(pick("丢弃", "Discard"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            (this.callback)(ScreenRecordAction::Close, window, cx);
                        })),
                )
                .into_any_element(),
            Phase::Ready => {
                let disabled = self.dialog_open;
                let save = button(
                    "save-screen-record",
                    if self.last_export.is_some() {
                        ButtonTone::Secondary
                    } else {
                        ButtonTone::Primary
                    },
                )
                .when(disabled, |b| b.opacity(0.5).cursor_default())
                .child(if disabled {
                    pick("选择保存位置…", "Choose a location…")
                } else {
                    match self.format {
                        ExportFormat::Mp4 => pick("保存 MP4", "Save MP4"),
                        ExportFormat::Gif => pick("保存 GIF", "Save GIF"),
                    }
                })
                .child(key_hint("↵", 0xffffff))
                .flex_1()
                .on_click(cx.listener(|this, _, window, cx| {
                    this.request_export(this.format, window, cx);
                }));
                let close = button("close-screen-record", ButtonTone::Secondary)
                    .child(pick("关闭", "Close"))
                    .when(disabled, |b| b.opacity(0.5).cursor_default())
                    .on_click(cx.listener(|this, _, window, cx| this.request_close(window, cx)));
                let mut stack = div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .gap(px(6.))
                    .child(div().flex().gap(px(6.)).child(save).child(close));
                let mut footer = div().flex().gap(px(6.));
                if let Some((_, path)) = self.last_export.clone() {
                    let open_path = path.clone();
                    let reveal_path = path;
                    footer = footer
                        .child(
                            button("open-exported-record", ButtonTone::Primary)
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
                if self.last_export.is_some() {
                    stack = stack.child(footer);
                }
                stack.into_any_element()
            }
            Phase::Exporting { .. } => div()
                .flex()
                .flex_none()
                .child(
                    button("cancel-export-record", ButtonTone::Secondary)
                        .flex_1()
                        .when(self.cancelling_export, |b| b.opacity(0.5).cursor_default())
                        .child(pick("取消保存", "Cancel save"))
                        .child(key_hint("Esc", MUTED))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.cancel_export(window, cx);
                        })),
                )
                .into_any_element(),
        };

        // The native window owns the sole outer outline and corner mask.
        div()
            .id("screen-record-control")
            .track_focus(&self.focus)
            .size_full()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
            .p(px(12.))
            .flex()
            .flex_col()
            .gap(px(8.))
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
                    .h(px(36.))
                    .gap(px(8.))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            (this.callback)(ScreenRecordAction::Move, window, cx);
                        }),
                    )
                    .child(summary)
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
            .on_key_down(cx.listener(Self::on_key_down))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Config, ExportFormat, ExportProgress, GifPreset, Locale, PathBuf, Rc,
        SCREEN_RECORD_PANEL_HEIGHT, SCREEN_RECORD_PANEL_WIDTH, SCREEN_RECORD_REVIEW_HEIGHT,
        SCREEN_RECORD_REVIEW_WIDTH, ScreenRecordAction, ScreenRecordView, ensure_extension,
        format_duration,
    };
    use super::{Phase, ScreenRecordStatus};
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
                Rc::new(move |action, _, _| recorded.borrow_mut().push(action)),
                cx,
            );
            view.set_source(
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

        for selector in [
            "pause-screen-record",
            "stop-screen-record",
            "screen-record-status",
        ] {
            let bounds = cx.debug_bounds(selector).unwrap();
            assert!(bounds.origin.y >= px(0.));
            assert!(bounds.bottom() <= px(SCREEN_RECORD_PANEL_HEIGHT));
            assert!(bounds.right() <= px(SCREEN_RECORD_PANEL_WIDTH));
            assert!(bounds.size.height >= px(17.));
        }

        let timer = cx.debug_bounds("screen-record-status").unwrap();
        let pause = cx.debug_bounds("pause-screen-record").unwrap();
        let stop = cx.debug_bounds("stop-screen-record").unwrap();
        assert!(timer.right() < pause.left() && pause.right() < stop.left());
        assert_eq!(pause.center().y, stop.center().y);
        assert_eq!(pause.size.height, px(32.));
        // Dragging the timer moves the window; the separate control hit areas
        // must never initiate a native drag.
        let drag = cx.debug_bounds("screen-record-drag").unwrap().center();
        cx.simulate_mouse_down(drag, MouseButton::Left, Default::default());
        cx.simulate_mouse_up(drag, MouseButton::Left, Default::default());
        assert_eq!(actions.borrow().as_slice(), [ScreenRecordAction::Move]);
        actions.borrow_mut().clear();

        // Queued recording progress cannot undo a requested pause. Repeated
        // keys wait for acknowledgement, then Space can resume normally.
        cx.simulate_keystrokes("space");
        view.update(cx, |view, cx| {
            view.progress(50, 1, ScreenRecordStatus::Recording, cx);
            assert_eq!(view.phase, Phase::Paused);
        });
        cx.simulate_keystrokes("space");
        assert_eq!(actions.borrow().as_slice(), [ScreenRecordAction::Pause]);
        view.update(cx, |view, cx| {
            view.progress(50, 1, ScreenRecordStatus::Paused, cx);
        });
        cx.update(|window, cx| window.draw(cx).clear(cx));
        let resume = cx.debug_bounds("pause-screen-record").unwrap();
        assert_eq!(resume, pause);
        cx.simulate_mouse_down(resume.center(), MouseButton::Left, Default::default());
        cx.simulate_mouse_up(resume.center(), MouseButton::Left, Default::default());
        assert_eq!(
            actions.borrow().as_slice(),
            [ScreenRecordAction::Pause, ScreenRecordAction::Resume]
        );

        // Enter stops recording.
        cx.simulate_keystrokes("enter");
        assert_eq!(actions.borrow().last(), Some(&ScreenRecordAction::Stop));
        view.update(cx, |view, cx| {
            view.progress(50, 1, ScreenRecordStatus::Paused, cx);
            assert_eq!(view.phase, Phase::Stopping);
        });

        // Worker delivers Stopped -> panel shows Export MP4 / GIF controls.
        view.update(cx, |view, cx| {
            view.stopped(2500, 40, None, cx);
        });
        cx.simulate_resize(size(
            px(SCREEN_RECORD_REVIEW_WIDTH),
            px(SCREEN_RECORD_REVIEW_HEIGHT),
        ));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("gif-preset-standard").is_none());
        assert_eq!(
            cx.debug_bounds("save-screen-record").unwrap().center().y,
            cx.debug_bounds("close-screen-record").unwrap().center().y
        );
        assert!(cx.debug_bounds("screen-record-status").unwrap().size.height >= px(34.));

        for selector in [
            "format-mp4",
            "format-gif",
            "save-screen-record",
            "close-screen-record",
        ] {
            let bounds = cx.debug_bounds(selector).unwrap();
            assert!(bounds.origin.x >= px(0.) && bounds.origin.y >= px(0.));
            assert!(bounds.right() <= px(SCREEN_RECORD_REVIEW_WIDTH));
            assert!(bounds.bottom() <= px(SCREEN_RECORD_REVIEW_HEIGHT));
        }

        // Click Export GIF -> dispatches Export with Standard preset (auto-save path).
        let gif_btn = cx.debug_bounds("format-gif").unwrap().center();
        cx.simulate_mouse_down(gif_btn, MouseButton::Left, Default::default());
        cx.simulate_mouse_up(gif_btn, MouseButton::Left, Default::default());
        let gif_height = view.read_with(cx, |view, _| view.review_height().unwrap());
        cx.simulate_resize(size(px(SCREEN_RECORD_REVIEW_WIDTH), px(gif_height)));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        for selector in [
            "gif-preset-standard",
            "gif-preset-high",
            "save-screen-record",
            "screen-record-status",
        ] {
            let bounds = cx.debug_bounds(selector).unwrap();
            assert!(bounds.bottom() <= px(gif_height));
            assert!(bounds.size.height >= px(17.));
        }
        cx.simulate_keystrokes("enter");
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
        let saved_height = view.read_with(cx, |view, _| view.review_height().unwrap());
        cx.simulate_resize(size(px(SCREEN_RECORD_REVIEW_WIDTH), px(saved_height)));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        for selector in [
            "save-screen-record",
            "close-screen-record",
            "open-exported-record",
            "reveal-exported-record",
            "screen-record-status",
        ] {
            let bounds = cx.debug_bounds(selector).unwrap();
            assert!(bounds.bottom() <= px(saved_height));
            assert!(bounds.right() <= px(SCREEN_RECORD_REVIEW_WIDTH));
            assert!(bounds.size.height >= px(17.));
        }
        let open_btn = cx.debug_bounds("open-exported-record").unwrap().center();
        cx.simulate_mouse_down(open_btn, MouseButton::Left, Default::default());
        cx.simulate_mouse_up(open_btn, MouseButton::Left, Default::default());
        assert_eq!(
            actions.borrow().last(),
            Some(&ScreenRecordAction::OpenFile(out_path))
        );
    }
    #[gpui::test]
    fn closing_and_cancelling_keep_unsaved_recordings(cx: &mut gpui::TestAppContext) {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let recorded = actions.clone();
        let mut settings = Config::new();
        settings.insert("save_path".into(), "/tmp".into());
        settings.insert("if_ask_save_path".into(), "false".into());
        let (view, cx) = cx.add_window_view(|window, cx| {
            let view = ScreenRecordView::new(
                Locale::Chinese,
                settings,
                Rc::new(move |action, _, _| recorded.borrow_mut().push(action)),
                cx,
            );
            view.focus(window, cx);
            view
        });
        cx.simulate_resize(size(
            px(SCREEN_RECORD_REVIEW_WIDTH),
            px(SCREEN_RECORD_REVIEW_HEIGHT),
        ));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_keystrokes("escape");
        cx.simulate_keystrokes("escape");
        assert_eq!(actions.borrow().as_slice(), [ScreenRecordAction::Stop]);
        view.update(cx, |view, cx| view.stopped(500, 8, None, cx));
        cx.simulate_keystrokes("escape");
        view.update(cx, |view, _| assert!(view.confirm_discard));
        // Enter chooses the safe action instead of discarding or exporting.
        cx.simulate_keystrokes("enter");
        assert_eq!(actions.borrow().as_slice(), [ScreenRecordAction::Stop]);
        view.update(cx, |view, _| assert!(!view.confirm_discard));
        cx.simulate_keystrokes("enter");
        assert!(matches!(
            actions.borrow().last(),
            Some(ScreenRecordAction::Export {
                format: ExportFormat::Mp4,
                ..
            })
        ));
        cx.simulate_keystrokes("escape");
        cx.simulate_keystrokes("escape");
        assert_eq!(
            actions
                .borrow()
                .iter()
                .filter(|a| **a == ScreenRecordAction::CancelExport)
                .count(),
            1
        );
        view.update(cx, |view, cx| {
            view.export_finished(
                ExportFormat::Mp4,
                PathBuf::from("/tmp/cancelled.mp4"),
                Err("Export cancelled".into()),
                cx,
            );
            assert_eq!(view.phase, Phase::Ready);
            assert!(view.last_export.is_none());
            assert!(view.error.is_none());
        });
        cx.simulate_keystrokes("escape");
        let confirm_height = view.read_with(cx, |view, _| view.review_height().unwrap());
        cx.simulate_resize(size(px(SCREEN_RECORD_REVIEW_WIDTH), px(confirm_height)));
        cx.update(|window, cx| window.draw(cx).clear(cx));
        for selector in [
            "keep-screen-record",
            "discard-screen-record",
            "screen-record-status",
        ] {
            assert!(cx.debug_bounds(selector).unwrap().bottom() <= px(confirm_height));
        }
        let discard = cx.debug_bounds("discard-screen-record").unwrap().center();
        cx.simulate_mouse_down(discard, MouseButton::Left, Default::default());
        cx.simulate_mouse_up(discard, MouseButton::Left, Default::default());
        assert_eq!(actions.borrow().last(), Some(&ScreenRecordAction::Close));
    }
}
