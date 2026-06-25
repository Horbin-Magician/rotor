use image::{self, DynamicImage, GrayImage, RgbaImage};
use oar_ocr::domain::TextRegion;
use oar_ocr::oarocr::OAROCRBuilder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::cmp;
use std::path::Path;

pub fn detect_rect(original_img: &RgbaImage) -> Vec<(u32, u32, u32, u32)> {
    let original_width = original_img.width();
    let original_height = original_img.height();
    let scale_factor = calculate_optimal_scale_factor(original_img.width(), original_img.height());
    let gray = image_to_scaled_gray(original_img, scale_factor);
    let edge_image = canny_edge_detection(&gray, 10.0, 30.0);

    let morph_size = cmp::max(1, 4 / scale_factor) as u8;
    let processed_image = morphological_close(edge_image, morph_size);

    let min_size = 100 / scale_factor;
    let rects = find_bounding_boxes(&processed_image, min_size);

    // 6. Rescale back
    rects
        .into_iter()
        .map(|(x, y, w, h)| {
            let left = x * scale_factor;
            let top = y * scale_factor;
            let right = ((x + w) * scale_factor).min(original_width);
            let bottom = ((y + h) * scale_factor).min(original_height);

            (left, top, right - left, bottom - top)
        })
        .collect()
}

fn calculate_optimal_scale_factor(width: u32, height: u32) -> u32 {
    let max_dimension = width.max(height);
    match max_dimension {
        0..=1000 => 1,
        1001..=2000 => 2,
        2001..=4000 => 3,
        _ => 4,
    }
}

fn image_to_scaled_gray(img: &RgbaImage, scale_factor: u32) -> GrayImage {
    let src_width = img.width();
    let src_height = img.height();
    let dst_width = src_width.div_ceil(scale_factor);
    let dst_height = src_height.div_ceil(scale_factor);
    let img_data = img.as_raw();

    let mut gray_data = vec![0u8; (dst_width * dst_height) as usize];
    let src_width_usize = src_width as usize;
    let scale_factor_usize = scale_factor as usize;

    gray_data
        .par_chunks_mut(dst_width as usize)
        .enumerate()
        .for_each(|(dst_y, row)| {
            let src_y = (dst_y * scale_factor_usize).min(src_height as usize - 1);
            let src_row_offset = src_y * src_width_usize * 4;

            for (dst_x, gray) in row.iter_mut().enumerate() {
                let src_x = (dst_x * scale_factor_usize).min(src_width_usize - 1);
                let src_offset = src_row_offset + src_x * 4;

                unsafe {
                    // SAFETY: src_x/src_y are clamped to src_width-1 / src_height-1, so
                    // src_offset + 2 stays within img_data (length src_width*src_height*4).
                    let r = *img_data.get_unchecked(src_offset) as u32;
                    let g = *img_data.get_unchecked(src_offset + 1) as u32;
                    let b = *img_data.get_unchecked(src_offset + 2) as u32;
                    // Y = 0.299R + 0.587G + 0.114B
                    *gray = ((r * 299 + g * 587 + b * 114) / 1000) as u8;
                }
            }
        });

    GrayImage::from_raw(dst_width, dst_height, gray_data).unwrap_or_else(|| {
        log::error!("Failed to create grayscale image from resized buffer");
        GrayImage::new(dst_width, dst_height)
    })
}

fn canny_edge_detection(img: &GrayImage, low_threshold: f32, high_threshold: f32) -> GrayImage {
    let (width, height) = img.dimensions();
    let mut result = GrayImage::new(width, height);
    let img_data = img.as_raw();
    let res_data = result.as_mut();

    let high_sq = (high_threshold * high_threshold) as i32;
    let low_sq = (low_threshold * low_threshold) as i32;
    let width_usize = width as usize;
    res_data
        .par_chunks_mut(width_usize)
        .enumerate()
        .for_each(|(y, row)| {
            if y > 0 && y < (height as usize - 1) {
                for (x, pixel) in row.iter_mut().enumerate().take(width_usize - 1).skip(1) {
                    let idx = |dx: i32, dy: i32| -> usize {
                        (y as i32 + dy) as usize * width_usize + (x as i32 + dx) as usize
                    };

                    unsafe {
                        // Sobel X
                        let gx = (*img_data.get_unchecked(idx(1, -1)) as i32
                            + 2 * *img_data.get_unchecked(idx(1, 0)) as i32
                            + *img_data.get_unchecked(idx(1, 1)) as i32)
                            - (*img_data.get_unchecked(idx(-1, -1)) as i32
                                + 2 * *img_data.get_unchecked(idx(-1, 0)) as i32
                                + *img_data.get_unchecked(idx(-1, 1)) as i32);

                        // Sobel Y
                        let gy = (*img_data.get_unchecked(idx(-1, 1)) as i32
                            + 2 * *img_data.get_unchecked(idx(0, 1)) as i32
                            + *img_data.get_unchecked(idx(1, 1)) as i32)
                            - (*img_data.get_unchecked(idx(-1, -1)) as i32
                                + 2 * *img_data.get_unchecked(idx(0, -1)) as i32
                                + *img_data.get_unchecked(idx(1, -1)) as i32);

                        let mag_sq = gx * gx + gy * gy;

                        *pixel = if mag_sq > high_sq {
                            255
                        } else if mag_sq > low_sq {
                            128
                        } else {
                            0
                        };
                    }
                }
            }
        });

    result
}

