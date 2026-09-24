//! Conservative vertical stitching for a manually scrolled, fixed viewport.
//! No window enumeration/capture lives here. Ambiguous or changing overlaps
//! fail without changing the last accepted preview.
use image::RgbaImage;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

const MIN_OVERLAP: u32 = 8;
const MAX_FRAMES: usize = 64;
const MAX_PIXELS: u64 = 16 * 1024 * 1024;
const MAX_DIMENSION: u32 = 16384;
const COMPARE_BUDGET: u64 = 32 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum StitchError {
    Cancelled,
    DimensionsChanged,
    LimitExceeded,
    NoOverlap,
    AmbiguousOverlap,
}
impl std::fmt::Display for StitchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "Stitching cancelled",
            Self::DimensionsChanged => "Keep the same viewport size and display scale",
            Self::LimitExceeded => "Long image size, frame count or comparison limit reached",
            Self::NoOverlap => {
                "No stable overlap: scroll less and exclude fixed headers or changing content"
            }
            Self::AmbiguousOverlap => {
                "Repeated content makes the overlap ambiguous; capture a more distinctive region"
            }
        })
    }
}
impl std::error::Error for StitchError {}
#[derive(Debug, PartialEq, Eq)]
pub enum AppendOutcome {
    Added { overlap: u32 },
    Unchanged,
}

