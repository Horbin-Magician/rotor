//! Screen recording frame spool and MP4/GIF exporters.
//!
//! Captured frames are written incrementally as dirty BGRA rectangles into an
//! append-only temporary file on disk so memory stays O(1) regardless of
//! recording length. Once stopped, the same immutable [`RecordedClip`] can be
//! exported as H.264 `.mp4`, animated `.gif`, or both.

use gif::{DisposalMethod, Encoder as GifEncoder, Frame as GifFrame, Repeat};
use image::RgbaImage;
use rotor_platform::video::{Mp4Config, Mp4Writer};
use std::{
    fs::File,
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
use tempfile::NamedTempFile;

/// Maximum disk space used by one recording's temporary frame spool (512 MiB).
pub const DEFAULT_SPOOL_BUDGET_BYTES: u64 = 512 * 1024 * 1024;
/// Maximum recording duration (5 minutes).
pub const MAX_RECORD_DURATION_MS: u64 = 5 * 60 * 1000;
/// Default recording capture cadence.
pub const DEFAULT_RECORD_FPS: u32 = 24;

static GIF_TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Mp4,
    Gif,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Mp4 => "mp4",
            Self::Gif => "gif",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GifPreset {
    /// Balanced size and clarity: targets logical resolution (capped at 1280px) at 12 fps.
    #[default]
    Standard,
    /// Higher clarity: preserves up to 1920px at 15 fps.
    High,
}

impl GifPreset {
    pub fn max_fps(self) -> u32 {
        match self {
            Self::Standard => 12,
            Self::High => 15,
        }
    }

    pub fn max_dimension(self) -> u32 {
        match self {
            Self::Standard => 1280,
            Self::High => 1920,
        }
    }

    pub fn quantize_speed(self) -> i32 {
        match self {
            Self::Standard => 12,
            Self::High => 10,
        }
    }

    pub fn target_dimensions(self, width: u32, height: u32, scale_factor: f32) -> (u32, u32) {
        let mut target_w = width.max(1);
        let mut target_h = height.max(1);
        // On HiDPI displays, Standard mode exports at logical (1x) pixels so
        // GIFs stay fast to encode and reasonable in file size.
        if self == Self::Standard && scale_factor.is_finite() && scale_factor > 1.25 {
            target_w = ((width as f32 / scale_factor).round() as u32).max(1);
            target_h = ((height as f32 / scale_factor).round() as u32).max(1);
        }
        let limit = self.max_dimension();
        let longest = target_w.max(target_h);
        if longest > limit {
            let ratio = limit as f64 / longest as f64;
            target_w = ((target_w as f64 * ratio).round() as u32).max(1);
            target_h = ((target_h as f64 * ratio).round() as u32).max(1);
        }
        (target_w.min(u16::MAX as u32), target_h.min(u16::MAX as u32))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportProgress {
    pub completed: usize,
    pub total: usize,
}

impl ExportProgress {
    pub fn fraction(self) -> f32 {
        if self.total == 0 {
            0.
        } else {
            (self.completed as f32 / self.total as f32).clamp(0., 1.)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordError {
    StorageBudget,
    DurationLimit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SpoolEntry {
    pts_ms: u64,
    file_offset: u64,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

/// Incremental disk-backed BGRA frame spool for an active recording.
pub struct RecordingSpool {
    width: u32,
    height: u32,
    fps: u32,
    scale_factor: f32,
    budget_bytes: u64,
    writer: BufWriter<NamedTempFile>,
    entries: Vec<SpoolEntry>,
    prev_bgra: Vec<u8>,
    spool_bytes: u64,
    captured_frames: usize,
    duration_ms: u64,
}

impl RecordingSpool {
    pub fn new(width: u32, height: u32, fps: u32, scale_factor: f32) -> Result<Self, String> {
        Self::with_budget(width, height, fps, scale_factor, DEFAULT_SPOOL_BUDGET_BYTES)
    }

    pub fn with_budget(
        width: u32,
        height: u32,
        fps: u32,
        scale_factor: f32,
        budget_bytes: u64,
    ) -> Result<Self, String> {
        if width < 2 || height < 2 {
            return Err("Recording region must be at least 2x2 pixels".into());
        }
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or("Recording dimensions overflow")?;
        if expected == 0 {
            return Err("Recording region is empty".into());
        }
        let file = NamedTempFile::new()
            .map_err(|e| format!("Could not create temporary recording spool: {e}"))?;
        Ok(Self {
            width,
            height,
            fps: fps.clamp(1, 60),
            scale_factor: if scale_factor.is_finite() && scale_factor > 0. {
                scale_factor
            } else {
                1.
            },
            budget_bytes,
            writer: BufWriter::with_capacity(256 * 1024, file),
            entries: Vec::new(),
            prev_bgra: Vec::new(),
            spool_bytes: 0,
            captured_frames: 0,
            duration_ms: 0,
        })
    }

    pub fn captured_frames(&self) -> usize {
        self.captured_frames
    }

    pub fn unique_frames(&self) -> usize {
        self.entries.len()
    }

    pub fn spool_bytes(&self) -> u64 {
        self.spool_bytes
    }

    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    /// Append one top-down BGRA frame at `pts_ms` relative to recording start.
    /// Identical consecutive frames cost zero spool bytes while advancing duration.
    pub fn push_bgra(
        &mut self,
        pts_ms: u64,
        bgra: Vec<u8>,
    ) -> Result<Result<(), RecordError>, String> {
        let expected = self.width as usize * self.height as usize * 4;
        if bgra.len() != expected {
            return Err(format!(
                "Captured frame size mismatch: expected {expected}, got {}",
                bgra.len()
            ));
        }
        if pts_ms > MAX_RECORD_DURATION_MS {
            return Ok(Err(RecordError::DurationLimit));
        }
        let pts_ms = pts_ms.max(self.duration_ms);
        let dirty = if self.entries.is_empty() {
            Some((0, 0, self.width, self.height))
        } else {
            diff_bgra_rect(&self.prev_bgra, &bgra, self.width, self.height)
        };
        if let Some((x, y, w, h)) = dirty {
            let patch_bytes = u64::from(w) * u64::from(h) * 4;
            if self.spool_bytes.saturating_add(patch_bytes) > self.budget_bytes {
                return Ok(Err(RecordError::StorageBudget));
            }
            let file_offset = self.spool_bytes;
            let stride = self.width as usize * 4;
            let row_start = x as usize * 4;
            let row_len = w as usize * 4;
            for row in y as usize..(y + h) as usize {
                let offset = row * stride + row_start;
                self.writer
                    .write_all(&bgra[offset..offset + row_len])
                    .map_err(|e| format!("Could not write recording frame: {e}"))?;
            }
            self.spool_bytes += patch_bytes;
            self.entries.push(SpoolEntry {
                pts_ms,
                file_offset,
                x,
                y,
                width: w,
                height: h,
            });
            self.prev_bgra = bgra;
        }
        self.captured_frames += 1;
        self.duration_ms = pts_ms;
        Ok(Ok(()))
    }

    /// Flush the spool and turn it into an immutable clip ready for MP4/GIF export.
    pub fn finish(mut self, final_elapsed_ms: u64) -> Result<RecordedClip, String> {
        if self.entries.is_empty() {
            return Err("No frames were recorded".into());
        }
        self.writer
            .flush()
            .map_err(|e| format!("Could not flush recording spool: {e}"))?;
        let file = self
            .writer
            .into_inner()
            .map_err(|e| format!("Could not finalize recording spool: {}", e.error()))?;
        let frame_period_ms = (1000 / u64::from(self.fps.max(1))).max(1);
        let duration_ms = final_elapsed_ms
            .max(self.duration_ms.saturating_add(frame_period_ms))
            .max(frame_period_ms);
        Ok(RecordedClip {
            width: self.width,
            height: self.height,
            fps: self.fps,
            scale_factor: self.scale_factor,
            duration_ms,
            captured_frames: self.captured_frames,
            spool_bytes: self.spool_bytes,
            entries: self.entries,
            file,
        })
    }
}

/// Immutable recorded clip backed by a temporary file that deletes itself on drop.
pub struct RecordedClip {
    width: u32,
    height: u32,
    fps: u32,
    scale_factor: f32,
    duration_ms: u64,
    captured_frames: usize,
    spool_bytes: u64,
    entries: Vec<SpoolEntry>,
    file: NamedTempFile,
}

impl RecordedClip {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    pub fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    pub fn captured_frames(&self) -> usize {
        self.captured_frames
    }

    pub fn unique_frames(&self) -> usize {
        self.entries.len()
    }

    pub fn spool_bytes(&self) -> u64 {
        self.spool_bytes
    }

    fn replay(
        &self,
        cancelled: &impl Fn() -> bool,
        mut on_frame: impl FnMut(usize, usize, u64, u64, &[u8]) -> Result<(), String>,
    ) -> Result<(), String> {
        let reader_file = File::open(self.file.path())
            .map_err(|e| format!("Could not open recording spool for export: {e}"))?;
        let mut reader = BufReader::with_capacity(256 * 1024, reader_file);
        let stride = self.width as usize * 4;
        let mut canvas = vec![0u8; stride * self.height as usize];
        let mut patch = Vec::new();
        let total = self.entries.len();

        for (index, entry) in self.entries.iter().enumerate() {
            if cancelled() {
                return Err("Export cancelled".into());
            }
            let row_len = entry.width as usize * 4;
            let patch_len = row_len * entry.height as usize;
            patch.resize(patch_len, 0);
            reader
                .seek(SeekFrom::Start(entry.file_offset))
                .map_err(|e| format!("Could not seek recording spool: {e}"))?;
            reader
                .read_exact(&mut patch)
                .map_err(|e| format!("Could not read recording frame: {e}"))?;
            let x_bytes = entry.x as usize * 4;
            for row in 0..entry.height as usize {
                let dst = (entry.y as usize + row) * stride + x_bytes;
                let src = row * row_len;
                canvas[dst..dst + row_len].copy_from_slice(&patch[src..src + row_len]);
            }
            let next_pts = self
                .entries
                .get(index + 1)
                .map_or(self.duration_ms, |next| next.pts_ms);
            let duration_ms = next_pts.saturating_sub(entry.pts_ms).max(1);
            on_frame(index, total, entry.pts_ms, duration_ms, &canvas)?;
        }
        Ok(())
    }
}

/// Export a recorded clip to an H.264 `.mp4` file using the OS native encoder.
pub fn export_mp4(
    clip: &RecordedClip,
    output: &Path,
    cancelled: impl Fn() -> bool,
    mut on_progress: impl FnMut(ExportProgress),
) -> Result<(), String> {
    let mut writer = Mp4Writer::create(
        output,
        Mp4Config {
            width: clip.width(),
            height: clip.height(),
            fps: clip.fps(),
            bitrate: None,
        },
    )?;
    clip.replay(&cancelled, |index, total, pts_ms, duration_ms, bgra| {
        writer.write_frame(bgra, pts_ms, duration_ms)?;
        on_progress(ExportProgress {
            completed: index + 1,
            total,
        });
        Ok(())
    })?;
    if cancelled() {
        return Err("Export cancelled".into());
    }
    writer.finish()
}

/// Export a recorded clip to an optimized, infinitely looping animated `.gif`.
pub fn export_gif(
    clip: &RecordedClip,
    output: &Path,
    preset: GifPreset,
    cancelled: impl Fn() -> bool,
    mut on_progress: impl FnMut(ExportProgress),
) -> Result<(), String> {
    let (out_w, out_h) = preset.target_dimensions(clip.width(), clip.height(), clip.scale_factor());
    let temp = temp_gif_path(output)?;
    let result = (|| -> Result<(), String> {
        let file =
            File::create(&temp).map_err(|e| format!("Could not create temporary GIF file: {e}"))?;
        let mut encoder = GifEncoder::new(
            BufWriter::with_capacity(256 * 1024, file),
            out_w as u16,
            out_h as u16,
            &[],
        )
        .map_err(|e| format!("Could not initialize GIF encoder: {e}"))?;
        encoder
            .set_repeat(Repeat::Infinite)
            .map_err(|e| format!("Could not configure GIF repeat: {e}"))?;

        let min_step_ms = (1000 / u64::from(preset.max_fps().max(1))).max(20);
        let speed = preset.quantize_speed();
        let total = clip.unique_frames();

        // Pending frame holds the latest downscaled RGB image and the accumulated
        // duration before the next emitted GIF frame.
        let mut pending: Option<(Vec<u8>, u64)> = None;
        let mut last_emitted_rgb: Option<Vec<u8>> = None;
        let mut residual_ms: i64 = 0;

        let mut emit_pending = |rgb: Vec<u8>,
                                duration_ms: u64,
                                last_emitted: &mut Option<Vec<u8>>,
                                encoder: &mut GifEncoder<BufWriter<File>>|
         -> Result<(), String> {
            let total_ms = duration_ms as i64 + residual_ms;
            let centis = ((total_ms + 5) / 10).max(2);
            residual_ms = total_ms - centis * 10;

            let mut remaining = centis as u64;
            let mut wrote_patch = false;
            while remaining > 0 {
                let chunk = remaining.min(u64::from(u16::MAX)) as u16;
                remaining -= u64::from(chunk);
                let mut frame = if !wrote_patch {
                    wrote_patch = true;
                    match last_emitted.as_deref() {
                        None => GifFrame::from_rgb_speed(out_w as u16, out_h as u16, &rgb, speed),
                        Some(prev) => {
                            let (dx, dy, dw, dh) =
                                diff_rgb_rect(prev, &rgb, out_w, out_h).unwrap_or((0, 0, 1, 1));
                            let patch = crop_rgb(&rgb, out_w, dx, dy, dw, dh);
                            let mut sub =
                                GifFrame::from_rgb_speed(dw as u16, dh as u16, &patch, speed);
                            sub.left = dx as u16;
                            sub.top = dy as u16;
                            sub
                        }
                    }
                } else {
                    // Long static hold split across u16::MAX centisecond blocks.
                    let pixel = &rgb[0..3];
                    let mut hold = GifFrame::from_rgb_speed(1, 1, pixel, speed);
                    hold.left = 0;
                    hold.top = 0;
                    hold
                };
                frame.dispose = DisposalMethod::Keep;
                frame.delay = chunk;
                encoder
                    .write_frame(&frame)
                    .map_err(|e| format!("Could not write GIF frame: {e}"))?;
            }
            *last_emitted = Some(rgb);
            Ok(())
        };

        clip.replay(&cancelled, |index, _, _pts_ms, duration_ms, bgra| {
            let rgb = bgra_to_scaled_rgb(bgra, clip.width(), clip.height(), out_w, out_h);
            match &mut pending {
                None => {
                    pending = Some((rgb, duration_ms));
                }
                Some((prev_rgb, prev_dur)) => {
                    if *prev_rgb == rgb {
                        *prev_dur = prev_dur.saturating_add(duration_ms);
                    } else if *prev_dur < min_step_ms {
                        // Coalesce overly dense frames while keeping total elapsed time exact.
                        *prev_rgb = rgb;
                        *prev_dur = prev_dur.saturating_add(duration_ms);
                    } else {
                        let (flush_rgb, flush_dur) = pending.replace((rgb, duration_ms)).unwrap();
                        emit_pending(flush_rgb, flush_dur, &mut last_emitted_rgb, &mut encoder)?;
                    }
                }
            }
            on_progress(ExportProgress {
                completed: index + 1,
                total,
            });
            Ok(())
        })?;

        if cancelled() {
            return Err("Export cancelled".into());
        }
        if let Some((flush_rgb, flush_dur)) = pending.take() {
            emit_pending(flush_rgb, flush_dur, &mut last_emitted_rgb, &mut encoder)?;
        }
        let mut writer = encoder
            .into_inner()
            .map_err(|e| format!("Could not finalize GIF encoder: {e}"))?;
        writer
            .flush()
            .map_err(|e| format!("Could not flush GIF file: {e}"))?;
        Ok(())
    })();

    if let Err(error) = result {
        let _ = std::fs::remove_file(&temp);
        return Err(error);
    }
    #[cfg(target_os = "windows")]
    if output.exists() {
        if let Err(error) = std::fs::remove_file(output) {
            let _ = std::fs::remove_file(&temp);
            return Err(format!("Cannot replace existing GIF file: {error}"));
        }
    }
    if let Err(error) = std::fs::rename(&temp, output) {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("Cannot move exported GIF to destination: {error}"));
    }
    Ok(())
}

/// Build a bounded RGBA preview thumbnail directly from a BGRA frame off the UI thread.
pub fn bgra_preview_rgba(
    bgra: &[u8],
    width: u32,
    height: u32,
    max_w: u32,
    max_h: u32,
) -> RgbaImage {
    let (out_w, out_h) = fit_dimensions(width, height, max_w, max_h);
    let mut out = RgbaImage::new(out_w, out_h);
    let dst = out.as_mut();
    for y in 0..out_h {
        let sy = ((u64::from(y) * u64::from(height)) / u64::from(out_h)) as usize;
        for x in 0..out_w {
            let sx = ((u64::from(x) * u64::from(width)) / u64::from(out_w)) as usize;
            let src = (sy * width as usize + sx) * 4;
            let idx = (y as usize * out_w as usize + x as usize) * 4;
            dst[idx] = bgra[src + 2];
            dst[idx + 1] = bgra[src + 1];
            dst[idx + 2] = bgra[src];
            dst[idx + 3] = 255;
        }
    }
    out
}

fn fit_dimensions(width: u32, height: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (1, 1);
    }
    if width <= max_w && height <= max_h {
        return (width, height);
    }
    let scale = (max_w as f64 / width as f64).min(max_h as f64 / height as f64);
    (
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    )
}

fn temp_gif_path(output: &Path) -> Result<PathBuf, String> {
    let parent = output
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create output directory: {e}"))?;
    }
    let pid = std::process::id();
    let seq = GIF_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(".rotor-gif-{pid}-{seq}.tmp.gif")))
}

fn diff_bgra_rect(
    prev: &[u8],
    curr: &[u8],
    width: u32,
    height: u32,
) -> Option<(u32, u32, u32, u32)> {
    let stride = width as usize * 4;
    let mut min_y = None;
    let mut max_y = 0usize;
    for y in 0..height as usize {
        let start = y * stride;
        let end = start + stride;
        if prev[start..end] != curr[start..end] {
            if min_y.is_none() {
                min_y = Some(y);
            }
            max_y = y;
        }
    }
    let min_y = min_y?;
    let mut min_x = width as usize;
    let mut max_x = 0usize;
    for y in min_y..=max_y {
        let row_offset = y * stride;
        let p_row = &prev[row_offset..row_offset + stride];
        let c_row = &curr[row_offset..row_offset + stride];
        if p_row == c_row {
            continue;
        }
        for x in 0..min_x {
            let i = x * 4;
            if p_row[i..i + 4] != c_row[i..i + 4] {
                min_x = x;
                break;
            }
        }
        for x in (max_x..width as usize).rev() {
            let i = x * 4;
            if p_row[i..i + 4] != c_row[i..i + 4] {
                max_x = x;
                break;
            }
        }
    }
    Some((
        min_x as u32,
        min_y as u32,
        (max_x - min_x + 1) as u32,
        (max_y - min_y + 1) as u32,
    ))
}

fn diff_rgb_rect(
    prev: &[u8],
    curr: &[u8],
    width: u32,
    height: u32,
) -> Option<(u32, u32, u32, u32)> {
    let stride = width as usize * 3;
    let mut min_y = None;
    let mut max_y = 0usize;
    for y in 0..height as usize {
        let start = y * stride;
        let end = start + stride;
        if prev[start..end] != curr[start..end] {
            if min_y.is_none() {
                min_y = Some(y);
            }
            max_y = y;
        }
    }
    let min_y = min_y?;
    let mut min_x = width as usize;
    let mut max_x = 0usize;
    for y in min_y..=max_y {
        let row_offset = y * stride;
        let p_row = &prev[row_offset..row_offset + stride];
        let c_row = &curr[row_offset..row_offset + stride];
        if p_row == c_row {
            continue;
        }
        for x in 0..min_x {
            let i = x * 3;
            if p_row[i..i + 3] != c_row[i..i + 3] {
                min_x = x;
                break;
            }
        }
        for x in (max_x..width as usize).rev() {
            let i = x * 3;
            if p_row[i..i + 3] != c_row[i..i + 3] {
                max_x = x;
                break;
            }
        }
    }
    Some((
        min_x as u32,
        min_y as u32,
        (max_x - min_x + 1) as u32,
        (max_y - min_y + 1) as u32,
    ))
}

fn crop_rgb(rgb: &[u8], width: u32, x: u32, y: u32, w: u32, h: u32) -> Vec<u8> {
    let stride = width as usize * 3;
    let row_bytes = w as usize * 3;
    let x_bytes = x as usize * 3;
    let mut out = Vec::with_capacity(row_bytes * h as usize);
    for row in y as usize..(y + h) as usize {
        let offset = row * stride + x_bytes;
        out.extend_from_slice(&rgb[offset..offset + row_bytes]);
    }
    out
}

fn bgra_to_scaled_rgb(bgra: &[u8], src_w: u32, src_h: u32, dst_w: u32, dst_h: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(dst_w as usize * dst_h as usize * 3);
    if src_w == dst_w && src_h == dst_h {
        for pixel in bgra.chunks_exact(4) {
            out.push(pixel[2]);
            out.push(pixel[1]);
            out.push(pixel[0]);
        }
        return out;
    }
    // Bilinear resize with BGRA -> RGB conversion for crisp downscaled UI text.
    let x_ratio = src_w as f64 / dst_w as f64;
    let y_ratio = src_h as f64 / dst_h as f64;
    let max_x = (src_w - 1) as usize;
    let max_y = (src_h - 1) as usize;
    let stride = src_w as usize * 4;
    for dy in 0..dst_h {
        let fy = ((dy as f64 + 0.5) * y_ratio - 0.5).clamp(0., max_y as f64);
        let y0 = fy.floor() as usize;
        let y1 = (y0 + 1).min(max_y);
        let wy = fy - y0 as f64;
        let wy0 = 1. - wy;
        let row0 = &bgra[y0 * stride..(y0 + 1) * stride];
        let row1 = &bgra[y1 * stride..(y1 + 1) * stride];
        for dx in 0..dst_w {
            let fx = ((dx as f64 + 0.5) * x_ratio - 0.5).clamp(0., max_x as f64);
            let x0 = fx.floor() as usize;
            let x1 = (x0 + 1).min(max_x);
            let wx = fx - x0 as f64;
            let wx0 = 1. - wx;
            let w00 = wx0 * wy0;
            let w10 = wx * wy0;
            let w01 = wx0 * wy;
            let w11 = wx * wy;
            let p00 = &row0[x0 * 4..x0 * 4 + 3];
            let p10 = &row0[x1 * 4..x1 * 4 + 3];
            let p01 = &row1[x0 * 4..x0 * 4 + 3];
            let p11 = &row1[x1 * 4..x1 * 4 + 3];
            let r = (p00[2] as f64 * w00
                + p10[2] as f64 * w10
                + p01[2] as f64 * w01
                + p11[2] as f64 * w11)
                .round() as u8;
            let g = (p00[1] as f64 * w00
                + p10[1] as f64 * w10
                + p01[1] as f64 * w01
                + p11[1] as f64 * w11)
                .round() as u8;
            let b = (p00[0] as f64 * w00
                + p10[0] as f64 * w10
                + p01[0] as f64 * w01
                + p11[0] as f64 * w11)
                .round() as u8;
            out.push(r);
            out.push(g);
            out.push(b);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_bgra(width: u32, height: u32, bgra: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::with_capacity(width as usize * height as usize * 4);
        for _ in 0..(width * height) {
            out.extend_from_slice(&bgra);
        }
        out
    }

    fn paint_rect(buf: &mut [u8], width: u32, rect: (u32, u32, u32, u32), bgra: [u8; 4]) {
        let (rx, ry, rw, rh) = rect;
        for y in ry..ry + rh {
            for x in rx..rx + rw {
                let i = ((y * width + x) * 4) as usize;
                buf[i..i + 4].copy_from_slice(&bgra);
            }
        }
    }

    #[test]
    fn spool_coalesces_duplicates_stores_dirty_patches_and_replays_exact_pixels() {
        let mut spool = RecordingSpool::new(16, 12, 20, 1.).unwrap();
        let f0 = solid_bgra(16, 12, [10, 20, 30, 255]);
        assert!(spool.push_bgra(0, f0.clone()).unwrap().is_ok());
        // Duplicate frame adds zero spool bytes.
        let bytes_after_first = spool.spool_bytes();
        assert!(spool.push_bgra(50, f0.clone()).unwrap().is_ok());
        assert_eq!(spool.spool_bytes(), bytes_after_first);
        assert_eq!(spool.captured_frames(), 2);
        assert_eq!(spool.unique_frames(), 1);

        // Small 4x3 dirty patch only appends 4*3*4 = 48 bytes.
        let mut f2 = f0.clone();
        paint_rect(&mut f2, 16, (4, 3, 4, 3), [200, 150, 50, 255]);
        assert!(spool.push_bgra(150, f2.clone()).unwrap().is_ok());
        assert_eq!(spool.spool_bytes(), bytes_after_first + 48);
        assert_eq!(spool.unique_frames(), 2);

        let clip = spool.finish(250).unwrap();
        assert_eq!(clip.duration_ms(), 250);
        let mut replayed = Vec::new();
        clip.replay(&|| false, |_, _, pts, dur, frame| {
            replayed.push((pts, dur, frame.to_vec()));
            Ok(())
        })
        .unwrap();
        assert_eq!(replayed.len(), 2);
        assert_eq!(replayed[0], (0, 150, f0));
        assert_eq!(replayed[1], (150, 100, f2));
    }

    #[test]
    fn spool_enforces_storage_budget_and_duration_limit() {
        let mut spool = RecordingSpool::with_budget(8, 8, 24, 1., 300).unwrap();
        let f0 = solid_bgra(8, 8, [0, 0, 0, 255]); // 256 bytes
        assert!(spool.push_bgra(0, f0).unwrap().is_ok());
        let f1 = solid_bgra(8, 8, [255, 0, 0, 255]); // +256 > 300
        assert_eq!(
            spool.push_bgra(40, f1).unwrap(),
            Err(RecordError::StorageBudget)
        );
        let mut spool = RecordingSpool::new(8, 8, 24, 1.).unwrap();
        assert_eq!(
            spool
                .push_bgra(MAX_RECORD_DURATION_MS + 1, solid_bgra(8, 8, [0, 0, 0, 255]))
                .unwrap(),
            Err(RecordError::DurationLimit)
        );
    }

    #[test]
    fn exports_valid_animated_gif_and_native_mp4_from_same_clip() {
        let mut spool = RecordingSpool::new(65, 49, 20, 2.).unwrap();
        let mut frame = solid_bgra(65, 49, [20, 40, 80, 255]);
        spool.push_bgra(0, frame.clone()).unwrap().unwrap();
        paint_rect(&mut frame, 65, (10, 10, 20, 15), [40, 220, 60, 255]);
        spool.push_bgra(120, frame.clone()).unwrap().unwrap();
        paint_rect(&mut frame, 65, (30, 20, 20, 15), [220, 60, 40, 255]);
        spool.push_bgra(260, frame).unwrap().unwrap();
        let clip = spool.finish(400).unwrap();

        let dir = tempfile::tempdir().unwrap();
        let gif_path = dir.path().join("clip.gif");
        let mut gif_progress = Vec::new();
        export_gif(
            &clip,
            &gif_path,
            GifPreset::Standard,
            || false,
            |p| gif_progress.push(p),
        )
        .unwrap();
        assert_eq!(gif_progress.last().unwrap().completed, 3);
        let gif_bytes = std::fs::read(&gif_path).unwrap();
        assert_eq!(&gif_bytes[..6], b"GIF89a");

        // Decode the GIF back to verify dimensions, frame count and timing.
        let mut decoder = gif::DecodeOptions::new();
        decoder.set_color_output(gif::ColorOutput::RGBA);
        let mut reader = decoder.read_info(std::io::Cursor::new(&gif_bytes)).unwrap();
        // Standard preset scales 2x Retina 65x49 -> 33x25 logical size.
        assert_eq!((reader.width(), reader.height()), (33, 25));
        let mut decoded_frames = 0usize;
        let mut total_delay_cs = 0u32;
        while let Some(frame) = reader.read_next_frame().unwrap() {
            decoded_frames += 1;
            total_delay_cs += u32::from(frame.delay);
        }
        assert_eq!(decoded_frames, 3);
        assert_eq!(total_delay_cs, 40); // 400ms == 40 centiseconds

        // Export the exact same clip to MP4 as well.
        let mp4_path = dir.path().join("clip.mp4");
        let mut mp4_progress = Vec::new();
        export_mp4(&clip, &mp4_path, || false, |p| mp4_progress.push(p)).unwrap();
        assert_eq!(mp4_progress.last().unwrap().completed, 3);
        let mp4_bytes = std::fs::read(&mp4_path).unwrap();
        assert!(mp4_bytes.len() > 256);
        assert_eq!(&mp4_bytes[4..8], b"ftyp");
    }

    #[test]
    fn gif_export_cancellation_leaves_no_partial_file() {
        let mut spool = RecordingSpool::new(32, 32, 20, 1.).unwrap();
        spool
            .push_bgra(0, solid_bgra(32, 32, [10, 20, 30, 255]))
            .unwrap()
            .unwrap();
        let clip = spool.finish(100).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let gif_path = dir.path().join("cancelled.gif");
        assert!(export_gif(&clip, &gif_path, GifPreset::High, || true, |_| {}).is_err());
        assert!(!gif_path.exists());
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