fn morphological_close(img: GrayImage, size: u8) -> GrayImage {
    if size == 0 {
        return img;
    }
    let (width, height) = img.dimensions();

    let mut current_buf = img.into_raw();
    let mut next_buf = current_buf.clone(); // 只需要一次 clone
    let w = width as usize;
    let h = height as usize;

    let process_pass = |src: &[u8], dst: &mut [u8], is_dilate: bool| {
        dst.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            if y > 0 && y < h - 1 {
                for x in 1..w - 1 {
                    let mut val = src[y * w + x];
                    // 3x3 kernel
                    for dy in -1isize..=1 {
                        for dx in -1isize..=1 {
                            let neighbor =
                                src[(y as isize + dy) as usize * w + (x as isize + dx) as usize];
                            if is_dilate {
                                if neighbor > val {
                                    val = neighbor;
                                }
                            } else if neighbor < val {
                                val = neighbor;
                            }
                        }
                    }
                    row[x] = val;
                }
            }
        });
    };

    // Dilate loop
    for _ in 0..size {
        process_pass(&current_buf, &mut next_buf, true);
        // Swap buffers
        std::mem::swap(&mut current_buf, &mut next_buf);
    }

    // Erode loop
    for _ in 0..size {
        process_pass(&current_buf, &mut next_buf, false);
        std::mem::swap(&mut current_buf, &mut next_buf);
    }

    GrayImage::from_raw(width, height, current_buf).unwrap_or_else(|| {
        log::error!("Failed to create grayscale image from morph buffer");
        GrayImage::new(width, height)
    })
}

fn find_bounding_boxes(img: &GrayImage, min_size: u32) -> Vec<(u32, u32, u32, u32)> {
    let (width, height) = img.dimensions();
    let mut visited = vec![false; (width * height) as usize];
    let mut boxes = Vec::new();
    let img_data = img.as_raw();

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if img_data[idx] > 64 && !visited[idx] {
                if let Some(rect) =
                    flood_fill_bbox(img_data, &mut visited, x, y, width, height, min_size)
                {
                    boxes.push(rect);
                }
            }
        }
    }

    boxes
}

