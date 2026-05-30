use image::{self, DynamicImage, GrayImage, RgbaImage};
use rayon::prelude::*;
use rust_paddle_ocr::{Det, Rec};
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

pub fn img2text(
    model_path: &Path,
    img: &DynamicImage,
) -> Result<Vec<TextResult>, Box<dyn std::error::Error>> {
    let det_model_path = model_path.join("PP-OCRv5_mobile_det_fp16.mnn");
    let rec_model_path = model_path.join("PP-OCRv5_mobile_rec_fp16.mnn");
    let keys_path = model_path.join("ppocr_keys_v5.txt");

    let mut det = Det::from_file(det_model_path)?.with_rect_border_size(12);

    let mut rec = Rec::from_file(rec_model_path, keys_path)?
        .with_min_score(0.8)
        .with_punct_min_score(0.1);

    let rects = det.find_text_rect(img)?;

    let mut text_results = Vec::new();
    for rect in rects.iter() {
        let text_img = img.crop_imm(
            rect.left() as u32,
            rect.top() as u32,
            rect.width(),
            rect.height(),
        );
        let text = rec.predict_str(&text_img)?;
        text_results.push(TextResult {
            left: rect.left(),
            top: rect.top(),
            width: rect.width(),
            height: rect.height(),
            text,
        });
    }
    Ok(text_results)
}
