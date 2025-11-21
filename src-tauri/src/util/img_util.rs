use std::path::Path;
use image::{self, DynamicImage, GrayImage, RgbaImage};
use rayon::prelude::*;
use rust_paddle_ocr::{Det, Rec};
use serde::{Deserialize, Serialize};
use fast_image_resize::{Resizer, ResizeOptions};
use fast_image_resize::images::Image;
use std::cmp;

#[allow(dead_code)]
pub fn detect_rect(original_img: RgbaImage) -> Vec<(u32, u32, u32, u32)> {
    let scale_factor = calculate_optimal_scale_factor(original_img.width(), original_img.height());
    let small_original = fast_resize(&original_img, scale_factor);

    let gray = image_to_gray(&small_original);
    let edge_image = canny_edge_detection(&gray, 10.0, 30.0);

    let morph_size = cmp::max(1, 4 / scale_factor) as u8;
    let processed_image = morphological_close(edge_image, morph_size);

    let min_size = (100 / scale_factor) as u32;
    let rects = find_bounding_boxes(&processed_image, min_size);

    // 6. Rescale back
    rects.into_iter()
        .map(|(x, y, w, h)| (
            x * scale_factor,
            y * scale_factor,
            w * scale_factor,
            h * scale_factor
        ))
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

fn fast_resize(img: &RgbaImage, scale_factor: u32) -> Image<'static> {
    let dst_width = img.width() / scale_factor;
    let dst_height = img.height() / scale_factor;

    let mut src_vec = img.as_raw().clone();
    let src_image = Image::from_slice_u8(
        img.width(),
        img.height(),
        src_vec.as_mut_slice(),
        fast_image_resize::PixelType::U8x4,
    ).unwrap();

    let mut dst_image = Image::new(dst_width, dst_height, fast_image_resize::PixelType::U8x4);

    let mut resizer = Resizer::new();
    let resize_options = ResizeOptions::new()
        .use_alpha(false)
        .resize_alg(fast_image_resize::ResizeAlg::Nearest);
    
    resizer.resize(&src_image, &mut dst_image, Some(&resize_options)).unwrap();

    dst_image
}

fn image_to_gray(img: &Image) -> GrayImage {
    let width = img.width();
    let height = img.height();
    let img_data = img.buffer();
    
    let mut gray_data = vec![0u8; (width * height) as usize];
    
    gray_data.par_chunks_mut(width as usize)
        .enumerate()
        .for_each(|(y, row)| {
            let row_offset = y * width as usize * 4;
            for x in 0..width as usize {
                let p = unsafe {
                    img_data.get_unchecked(row_offset + x * 4..row_offset + x * 4 + 3)
                };
                // Y = 0.299R + 0.587G + 0.114B
                row[x] = ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8;
            }
        });
    
    GrayImage::from_raw(width, height, gray_data).unwrap()
}

fn canny_edge_detection(img: &GrayImage, low_threshold: f32, high_threshold: f32) -> GrayImage {
    let (width, height) = img.dimensions();
    let mut result = GrayImage::new(width, height);
    let img_data = img.as_raw();
    let res_data = result.as_mut();

    let high_sq = (high_threshold * high_threshold) as i32;
    let low_sq = (low_threshold * low_threshold) as i32;
    let width_usize = width as usize;
    res_data.par_chunks_mut(width_usize)
        .enumerate()
        .for_each(|(y, row)| {
            if y > 0 && y < (height as usize - 1) {
                for x in 1..(width_usize - 1) {
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

                        row[x] = if mag_sq > high_sq {
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
    if size == 0 { return img; }
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
                            let neighbor = src[(y as isize + dy) as usize * w + (x as isize + dx) as usize];
                            if is_dilate {
                                if neighbor > val { val = neighbor; }
                            } else {
                                if neighbor < val { val = neighbor; }
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

    GrayImage::from_raw(width, height, current_buf).unwrap()
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
                if let Some(rect) = flood_fill_bbox(img_data, &mut visited, x, y, width, height, min_size) {
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
    min_size: u32
) -> Option<(u32, u32, u32, u32)> {
    let mut stack = Vec::with_capacity(512);
    stack.push((start_x, start_y));
    
    let mut min_x = start_x;
    let mut max_x = start_x;
    let mut min_y = start_y;
    let mut max_y = start_y;
    let mut pixel_count = 0;

    let w_usize = width as usize;

    while let Some((x, y)) = stack.pop() {
        let idx = (y * width + x) as usize;
        
        if visited[idx] { continue; }
        visited[idx] = true;
        
        // 更新 Bbox
        if x < min_x { min_x = x; }
        if x > max_x { max_x = x; }
        if y < min_y { min_y = y; }
        if y > max_y { max_y = y; }
        pixel_count += 1;

        // 4-connectivity neighbor check
        // Right
        if x + 1 < width {
            let n_idx = idx + 1;
            if !visited[n_idx] && img_data[n_idx] > 64 {
                stack.push((x + 1, y));
            }
        }
        // Left
        if x > 0 {
            let n_idx = idx - 1;
            if !visited[n_idx] && img_data[n_idx] > 64 {
                stack.push((x - 1, y));
            }
        }
        // Down
        if y + 1 < height {
            let n_idx = idx + w_usize;
            if !visited[n_idx] && img_data[n_idx] > 64 {
                stack.push((x, y + 1));
            }
        }
        // Up
        if y > 0 {
            let n_idx = idx - w_usize;
            if !visited[n_idx] && img_data[n_idx] > 64 {
                stack.push((x, y - 1));
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

#[allow(dead_code)]
pub fn img2text(model_path: &Path, img: &DynamicImage) -> Vec<TextResult> {
    let det_model_path = model_path.join("PP-OCRv5_mobile_det_fp16.mnn");
    let rec_model_path = model_path.join("PP-OCRv5_mobile_rec_fp16.mnn");
    let keys_path = model_path.join("ppocr_keys_v5.txt");

    let mut det = Det::from_file(det_model_path).unwrap()
        .with_rect_border_size(12);

    let mut rec = Rec::from_file(rec_model_path, keys_path).unwrap()
        .with_min_score(0.8)
        .with_punct_min_score(0.1);

    let rects = det.find_text_rect(&img).unwrap();

    let mut text_results = Vec::new();
    for rect in rects.iter() {
        let text_img = img.crop_imm(rect.left() as u32, rect.top() as u32, rect.width(), rect.height());
        let text = rec.predict_str(&text_img).unwrap();
        text_results.push(TextResult {
            left: rect.left(),
            top: rect.top(),
            width: rect.width(),
            height: rect.height(),
            text,
        });
    }
    text_results
}