fn flood_fill_bbox(
    img_data: &[u8],
    visited: &mut [bool],
    start_x: u32,
    start_y: u32,
    width: u32,
    height: u32,
    min_size: u32,
) -> Option<(u32, u32, u32, u32)> {
    let w_usize = width as usize;
    let start_idx = start_y as usize * w_usize + start_x as usize;
    let mut stack = Vec::with_capacity(512);
    stack.push(start_idx);
    visited[start_idx] = true;

    let mut min_x = start_x;
    let mut max_x = start_x;
    let mut min_y = start_y;
    let mut max_y = start_y;
    let mut pixel_count = 0;

    while let Some(idx) = stack.pop() {
        let x = (idx % w_usize) as u32;
        let y = (idx / w_usize) as u32;

        // 更新 Bbox
        if x < min_x {
            min_x = x;
        }
        if x > max_x {
            max_x = x;
        }
        if y < min_y {
            min_y = y;
        }
        if y > max_y {
            max_y = y;
        }
        pixel_count += 1;

        // 4-connectivity neighbor check
        // Right
        if x + 1 < width {
            let n_idx = idx + 1;
            if !visited[n_idx] && img_data[n_idx] > 64 {
                visited[n_idx] = true;
                stack.push(n_idx);
            }
        }
        // Left
        if x > 0 {
            let n_idx = idx - 1;
            if !visited[n_idx] && img_data[n_idx] > 64 {
                visited[n_idx] = true;
                stack.push(n_idx);
            }
        }
        // Down
        if y + 1 < height {
            let n_idx = idx + w_usize;
            if !visited[n_idx] && img_data[n_idx] > 64 {
                visited[n_idx] = true;
                stack.push(n_idx);
            }
        }
        // Up
        if y > 0 {
            let n_idx = idx - w_usize;
            if !visited[n_idx] && img_data[n_idx] > 64 {
                visited[n_idx] = true;
                stack.push(n_idx);
            }
        }
    }

    let w = max_x - min_x + 1;
    let h = max_y - min_y + 1;

    // 过滤过小的区域
    if w < min_size || h < min_size || pixel_count < min_size * min_size / 4 {
        return None;
    }

    Some((min_x, min_y, w, h))
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TextResult {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub text: String,
}

const LINE_MIN_VERTICAL_OVERLAP: f32 = 0.45;
const LINE_CENTER_TOLERANCE: f32 = 0.35;
const LINE_MAX_HEIGHT_RATIO: f32 = 2.4;
const MERGE_MAX_HORIZONTAL_GAP_RATIO: f32 = 1.0;
const MERGE_MIN_HORIZONTAL_GAP: i32 = 10;
const ASCII_WORD_SPACE_GAP_RATIO: f32 = 0.18;
const SYMBOL_SPACE_GAP_RATIO: f32 = 0.35;

#[derive(Debug)]
struct TextLine {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    items: Vec<TextResult>,
}

impl TextLine {
    fn new(result: TextResult) -> Self {
        let left = result.left;
        let top = result.top;
        let right = result_right(&result);
        let bottom = result_bottom(&result);

        Self {
            left,
            top,
            right,
            bottom,
            items: vec![result],
        }
    }

    fn push(&mut self, result: TextResult) {
        self.left = cmp::min(self.left, result.left);
        self.top = cmp::min(self.top, result.top);
        self.right = cmp::max(self.right, result_right(&result));
        self.bottom = cmp::max(self.bottom, result_bottom(&result));
        self.items.push(result);
    }

    fn height(&self) -> i32 {
        cmp::max(1, self.bottom.saturating_sub(self.top))
    }

    fn center_y_times_two(&self) -> i32 {
        self.top.saturating_add(self.bottom)
    }
}

pub fn img2text(
    model_path: &Path,
    img: &DynamicImage,
) -> Result<Vec<TextResult>, Box<dyn std::error::Error>> {
    let det_model_path = model_path.join("pp-ocrv6_tiny_det.onnx");
    let rec_model_path = model_path.join("pp-ocrv6_tiny_rec.onnx");
    let dict_path = model_path.join("ppocrv6_tiny_dict.txt");

    let ocr = OAROCRBuilder::new(det_model_path, rec_model_path, dict_path)
        .image_batch_size(1)
        .region_batch_size(32)
        .build()?;
    let Some(result) = ocr.predict(vec![img.to_rgb8()])?.into_iter().next() else {
        return Ok(Vec::new());
    };

    let text_results = result
        .text_regions
        .iter()
        .filter_map(text_region_to_result)
        .collect();

    Ok(merge_text_results(text_results))
}

fn text_region_to_result(region: &TextRegion) -> Option<TextResult> {
    let text = region.text.as_deref()?.trim().to_string();
    if text.is_empty() {
        return None;
    }

    let (min_x, min_y, max_x, max_y) = region.bounding_box.aabb();
    let left = min_x.floor().max(0.0) as i32;
    let top = min_y.floor().max(0.0) as i32;
    let right = max_x.ceil().max(left as f32) as i32;
    let bottom = max_y.ceil().max(top as f32) as i32;
    let width = right.saturating_sub(left) as u32;
    let height = bottom.saturating_sub(top) as u32;

    (width > 0 && height > 0).then_some(TextResult {
        left,
        top,
        width,
        height,
        text,
    })
}

fn merge_text_results(results: Vec<TextResult>) -> Vec<TextResult> {
    let mut results: Vec<TextResult> = results
        .into_iter()
        .filter_map(|mut result| {
            result.text = result.text.trim().to_string();
            (!result.text.is_empty()).then_some(result)
        })
        .collect();

    if results.len() <= 1 {
        return results;
    }

    results.sort_by(|a, b| {
        result_center_y_times_two(a)
            .cmp(&result_center_y_times_two(b))
            .then(a.left.cmp(&b.left))
    });

    let mut lines: Vec<TextLine> = Vec::new();
    for result in results {
        let mut best_line_index = None;
        let mut best_center_distance = i32::MAX;

        for (index, line) in lines.iter().enumerate() {
            if !is_result_on_text_line(line, &result) {
                continue;
            }

            let center_distance =
                (line.center_y_times_two() - result_center_y_times_two(&result)).abs();
            if center_distance < best_center_distance {
                best_center_distance = center_distance;
                best_line_index = Some(index);
            }
        }

        if let Some(index) = best_line_index {
            lines[index].push(result);
        } else {
            lines.push(TextLine::new(result));
        }
    }

    lines.sort_by(|a, b| a.top.cmp(&b.top).then(a.left.cmp(&b.left)));

    lines
        .into_iter()
        .flat_map(|mut line| {
            line.items
                .sort_by(|a, b| a.left.cmp(&b.left).then(a.top.cmp(&b.top)));
            merge_text_line(line.items)
        })
        .collect()
}

fn merge_text_line(results: Vec<TextResult>) -> Vec<TextResult> {
    let mut iter = results.into_iter();
    let Some(mut current) = iter.next() else {
        return Vec::new();
    };

    let mut merged = Vec::new();
    for next in iter {
        if should_merge_adjacent_results(&current, &next) {
            merge_result_into(&mut current, next);
        } else {
            merged.push(current);
            current = next;
        }
    }

    merged.push(current);
    merged
}

fn should_merge_adjacent_results(left: &TextResult, right: &TextResult) -> bool {
    if !are_results_on_same_line(left, right) {
        return false;
    }

    let gap = horizontal_gap(left, right);
    if gap == 0 {
        return true;
    }

    let average_height = average_height(left, right);
    let max_gap = cmp::max(
        MERGE_MIN_HORIZONTAL_GAP,
        (average_height * MERGE_MAX_HORIZONTAL_GAP_RATIO).round() as i32,
    );

    gap <= max_gap
}

fn merge_result_into(current: &mut TextResult, next: TextResult) {
    let gap = horizontal_gap(current, &next);
    let average_height = average_height(current, &next);
    let should_insert_space = should_insert_space_between(
        current.text.as_str(),
        next.text.as_str(),
        gap,
        average_height,
    );

    let left = cmp::min(current.left, next.left);
    let top = cmp::min(current.top, next.top);
    let right = cmp::max(result_right(current), result_right(&next));
    let bottom = cmp::max(result_bottom(current), result_bottom(&next));

    if should_insert_space {
        current.text.push(' ');
    }
    current.text.push_str(next.text.as_str());
    current.left = left;
    current.top = top;
    current.width = right.saturating_sub(left) as u32;
    current.height = bottom.saturating_sub(top) as u32;
}

fn is_result_on_text_line(line: &TextLine, result: &TextResult) -> bool {
    is_same_vertical_band(
        line.top,
        line.bottom,
        line.height(),
        line.center_y_times_two(),
        result.top,
        result_bottom(result),
        result_height(result),
        result_center_y_times_two(result),
    )
}

fn are_results_on_same_line(left: &TextResult, right: &TextResult) -> bool {
    is_same_vertical_band(
        left.top,
        result_bottom(left),
        result_height(left),
        result_center_y_times_two(left),
        right.top,
        result_bottom(right),
        result_height(right),
        result_center_y_times_two(right),
    )
}

fn is_same_vertical_band(
    first_top: i32,
    first_bottom: i32,
    first_height: i32,
    first_center_y_times_two: i32,
    second_top: i32,
    second_bottom: i32,
    second_height: i32,
    second_center_y_times_two: i32,
) -> bool {
    let min_height = cmp::min(first_height, second_height) as f32;
    let max_height = cmp::max(first_height, second_height) as f32;
    if max_height / min_height > LINE_MAX_HEIGHT_RATIO {
        return false;
    }

    let vertical_overlap = cmp::max(
        0,
        cmp::min(first_bottom, second_bottom).saturating_sub(cmp::max(first_top, second_top)),
    ) as f32;
    if vertical_overlap / min_height >= LINE_MIN_VERTICAL_OVERLAP {
        return true;
    }

    let center_distance = (first_center_y_times_two - second_center_y_times_two).abs() as f32 / 2.0;
    center_distance <= max_height * LINE_CENTER_TOLERANCE
}

fn should_insert_space_between(
    left_text: &str,
    right_text: &str,
    gap: i32,
    average_height: f32,
) -> bool {
    if gap <= 0 {
        return false;
    }

    let Some(left_char) = left_text.chars().rev().find(|ch| !ch.is_whitespace()) else {
        return false;
    };
    let Some(right_char) = right_text.chars().find(|ch| !ch.is_whitespace()) else {
        return false;
    };

    if is_cjk(left_char)
        || is_cjk(right_char)
        || is_no_space_before(right_char)
        || is_no_space_after(left_char)
    {
        return false;
    }

    if left_char.is_ascii_alphanumeric() && right_char.is_ascii_alphanumeric() {
        return gap as f32 >= average_height * ASCII_WORD_SPACE_GAP_RATIO;
    }

    gap as f32 >= average_height * SYMBOL_SPACE_GAP_RATIO
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch,
        '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{3040}'..='\u{30FF}'
            | '\u{AC00}'..='\u{D7AF}'
            | '\u{F900}'..='\u{FAFF}'
    )
}

