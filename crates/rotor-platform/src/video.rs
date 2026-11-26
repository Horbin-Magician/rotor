//! Native hardware/OS H.264 MP4 video encoding for macOS and Windows.
//!
//! macOS uses AVFoundation (`AVAssetWriter`); Windows uses Media Foundation
//! (`IMFSinkWriter`). Writers must be created, fed frames and finished on the
//! same worker thread.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "macos")]
use macos as native;
#[cfg(target_os = "windows")]
use windows as native;

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mp4Config {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub bitrate: Option<u32>,
}

impl Mp4Config {
    pub fn effective_fps(self) -> u32 {
        self.fps.clamp(1, 60)
    }

    pub fn effective_bitrate(self, encoded_width: u32, encoded_height: u32) -> u32 {
        self.bitrate.unwrap_or_else(|| {
            let pixels = u64::from(encoded_width) * u64::from(encoded_height);
            let bps = pixels * u64::from(self.effective_fps()) / 7;
            bps.clamp(1_000_000, 16_000_000) as u32
        })
    }
}

/// Normalize to H.264-compatible even dimensions by trimming at most 1 pixel
/// from odd width or height.
pub fn even_dimensions(width: u32, height: u32) -> Result<(u32, u32), String> {
    let encoded_width = width & !1;
    let encoded_height = height & !1;
    if encoded_width < 2 || encoded_height < 2 {
        return Err("Video dimensions must be at least 2x2 pixels".into());
    }
    Ok((encoded_width, encoded_height))
}

fn temp_output_path(output: &Path) -> Result<PathBuf, String> {
    let parent = output
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if !parent.exists() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Cannot create output directory: {e}"))?;
    }
    let pid = std::process::id();
    let seq = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(".rotor-video-{pid}-{seq}.tmp.mp4")))
}

fn commit_temp_file(temp: &Path, output: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    if output.exists() {
        std::fs::remove_file(output)
            .map_err(|e| format!("Cannot replace existing output file: {e}"))?;
    }
    std::fs::rename(temp, output)
        .map_err(|e| format!("Cannot move exported video to destination: {e}"))
}

pub struct Mp4Writer {
    source_width: u32,
    source_height: u32,
    encoded_width: u32,
    encoded_height: u32,
    output: PathBuf,
    temp: PathBuf,
    inner: Option<native::NativeMp4Writer>,
}

impl Mp4Writer {
    pub fn create(output: &Path, config: Mp4Config) -> Result<Self, String> {
        if config.width == 0 || config.height == 0 {
            return Err("Video dimensions cannot be zero".into());
        }
        let (encoded_width, encoded_height) = even_dimensions(config.width, config.height)?;
        let temp = temp_output_path(output)?;
        let fps = config.effective_fps();
        let bitrate = config.effective_bitrate(encoded_width, encoded_height);
        let inner = match native::NativeMp4Writer::create(
            &temp,
            encoded_width,
            encoded_height,
            fps,
            bitrate,
        ) {
            Ok(writer) => writer,
            Err(error) => {
                let _ = std::fs::remove_file(&temp);
                return Err(error);
            }
        };
        Ok(Self {
            source_width: config.width,
            source_height: config.height,
            encoded_width,
            encoded_height,
            output: output.to_path_buf(),
            temp,
            inner: Some(inner),
        })
    }

    pub fn encoded_dimensions(&self) -> (u32, u32) {
        (self.encoded_width, self.encoded_height)
    }

    /// Encode one top-down BGRA frame (`source_width * source_height * 4` bytes).
    /// `pts_ms` is the presentation timestamp in milliseconds; `duration_ms` is
    /// how long the frame remains visible.
    pub fn write_frame(
        &mut self,
        bgra: &[u8],
        pts_ms: u64,
        duration_ms: u64,
    ) -> Result<(), String> {
        let expected = (self.source_width as usize)
            .checked_mul(self.source_height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or("Frame buffer dimensions overflow")?;
        if bgra.len() != expected {
            return Err(format!(
                "Invalid BGRA frame length: expected {expected}, got {}",
                bgra.len()
            ));
        }
        let writer = self
            .inner
            .as_mut()
            .ok_or("MP4 writer is already finished")?;
        writer.write_frame(
            bgra,
            self.source_width as usize * 4,
            self.encoded_width,
            self.encoded_height,
            pts_ms,
            duration_ms.max(1),
        )
    }

    pub fn finish(mut self) -> Result<(), String> {
        let writer = self.inner.take().ok_or("MP4 writer is already finished")?;
        if let Err(error) = writer.finish() {
            let _ = std::fs::remove_file(&self.temp);
            return Err(error);
        }
        if let Err(error) = commit_temp_file(&self.temp, &self.output) {
            let _ = std::fs::remove_file(&self.temp);
            return Err(error);
        }
        Ok(())
    }
}

impl Drop for Mp4Writer {
    fn drop(&mut self) {
        if let Some(writer) = self.inner.take() {
            writer.cancel();
            let _ = std::fs::remove_file(&self.temp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_dimensions_trims_odd_sizes_and_rejects_tiny_inputs() {
        assert_eq!(even_dimensions(1920, 1080).unwrap(), (1920, 1080));
        assert_eq!(even_dimensions(301, 203).unwrap(), (300, 202));
        assert_eq!(even_dimensions(3, 3).unwrap(), (2, 2));
        assert!(even_dimensions(1, 100).is_err());
        assert!(even_dimensions(100, 0).is_err());
    }

    #[test]
    fn encodes_synthetic_mp4_and_removes_temp_on_cancel() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("synthetic.mp4");
        let (w, h) = (65u32, 49u32);
        let mut writer = Mp4Writer::create(
            &output,
            Mp4Config {
                width: w,
                height: h,
                fps: 15,
                bitrate: None,
            },
        )
        .unwrap();
        assert_eq!(writer.encoded_dimensions(), (64, 48));
        for index in 0u64..10 {
            let mut frame = vec![0u8; (w * h * 4) as usize];
            for pixel in frame.chunks_exact_mut(4) {
                pixel[0] = (index * 24) as u8;
                pixel[1] = 120;
                pixel[2] = 220u8.saturating_sub((index * 20) as u8);
                pixel[3] = 255;
            }
            writer.write_frame(&frame, index * 66, 66).unwrap();
        }
        writer.finish().unwrap();
        let bytes = std::fs::read(&output).unwrap();
        assert!(bytes.len() > 256, "MP4 output should be non-empty");
        assert_eq!(&bytes[4..8], b"ftyp", "MP4 must begin with ftyp box");

        // Cancelling (dropping without finish) must not leave output or temp files.
        let cancelled = dir.path().join("cancelled.mp4");
        {
            let _writer = Mp4Writer::create(
                &cancelled,
                Mp4Config {
                    width: 64,
                    height: 48,
                    fps: 15,
                    bitrate: None,
                },
            )
            .unwrap();
        }
        assert!(!cancelled.exists());
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name())
            .collect();
        assert_eq!(leftovers, vec![std::ffi::OsString::from("synthetic.mp4")]);
    }
}