pub struct StitchSession {
    preview: Arc<RgbaImage>,
    last: Arc<RgbaImage>,
    frames: usize,
}
impl StitchSession {
    pub fn new(first: Arc<RgbaImage>) -> Result<Self, StitchError> {
        validate_size(first.width(), first.height())?;
        if first.width() < 8 || first.height() < MIN_OVERLAP * 2 {
            return Err(StitchError::LimitExceeded);
        }
        Ok(Self {
            preview: first.clone(),
            last: first,
            frames: 1,
        })
    }
    pub fn preview(&self) -> Arc<RgbaImage> {
        self.preview.clone()
    }
    pub fn frame_count(&self) -> usize {
        self.frames
    }
    pub fn append(
        &mut self,
        next: Arc<RgbaImage>,
        cancel: &AtomicBool,
    ) -> Result<AppendOutcome, StitchError> {
        check_cancel(cancel)?;
        if next.dimensions() != self.last.dimensions() {
            return Err(StitchError::DimensionsChanged);
        }
        if self.last.as_raw() == next.as_raw() {
            check_cancel(cancel)?;
            return Ok(AppendOutcome::Unchanged);
        }
        if self.frames >= MAX_FRAMES {
            return Err(StitchError::LimitExceeded);
        }
        let overlap = find_overlap(&self.last, &next, cancel)?;
        let height = self
            .preview
            .height()
            .checked_add(next.height() - overlap)
            .ok_or(StitchError::LimitExceeded)?;
        validate_size(next.width(), height)?;
        // Allocate only after validating the full size. Old preview and source
        // frames remain intact until the complete, uncancelled copy is accepted.
        let mut image = RgbaImage::new(next.width(), height);
        let stride = next.width() as usize * 4;
        for (row, target) in image.as_mut().chunks_exact_mut(stride).enumerate() {
            check_cancel(cancel)?;
            let (source, index) = if row < self.preview.height() as usize {
                (&self.preview, row)
            } else {
                (
                    &next,
                    row - self.preview.height() as usize + overlap as usize,
                )
            };
            target.copy_from_slice(&source.as_raw()[index * stride..(index + 1) * stride]);
        }
        check_cancel(cancel)?;
        self.preview = Arc::new(image);
        self.last = next;
        self.frames += 1;
        Ok(AppendOutcome::Added { overlap })
    }
}
fn validate_size(width: u32, height: u32) -> Result<(), StitchError> {
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        Err(StitchError::LimitExceeded)
    } else {
        Ok(())
    }
}
fn check_cancel(cancel: &AtomicBool) -> Result<(), StitchError> {
    if cancel.load(Ordering::Relaxed) {
        Err(StitchError::Cancelled)
    } else {
        Ok(())
    }
}
fn find_overlap(
    previous: &RgbaImage,
    next: &RgbaImage,
    cancel: &AtomicBool,
) -> Result<u32, StitchError> {
    let mut found = None;
    let mut budget = COMPARE_BUDGET;
    for overlap in MIN_OVERLAP..previous.height() {
        check_cancel(cancel)?;
        let offset = previous.height() - overlap;
        // Sparse rejection is only a prefilter; every accepted pixel is checked.
        let sampled = (0..8).all(|y| {
            (0..8).all(|x| {
                let x = x * (previous.width() - 1) / 7;
                let y = y * (overlap - 1) / 7;
                previous.get_pixel(x, offset + y) == next.get_pixel(x, y)
            })
        });
        if !sampled {
            continue;
        }
        let stride = previous.width() as usize * 4;
        let mut equal = true;
        for y in 0..overlap {
            check_cancel(cancel)?;
            budget = budget
                .checked_sub(u64::from(previous.width()))
                .ok_or(StitchError::LimitExceeded)?;
            let a = (offset + y) as usize * stride;
            let b = y as usize * stride;
            if previous.as_raw()[a..a + stride] != next.as_raw()[b..b + stride] {
                equal = false;
                break;
            }
        }
        if equal {
            if found.is_some() {
                return Err(StitchError::AmbiguousOverlap);
            }
            found = Some(overlap);
        }
    }
    found.ok_or(StitchError::NoOverlap)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn source() -> RgbaImage {
        RgbaImage::from_fn(64, 160, |x, y| {
            let n = (x.wrapping_mul(73856093)) ^ y.wrapping_mul(19349663);
            image::Rgba([n as u8, (n >> 8) as u8, (n >> 16) as u8, 255])
        })
    }
    fn frame(source: &RgbaImage, y: u32) -> Arc<RgbaImage> {
        Arc::new(image::imageops::crop_imm(source, 0, y, 64, 64).to_image())
    }
    #[test]
    fn manual_frames_reconstruct_pixels_and_skip_unchanged_captures() {
        let source = source();
        let first = frame(&source, 0);
        let mut session = StitchSession::new(first.clone()).unwrap();
        let cancel = AtomicBool::new(false);
        assert_eq!(session.append(first, &cancel), Ok(AppendOutcome::Unchanged));
        assert_eq!(
            session.append(frame(&source, 32), &cancel),
            Ok(AppendOutcome::Added { overlap: 32 })
        );
        assert_eq!(
            session.append(frame(&source, 64), &cancel),
            Ok(AppendOutcome::Added { overlap: 32 })
        );
        assert_eq!(
            *session.preview(),
            image::imageops::crop_imm(&source, 0, 0, 64, 128).to_image()
        );
        assert_eq!(session.frame_count(), 3);
    }
    #[test]
    fn dynamic_content_fixed_headers_and_scale_changes_leave_preview_intact() {
        let source = source();
        let first = frame(&source, 0);
        let mut session = StitchSession::new(first.clone()).unwrap();
        let cancel = AtomicBool::new(false);
        let mut dynamic = (*frame(&source, 32)).clone();
        dynamic.put_pixel(17, 17, image::Rgba([255, 0, 0, 255]));
        assert_eq!(
            session.append(Arc::new(dynamic), &cancel),
            Err(StitchError::NoOverlap)
        );
        let mut header = (*frame(&source, 32)).clone();
        for x in 0..64 {
            header.put_pixel(x, 0, image::Rgba([0, 0, 0, 255]));
        }
        assert_eq!(
            session.append(Arc::new(header), &cancel),
            Err(StitchError::NoOverlap)
        );
        assert_eq!(
            session.append(Arc::new(RgbaImage::new(65, 64)), &cancel),
            Err(StitchError::DimensionsChanged)
        );
        assert_eq!(
            session.append(frame(&source, 32), &AtomicBool::new(true)),
            Err(StitchError::Cancelled)
        );
        assert!(Arc::ptr_eq(&first, &session.preview()));
        assert_eq!(session.frame_count(), 1);
    }
    #[test]
    fn repeating_rows_are_rejected_instead_of_selecting_an_arbitrary_offset() {
        let a = Arc::new(RgbaImage::from_fn(32, 64, |x, y| {
            image::Rgba([x as u8, (y % 8) as u8, 0, 255])
        }));
        let b = Arc::new(RgbaImage::from_fn(32, 64, |x, y| {
            image::Rgba([x as u8, ((y + 3) % 8) as u8, 0, 255])
        }));
        let mut session = StitchSession::new(a.clone()).unwrap();
        assert_eq!(
            session.append(b, &AtomicBool::new(false)),
            Err(StitchError::AmbiguousOverlap)
        );
        assert!(Arc::ptr_eq(&a, &session.preview()));
        assert!(validate_size(16384, 16384).is_err());
        assert!(validate_size(64, 16385).is_err());
    }
}