fn is_no_space_before(ch: char) -> bool {
    matches!(
        ch,
        ',' | '.'
            | ':'
            | ';'
            | '!'
            | '?'
            | '%'
            | ')'
            | ']'
            | '}'
            | '\u{3001}'
            | '\u{3002}'
            | '\u{FF0C}'
            | '\u{FF1A}'
            | '\u{FF1B}'
            | '\u{FF01}'
            | '\u{FF1F}'
            | '\u{FF09}'
            | '\u{3011}'
            | '\u{300B}'
    )
}

fn is_no_space_after(ch: char) -> bool {
    matches!(ch, '(' | '[' | '{' | '\u{FF08}' | '\u{3010}' | '\u{300A}')
}

fn average_height(left: &TextResult, right: &TextResult) -> f32 {
    (result_height(left) + result_height(right)) as f32 / 2.0
}

fn result_height(result: &TextResult) -> i32 {
    cmp::max(1, result.height.min(i32::MAX as u32) as i32)
}

fn result_right(result: &TextResult) -> i32 {
    result
        .left
        .saturating_add(result.width.min(i32::MAX as u32) as i32)
}

fn result_bottom(result: &TextResult) -> i32 {
    result
        .top
        .saturating_add(result.height.min(i32::MAX as u32) as i32)
}

