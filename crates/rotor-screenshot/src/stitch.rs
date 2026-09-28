//! Vertical stitching for a manually scrolled, fixed viewport.
//! Frames are matched by their scroll displacement. Rows that stay in place
//! while the page scrolls (sticky headers, input boxes) appear once: the header
//! from the first frame and the footer from the latest frame. Small rendering
//! differences are tolerated; ambiguous or unrelated content fails without
//! changing the last accepted preview.
use image::RgbaImage;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

const MIN_OVERLAP: u32 = 8;
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
            Self::LimitExceeded => "Long image size or comparison limit reached",
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
    /// `overlap` counts the scrolling rows shared with the previous frame.
    Added {
        overlap: u32,
    },
    Unchanged,
    /// The content moved down. Nothing is added and the last accepted frame
    /// stays the reference, so scrolling down again continues the image.
    ScrolledBack,
}

struct Frame {
    image: Arc<RgbaImage>,
    edges: Vec<bool>,
}

pub struct StitchSession {
    preview: Arc<RgbaImage>,
    last: Frame,
    // The preview ends with rows `tail..` of the last frame (its current footer
    // strip); everything above them is the accumulated body.
    tail: u32,
    frames: usize,
}
impl StitchSession {
    pub fn new(first: Arc<RgbaImage>) -> Result<Self, StitchError> {
        validate_size(first.width(), first.height())?;
        if first.width() < 8 || first.height() < MIN_OVERLAP * 2 {
            return Err(StitchError::LimitExceeded);
        }
        let edges = edge_map(&first, &AtomicBool::new(false))?;
        Ok(Self {
            tail: first.height(),
            preview: first.clone(),
            last: Frame {
                image: first,
                edges,
            },
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
        let last = &self.last;
        if next.dimensions() != last.image.dimensions() {
            return Err(StitchError::DimensionsChanged);
        }
        if last.image.as_raw() == next.as_raw() {
            return Ok(AppendOutcome::Unchanged);
        }
        let next = Frame {
            edges: edge_map(&next, cancel)?,
            image: next,
        };
        let (width, height) = next.image.dimensions();
        let (top, bottom) = static_bands(&last.image, &next.image, cancel)?;
        let region = height - top - bottom;
        // "Not scrolled" competes with every displacement, so hover highlights
        // or a pointer moving over the page never count as a new frame.
        let displacement = find_displacement(last, &next, top, region, cancel)?;
        if displacement == 0 {
            return Ok(AppendOutcome::Unchanged);
        }
        let Ok(scrolled) = u32::try_from(displacement) else {
            return Ok(AppendOutcome::ScrolledBack);
        };
        // Footer rows found in this pair were counted as body rows of the
        // previous frame; drop them from the body and use the new footer strip.
        let footer_top = height - bottom;
        let end = self.tail.min(footer_top);
        if end < top + scrolled {
            return Err(StitchError::NoOverlap);
        }
        let start = end - scrolled;
        let kept = self.preview.height() - (height - end);
        let new_height = kept
            .checked_add(height - start)
            .ok_or(StitchError::LimitExceeded)?;
        validate_size(width, new_height)?;
        check_cancel(cancel)?;
        // Validation is complete: grow the accepted buffer in place when no
        // preview consumer still shares it.
        let stride = width as usize * 4;
        let previous = std::mem::replace(&mut self.preview, Arc::new(RgbaImage::new(0, 0)));
        let mut raw = Arc::try_unwrap(previous)
            .unwrap_or_else(|shared| (*shared).clone())
            .into_raw();
        raw.truncate(kept as usize * stride);
        let needed = new_height as usize * stride;
        if raw.capacity() < needed {
            let grown = (needed + needed / 2)
                .min(MAX_PIXELS as usize * 4)
                .max(needed);
            raw.reserve_exact(grown - raw.len());
        }
        raw.extend_from_slice(&next.image.as_raw()[start as usize * stride..]);
        self.preview = Arc::new(
            RgbaImage::from_raw(width, new_height, raw).ok_or(StitchError::LimitExceeded)?,
        );
        self.tail = footer_top;
        self.last = next;
        self.frames += 1;
        Ok(AppendOutcome::Added {
            overlap: region - scrolled,
        })
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

// RGB differences only: desktop capture alpha is not content. A small channel
// tolerance handles antialiasing; the separate edge check prevents large white
// backgrounds from hiding text that moved to a different row.
fn close(a: &[u8], b: &[u8]) -> bool {
    (0..3).all(|c| a[c].abs_diff(b[c]) <= 8)
}

// Computed once per frame; each frame is compared against its predecessor and
// successor at many offsets.
fn edge_map(image: &RgbaImage, cancel: &AtomicBool) -> Result<Vec<bool>, StitchError> {
    let (width, height) = (image.width() as usize, image.height() as usize);
    let raw = image.as_raw();
    let mut edges = Vec::with_capacity(width * height);
    for y in 0..height {
        check_cancel(cancel)?;
        for x in 0..width {
            let p = (y * width + x) * 4;
            let right = if x + 1 < width { p + 4 } else { p };
            let below = if y + 1 < height { p + width * 4 } else { p };
            edges.push((0..3).any(|c| {
                raw[p + c].abs_diff(raw[right + c]) >= 16
                    || raw[p + c].abs_diff(raw[below + c]) >= 16
            }));
        }
    }
    Ok(edges)
}

// Leading and trailing rows that did not move between the frames. Each band is
// capped so most of the viewport remains available for matching. Blank page
// content mistaken for a band only narrows the compared region: the output is
// still continuous because the body always ends where the footer strip starts.
fn static_bands(
    previous: &RgbaImage,
    next: &RgbaImage,
    cancel: &AtomicBool,
) -> Result<(u32, u32), StitchError> {
    let (width, height) = previous.dimensions();
    let stride = width as usize * 4;
    let still = |y: u32| {
        let row = y as usize * stride..(y as usize + 1) * stride;
        let bad = previous.as_raw()[row.clone()]
            .chunks_exact(4)
            .zip(next.as_raw()[row].chunks_exact(4))
            .filter(|(a, b)| !close(a, b))
            .count();
        // Allow a caret, scrollbar thumb or small hover mark inside the band.
        bad * 50 <= width as usize
    };
    let limit = height / 3;
    let mut top = 0;
    while top < limit && still(top) {
        check_cancel(cancel)?;
        top += 1;
    }
    let mut bottom = 0;
    while bottom < limit && still(height - 1 - bottom) {
        check_cancel(cancel)?;
        bottom += 1;
    }
    Ok((top, bottom))
}

const BAND: usize = 16;

// Compare `rows` rows starting at `previous_start` and `next_start`. Returns a
// mismatch score (lower is better) when the rows show the same content.
// Rows are judged in bands of 16: up to `1 / tolerance` of the bands may differ
// entirely, so a hover highlight, a floating button or an animated element
// does not reject an otherwise matching overlap. Failed bands raise the score.
// `step` samples every n-th row and column for a cheap first pass.
fn compare(
    previous: &Frame,
    next: &Frame,
    (previous_start, next_start, rows): (u32, u32, u32),
    (tolerance, step): (usize, usize),
    cancel: &AtomicBool,
    budget: &mut u64,
) -> Result<Option<f64>, StitchError> {
    let width = previous.image.width() as usize;
    let stride = width * 4;
    let (a, b) = (previous.image.as_raw(), next.image.as_raw());
    let rows = rows as usize;
    let bands = rows.div_ceil(BAND);
    let first = &b[next_start as usize * stride..][..3];
    let (mut bad, mut edges, mut bad_edges, mut compared, mut failed) = (0, 0, 0, 0, 0);
    let (mut exact, mut varied) = (true, false);
    for band in (0..rows).step_by(BAND) {
        let band_rows = BAND.min(rows - band);
        let (mut band_bad, mut band_edges, mut band_bad_edges) = (0, 0, 0);
        let mut pixels = 0;
        for y in (band..band + band_rows).step_by(step) {
            check_cancel(cancel)?;
            *budget = budget
                .checked_sub(width.div_ceil(step) as u64)
                .ok_or(StitchError::LimitExceeded)?;
            let pa = (previous_start as usize + y) * width;
            let pb = (next_start as usize + y) * width;
            let row_a = &a[pa * 4..pa * 4 + stride];
            let row_b = &b[pb * 4..pb * 4 + stride];
            exact &= row_a == row_b;
            for x in (0..width).step_by(step) {
                pixels += 1;
                let (pixel_a, pixel_b) = (&row_a[x * 4..x * 4 + 4], &row_b[x * 4..x * 4 + 4]);
                varied |= pixel_b[..3] != *first;
                let differs = !close(pixel_a, pixel_b);
                band_bad += usize::from(differs);
                if previous.edges[pa + x] || next.edges[pb + x] {
                    band_edges += 1;
                    band_bad_edges += usize::from(differs);
                }
            }
        }
        if band_bad * 5 > pixels || (band_edges >= 16 && band_bad_edges * 5 > band_edges) {
            failed += 1;
            if failed > bands / tolerance {
                return Ok(None);
            }
            continue;
        }
        bad += band_bad;
        edges += band_edges;
        bad_edges += band_bad_edges;
        compared += pixels;
    }
    Ok(
        if edges >= 16 && bad_edges * 100 <= edges * 10 && bad * 100 <= compared * 3 {
            Some(
                bad_edges as f64 / edges as f64
                    + bad as f64 / compared as f64
                    + failed as f64 / bands as f64,
            )
        } else if exact && varied {
            // Low-contrast content without edges can still match exactly, but a
            // blank strip is never evidence of a scrolling displacement.
            Some(0.)
        } else {
            None
        },
    )
}

// Search both scroll directions within the moving rows `top..top + region`.
// Every plausible displacement is scored; the best one must be clearly better
// than any non-adjacent alternative, otherwise repeated content is ambiguous.
fn find_displacement(
    previous: &Frame,
    next: &Frame,
    top: u32,
    region: u32,
    cancel: &AtomicBool,
) -> Result<i64, StitchError> {
    let minimum = MIN_OVERLAP.max((region / 8).min(64));
    let width = next.image.width();
    let w = width as usize;
    // Choose edge anchors in the incoming frame once. Comparing these across
    // each possible offset is bounded and rejects background-only coincidences.
    let anchors: Vec<Vec<u32>> = (0..next.image.height() as usize)
        .map(|y| {
            (0..8)
                .filter_map(|band| {
                    (band * width / 8..(band + 1) * width / 8)
                        .find(|&x| next.edges[y * w + x as usize])
                })
                .collect()
        })
        .collect();
    let grid: Vec<u32> = (0..8).map(|x| x * (width - 1) / 7).collect();
    fn pixel(image: &RgbaImage, x: u32, y: u32) -> &[u8] {
        let offset = (y as usize * image.width() as usize + x as usize) * 4;
        &image.as_raw()[offset..offset + 4]
    }
    let mut budget = COMPARE_BUDGET.max(u64::from(region) * u64::from(width) * 12);
    // Large regions are first scored on a sparse grid; only the offsets close
    // to the best coarse score are compared at full resolution.
    let pixels = u64::from(region) * u64::from(width);
    let step = if pixels > 1 << 20 {
        4
    } else if pixels > 1 << 18 {
        2
    } else {
        1
    };
    let mut coarse = Vec::new();
    // Without scrolling, the old and the new highlighted item both change;
    // compare the whole frame and allow more changed bands than for a scroll.
    let height = next.image.height();
    let still = (0, 0, height, 2);
    if let Some(score) = compare(
        previous,
        next,
        (0, 0, height),
        (2, step),
        cancel,
        &mut budget,
    )? {
        coarse.push((0, still, score));
    }
    for magnitude in 1..=region.saturating_sub(minimum) {
        for displacement in [i64::from(magnitude), -i64::from(magnitude)] {
            check_cancel(cancel)?;
            let (previous_start, next_start) = if displacement > 0 {
                (top + magnitude, top)
            } else {
                (top, top + magnitude)
            };
            let rows = region - magnitude;
            let samples = rows.min(32);
            let (mut count, mut bad) = (0, 0);
            let (mut grid_count, mut grid_bad) = (0, 0);
            for sample in 0..samples {
                let y = sample * (rows - 1) / (samples - 1);
                let (py, ny) = (previous_start + y, next_start + y);
                for &x in &anchors[ny as usize] {
                    count += 1;
                    bad += usize::from(!close(
                        pixel(&previous.image, x, py),
                        pixel(&next.image, x, ny),
                    ));
                }
                for &x in &grid {
                    grid_count += 1;
                    grid_bad += usize::from(!close(
                        pixel(&previous.image, x, py),
                        pixel(&next.image, x, ny),
                    ));
                }
            }
            // Sparse rejection is only a prefilter; remaining offsets are
            // compared in full. Edge-free content falls back to a fixed grid.
            let (count, bad) = if count >= 16 {
                (count, bad)
            } else {
                (grid_count, grid_bad)
            };
            // Tolerant enough for a hover band; wrong offsets on text pages
            // mismatch most anchors and are still rejected here.
            if bad * 100 > count * 30 {
                continue;
            }
            let range = (previous_start, next_start, rows, 4);
            if let Some(score) = compare(
                previous,
                next,
                (previous_start, next_start, rows),
                (4, step),
                cancel,
                &mut budget,
            )? {
                coarse.push((displacement, range, score));
            }
        }
    }
    let candidates = if step == 1 {
        coarse
            .into_iter()
            .map(|(displacement, _, score)| (displacement, score))
            .collect::<Vec<_>>()
    } else {
        coarse.sort_by(|a, b| a.2.total_cmp(&b.2));
        let limit = coarse.first().map_or(0., |best| best.2 * 2. + 0.05);
        let mut fine = Vec::new();
        for (displacement, (previous_start, next_start, rows, tolerance), _) in coarse
            .into_iter()
            .take_while(|candidate| candidate.2 <= limit)
            .take(6)
        {
            if let Some(score) = compare(
                previous,
                next,
                (previous_start, next_start, rows),
                (tolerance, 1),
                cancel,
                &mut budget,
            )? {
                fine.push((displacement, score));
            }
        }
        fine
    };
    let Some(&(best, score)) = candidates
        .iter()
        .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.abs().cmp(&b.0.abs())))
    else {
        return Err(StitchError::NoOverlap);
    };
    if candidates
        .iter()
        .any(|&(other, other_score)| (other - best).abs() > 1 && other_score <= score * 2. + 0.01)
    {
        return Err(StitchError::AmbiguousOverlap);
    }
    Ok(best)
}

/// Fit the whole image within `max_width` x `max_height`, preserving its aspect
/// ratio and never upscaling.
pub fn fit_preview(image: &RgbaImage, max_width: u32, max_height: u32) -> RgbaImage {
    scaled(image, 0, image.height(), max_width, max_height)
}

/// The newest rows of a long image, as many as fit the preview aspect ratio,
/// scaled to fit. Tall images stay readable instead of becoming a sliver.
pub fn tail_preview(image: &RgbaImage, max_width: u32, max_height: u32) -> RgbaImage {
    let rows = image
        .height()
        .min(image.width() * max_height / max_width.max(1))
        .max(1);
    scaled(image, image.height() - rows, rows, max_width, max_height)
}

// Averages up to 4x4 evenly spaced samples per output pixel. The cost depends
// on the preview size, not on the length of the stitched image.
fn scaled(image: &RgbaImage, top: u32, rows: u32, max_width: u32, max_height: u32) -> RgbaImage {
    let scale = (f64::from(max_width) / f64::from(image.width()))
        .min(f64::from(max_height) / f64::from(rows))
        .min(1.);
    let size = |value: u32| ((f64::from(value) * scale).round() as u32).max(1);
    let (width, height) = (size(image.width()), size(rows));
    let (step_x, step_y) = (
        f64::from(image.width()) / f64::from(width),
        f64::from(rows) / f64::from(height),
    );
    let (samples_x, samples_y) = (
        (step_x.ceil() as u32).clamp(1, 4),
        (step_y.ceil() as u32).clamp(1, 4),
    );
    RgbaImage::from_fn(width, height, |x, y| {
        let mut sum = [0u32; 4];
        for j in 0..samples_y {
            let offset = (f64::from(y) + (f64::from(j) + 0.5) / f64::from(samples_y)) * step_y;
            let source_y = top + (offset as u32).min(rows - 1);
            for i in 0..samples_x {
                let offset = (f64::from(x) + (f64::from(i) + 0.5) / f64::from(samples_x)) * step_x;
                let pixel = image.get_pixel((offset as u32).min(image.width() - 1), source_y);
                for (total, value) in sum.iter_mut().zip(pixel.0) {
                    *total += u32::from(value);
                }
            }
        }
        let count = samples_x * samples_y;
        image::Rgba(sum.map(|value| ((value + count / 2) / count) as u8))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> RgbaImage {
        RgbaImage::from_fn(384, 1200, |x, y| {
            let line = y / 24;
            let glyph = x / 10;
            let ink = x > 18
                && x < 350
                && y % 24 > 5
                && y % 24 < 18
                && x % 10 < 7
                && ((line * 31 + glyph * 17) ^ (y % 24 * 7 + x % 10)) % 5 < 3;
            image::Rgba(if ink {
                [40, 45, 50, 255]
            } else {
                [245, 245, 245, 255]
            })
        })
    }
    #[test]
    fn document_scroll_tolerates_rendering_noise_and_a_changed_scrollbar() {
        let source = document();
        let first = Arc::new(image::imageops::crop_imm(&source, 0, 0, 384, 400).to_image());
        let mut session = StitchSession::new(first).unwrap();
        let mut next = image::imageops::crop_imm(&source, 0, 137, 384, 400).to_image();
        for (x, y, pixel) in next.enumerate_pixels_mut() {
            for c in 0..3 {
                pixel[c] = pixel[c].saturating_sub(((x + y) % 4) as u8);
            }
            if x >= 380 {
                *pixel = image::Rgba([160, 160, 160, 255]);
            }
        }
        assert_eq!(
            session.append(Arc::new(next.clone()), &AtomicBool::new(false)),
            Ok(AppendOutcome::Added { overlap: 263 })
        );
        let result = session.preview();
        assert_eq!(result.height(), 537);
        assert_eq!(
            image::imageops::crop_imm(result.as_ref(), 0, 400, 384, 137).to_image(),
            image::imageops::crop_imm(&next, 0, 263, 384, 137).to_image()
        );
    }
    #[test]
    fn isolated_pixel_changes_do_not_block_scrolling_or_finishing() {
        let source = source();
        let mut session = StitchSession::new(frame(&source, 0)).unwrap();
        let mut next = (*frame(&source, 32)).clone();
        next.put_pixel(17, 17, image::Rgba([255, 0, 0, 255]));
        assert_eq!(
            session.append(Arc::new(next.clone()), &AtomicBool::new(false)),
            Ok(AppendOutcome::Added { overlap: 32 })
        );
        next.put_pixel(18, 18, image::Rgba([255, 0, 0, 255]));
        assert_eq!(
            session.append(Arc::new(next), &AtomicBool::new(false)),
            Ok(AppendOutcome::Unchanged)
        );
    }
    #[test]
    fn leaving_the_page_for_finish_can_hide_a_hover_control_without_adding_a_frame() {
        let source = document();
        let first = Arc::new(image::imageops::crop_imm(&source, 0, 0, 384, 400).to_image());
        let mut session = StitchSession::new(first.clone()).unwrap();
        let mut next = (*first).clone();
        for y in 365..385 {
            for x in 340..372 {
                next.put_pixel(x, y, image::Rgba([160, 160, 160, 255]));
            }
        }
        assert_eq!(
            session.append(Arc::new(next), &AtomicBool::new(false)),
            Ok(AppendOutcome::Unchanged)
        );
        assert_eq!(session.frame_count(), 1);
        assert!(Arc::ptr_eq(&session.preview(), &first));
        // Actual scrolling must still append pixels, even on a mostly white page.
        let scrolled = Arc::new(image::imageops::crop_imm(&source, 0, 5, 384, 400).to_image());
        assert!(matches!(
            session.append(scrolled, &AtomicBool::new(false)),
            Ok(AppendOutcome::Added { .. })
        ));
        assert_eq!(
            *session.preview(),
            image::imageops::crop_imm(&source, 0, 0, 384, 405).to_image()
        );
    }
    // A page with a sticky title bar and a fixed input box, like a chat log.
    fn chat_frame(source: &RgbaImage, scroll: u32, caret: bool) -> Arc<RgbaImage> {
        Arc::new(RgbaImage::from_fn(384, 400, |x, y| {
            let bar = |seed: u32| {
                let on = x % 12 < 7 && (y % 9) > 2 && !(x / 12 * 7 + seed).is_multiple_of(3);
                image::Rgba(if on {
                    [230, 230, 240, 255]
                } else {
                    [30, 34, 40, 255]
                })
            };
            if y < 40 {
                bar(1)
            } else if y >= 340 {
                if caret && x == 60 && (352..372).contains(&y) {
                    image::Rgba([255, 255, 255, 255])
                } else {
                    bar(2)
                }
            } else {
                *source.get_pixel(x, scroll + y - 40)
            }
        }))
    }
    #[test]
    fn sticky_header_and_footer_appear_once_around_the_scrolled_content() {
        let source = document();
        let cancel = AtomicBool::new(false);
        let mut session = StitchSession::new(chat_frame(&source, 0, false)).unwrap();
        for (scroll, caret) in [(120, true), (230, false), (300, true)] {
            assert!(matches!(
                session.append(chat_frame(&source, scroll, caret), &cancel),
                Ok(AppendOutcome::Added { .. })
            ));
        }
        let result = session.preview();
        let last = chat_frame(&source, 300, true);
        // Header once, the whole scrolled document, then the latest footer.
        assert_eq!(result.dimensions(), (384, 40 + 600 + 60));
        assert_eq!(
            image::imageops::crop_imm(result.as_ref(), 0, 0, 384, 40).to_image(),
            image::imageops::crop_imm(last.as_ref(), 0, 0, 384, 40).to_image()
        );
        assert_eq!(
            image::imageops::crop_imm(result.as_ref(), 0, 40, 384, 600).to_image(),
            image::imageops::crop_imm(&source, 0, 0, 384, 600).to_image()
        );
        assert_eq!(
            image::imageops::crop_imm(result.as_ref(), 0, 640, 384, 60).to_image(),
            image::imageops::crop_imm(last.as_ref(), 0, 340, 384, 60).to_image()
        );
    }
    #[test]
    fn scrolling_back_is_not_an_error_and_resumes_from_the_accepted_frame() {
        let source = document();
        let cancel = AtomicBool::new(false);
        let crop = |y| Arc::new(image::imageops::crop_imm(&source, 0, y, 384, 400).to_image());
        let mut session = StitchSession::new(crop(0)).unwrap();
        assert!(matches!(
            session.append(crop(200), &cancel),
            Ok(AppendOutcome::Added { .. })
        ));
        let accepted = session.preview();
        assert_eq!(
            session.append(crop(90), &cancel),
            Ok(AppendOutcome::ScrolledBack)
        );
        assert!(Arc::ptr_eq(&accepted, &session.preview()));
        drop(accepted);
        assert!(matches!(
            session.append(crop(410), &cancel),
            Ok(AppendOutcome::Added { .. })
        ));
        assert_eq!(
            *session.preview(),
            image::imageops::crop_imm(&source, 0, 0, 384, 810).to_image()
        );
        assert_eq!(session.frame_count(), 3);
    }
    #[test]
    fn similar_layouts_choose_the_offset_with_matching_text() {
        // Lines repeat every 48 rows except for a short distinct mark, as in a
        // list of similar messages; only the true offset matches the marks.
        let source = RgbaImage::from_fn(320, 1200, |x, y| {
            let line = y / 48;
            let row = y % 48;
            let ink = (10..22).contains(&row) && x % 9 < 6 && x > 20 && x < 280;
            let mark = (26..30).contains(&row) && x / 8 == 4 + line % 30;
            image::Rgba(if ink || mark {
                [40, 40, 40, 255]
            } else {
                [250, 250, 250, 255]
            })
        });
        let cancel = AtomicBool::new(false);
        let crop = |y| Arc::new(image::imageops::crop_imm(&source, 0, y, 320, 360).to_image());
        let mut session = StitchSession::new(crop(0)).unwrap();
        assert!(matches!(
            session.append(crop(100), &cancel),
            Ok(AppendOutcome::Added { .. })
        ));
        assert_eq!(
            *session.preview(),
            image::imageops::crop_imm(&source, 0, 0, 320, 460).to_image()
        );
    }
    // Tint the rows under the pointer, as list and chat items do on hover.
    fn hover(frame: &RgbaImage, rows: std::ops::Range<u32>) -> Arc<RgbaImage> {
        let mut frame = frame.clone();
        for y in rows {
            for x in 0..frame.width() {
                let pixel = frame.get_pixel_mut(x, y);
                for c in 0..3 {
                    pixel[c] = pixel[c].saturating_sub(24);
                }
            }
        }
        Arc::new(frame)
    }
    #[test]
    fn hover_highlights_neither_block_scrolling_nor_add_frames() {
        let source = document();
        let cancel = AtomicBool::new(false);
        let crop = |y| image::imageops::crop_imm(&source, 0, y, 384, 400).to_image();
        let mut session = StitchSession::new(Arc::new(crop(0))).unwrap();
        // The pointer stays still while the page scrolls under it, so a
        // different item is highlighted inside the overlap.
        assert!(matches!(
            session.append(hover(&crop(120), 150..198), &cancel),
            Ok(AppendOutcome::Added { .. })
        ));
        assert_eq!(session.preview().height(), 520);
        assert_eq!(
            image::imageops::crop_imm(session.preview().as_ref(), 0, 0, 384, 400).to_image(),
            crop(0)
        );
        // Moving the pointer without scrolling only changes the highlight.
        assert_eq!(
            session.append(hover(&crop(120), 250..298), &cancel),
            Ok(AppendOutcome::Unchanged)
        );
        assert_eq!(session.frame_count(), 2);
    }
    #[test]
    fn small_scrolls_on_sparse_pages_are_not_mistaken_for_hover() {
        // One short line of text every 120 rows on a white page.
        let source = RgbaImage::from_fn(384, 1600, |x, y| {
            let ink =
                y % 120 > 50 && y % 120 < 62 && x > 30 && x < 200 && (x / 7 + y / 120) % 3 != 0;
            image::Rgba(if ink {
                [50, 50, 50, 255]
            } else {
                [250, 250, 250, 255]
            })
        });
        let cancel = AtomicBool::new(false);
        let crop = |y| Arc::new(image::imageops::crop_imm(&source, 0, y, 384, 400).to_image());
        let mut session = StitchSession::new(crop(0)).unwrap();
        for y in [3, 6, 40] {
            assert!(
                matches!(
                    session.append(crop(y), &cancel),
                    Ok(AppendOutcome::Added { .. })
                ),
                "scroll to {y}"
            );
        }
        assert_eq!(
            *session.preview(),
            image::imageops::crop_imm(&source, 0, 0, 384, 440).to_image()
        );
    }
    #[test]
    fn previews_keep_aspect_and_average_samples() {
        let image = RgbaImage::from_fn(400, 8000, |x, y| {
            image::Rgba(if y < 4000 {
                [255, 0, 0, 255]
            } else if x % 2 == 0 {
                [0, 0, 0, 255]
            } else {
                [0, 0, 254, 255]
            })
        });
        let overview = fit_preview(&image, 96, 400);
        assert_eq!(overview.dimensions(), (20, 400));
        assert_eq!(overview.get_pixel(0, 0).0, [255, 0, 0, 255]);
        // Alternating columns average instead of aliasing to one of them.
        assert_eq!(overview.get_pixel(0, 399).0, [0, 0, 127, 255]);
        let tail = tail_preview(&image, 480, 400);
        assert_eq!(tail.dimensions(), (400, 333));
        assert_eq!(tail.get_pixel(1, 0).0, [0, 0, 254, 255]);
        assert_eq!(fit_preview(&image, 480, 400).dimensions(), (20, 400));
    }
    #[test]
    fn white_background_does_not_hide_unrelated_text_or_create_false_short_overlap() {
        let source = document();
        let first = Arc::new(image::imageops::crop_imm(&source, 0, 0, 384, 400).to_image());
        let mut session = StitchSession::new(first.clone()).unwrap();
        let unrelated = Arc::new(image::imageops::crop_imm(&source, 0, 700, 384, 400).to_image());
        assert!(session.append(unrelated, &AtomicBool::new(false)).is_err());
        assert!(Arc::ptr_eq(&first, &session.preview()));
    }
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
    fn small_scroll_steps_are_bounded_by_image_size_not_frame_count() {
        let source = source();
        let mut session = StitchSession::new(frame(&source, 0)).unwrap();
        let cancel = AtomicBool::new(false);
        for y in 1..=96 {
            assert_eq!(
                session.append(frame(&source, y), &cancel),
                Ok(AppendOutcome::Added { overlap: 63 })
            );
        }
        assert_eq!(session.frame_count(), 97);
        assert_eq!(*session.preview(), source);
        assert_eq!(
            validate_size(64, MAX_DIMENSION + 1),
            Err(StitchError::LimitExceeded)
        );
        assert_eq!(validate_size(8192, 4096), Err(StitchError::LimitExceeded));
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
        for y in 0..20 {
            for x in 0..64 {
                dynamic.put_pixel(x, y, image::Rgba([255, 0, 0, 255]));
            }
        }
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