fn result_center_y_times_two(result: &TextResult) -> i32 {
    result
        .top
        .saturating_mul(2)
        .saturating_add(result_height(result))
}

fn horizontal_gap(left: &TextResult, right: &TextResult) -> i32 {
    right.left.saturating_sub(result_right(left))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_result(left: i32, top: i32, width: u32, height: u32, text: &str) -> TextResult {
        TextResult {
            left,
            top,
            width,
            height,
            text: text.to_string(),
        }
    }

    #[test]
    fn merges_nearby_chinese_results_without_spaces() {
        let merged = merge_text_results(vec![
            text_result(0, 0, 40, 20, "相邻"),
            text_result(48, 1, 40, 19, "文字"),
        ]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].text, "相邻文字");
        assert_eq!(merged[0].left, 0);
        assert_eq!(merged[0].top, 0);
        assert_eq!(merged[0].width, 88);
        assert_eq!(merged[0].height, 20);
    }

    #[test]
    fn inserts_spaces_between_separate_ascii_words() {
        let merged = merge_text_results(vec![
            text_result(0, 0, 42, 20, "Hello"),
            text_result(50, 0, 44, 20, "World"),
        ]);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].text, "Hello World");
    }

    #[test]
    fn keeps_large_horizontal_gaps_separate() {
        let merged = merge_text_results(vec![
            text_result(0, 0, 40, 20, "Name"),
            text_result(90, 0, 48, 20, "Value"),
        ]);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].text, "Name");
        assert_eq!(merged[1].text, "Value");
    }

    #[test]
    fn keeps_different_lines_separate() {
        let merged = merge_text_results(vec![
            text_result(0, 0, 44, 18, "First"),
            text_result(4, 28, 54, 18, "Second"),
        ]);

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].text, "First");
        assert_eq!(merged[1].text, "Second");
    }
}
