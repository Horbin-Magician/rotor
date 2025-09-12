use std::path::Path;
use image::{self, imageops::resize, DynamicImage, GrayImage, RgbaImage};
use rayon::prelude::*;
use rust_paddle_ocr::{Det, Rec};
use serde::{Deserialize, Serialize};

#[allow(dead_code)]
pub fn detect_rect(original_img: &RgbaImage) -> Vec<(u32, u32, u32, u32)> {
    let scale_factor = calculate_optimal_scale_factor(original_img.width(), original_img.height());

    let small_original = resize(
        original_img,
        original_img.width() / scale_factor,
        original_img.height() / scale_factor,
        image::imageops::FilterType::Nearest, // the fastest filter
    );

    let gray = rgb_to_gray_direct(&small_original);
    let edge_image = canny_edge_detection(&gray, 10.0, 30.0);

    // morphological operations
    let morph_size = 4 / scale_factor;
    let processed_image = morphological_close(edge_image, morph_size as u8);

    let min_size = (100 / scale_factor) as u32;
    let contours = find_contours(&processed_image, min_size as usize);

    // Parallel processing of contours
    let res_rects: Vec<(u32, u32, u32, u32)> = contours
        .par_iter()
        .filter_map(|contour| {
            if contour.len() < 4 {
                return None;
            }

            let (rect_left, rect_right, rect_top, rect_bottom) = contour.iter().fold(
                (u32::MAX, 0, u32::MAX, 0),
                |(min_x, max_x, min_y, max_y), &(x, y)| {
                    (min_x.min(x), max_x.max(x), min_y.min(y), max_y.max(y))
                },
            );

            let width = rect_right - rect_left;
            let height = rect_bottom - rect_top;

            if height < min_size || width < min_size { return None; }

            Some((
                rect_left * scale_factor,
                rect_top * scale_factor,
                width * scale_factor,
                height * scale_factor,
            ))
        })
        .collect();

    res_rects
}

fn calculate_optimal_scale_factor(width: u32, height: u32) -> u32 {
    let max_dimension = width.max(height);
    match max_dimension {
        0..=1000 => 1,      // 小图不缩放
        1001..=2000 => 2,   // 中等图像2倍缩放
        2001..=4000 => 3,   // 大图像3倍缩放
        _ => 4,             // 超大图像4倍缩放
    }
}

fn rgb_to_gray_direct(img: &RgbaImage) -> GrayImage {
    let (width, height) = img.dimensions();
    let mut gray = GrayImage::new(width, height);
    
    // 并行处理每一行
    let gray_data: Vec<u8> = (0..height)
        .into_par_iter()
        .flat_map(|y| {
            (0..width).map(|x| {
                let pixel = img.get_pixel(x, y);
                // 使用整数运算代替浮点数
                ((pixel[0] as u32 * 299 + pixel[1] as u32 * 587 + pixel[2] as u32 * 114) / 1000) as u8
            }).collect::<Vec<_>>()
        })
        .collect();
    
    gray.as_mut().copy_from_slice(&gray_data);
    gray
}

fn canny_edge_detection(img: &GrayImage, low_threshold: f32, high_threshold: f32) -> GrayImage {
    let (width, height) = img.dimensions();
    let mut result = GrayImage::new(width, height);
    
    // Pre-calculate gradients in parallel
    let img_data = img.as_raw();
    let width_usize = width as usize;
    
    // Create a buffer for the result
    let result_buffer: Vec<u8> = (0..height)
        .into_par_iter()
        .flat_map(|y| {
            let mut row = vec![0u8; width_usize];
            
            if y > 0 && y < height - 1 {
                for x in 1..width - 1 {
                    let idx = |x: u32, y: u32| (y * width + x) as usize;
                    
                    // Use unsafe for performance (bounds are guaranteed)
                    unsafe {
                        let gx = (*img_data.get_unchecked(idx(x + 1, y - 1)) as i16
                            + 2 * *img_data.get_unchecked(idx(x + 1, y)) as i16
                            + *img_data.get_unchecked(idx(x + 1, y + 1)) as i16)
                            - (*img_data.get_unchecked(idx(x - 1, y - 1)) as i16
                                + 2 * *img_data.get_unchecked(idx(x - 1, y)) as i16
                                + *img_data.get_unchecked(idx(x - 1, y + 1)) as i16);

                        let gy = (*img_data.get_unchecked(idx(x - 1, y + 1)) as i16
                            + 2 * *img_data.get_unchecked(idx(x, y + 1)) as i16
                            + *img_data.get_unchecked(idx(x + 1, y + 1)) as i16)
                            - (*img_data.get_unchecked(idx(x - 1, y - 1)) as i16
                                + 2 * *img_data.get_unchecked(idx(x, y - 1)) as i16
                                + *img_data.get_unchecked(idx(x + 1, y - 1)) as i16);

                        let magnitude = ((gx as i32 * gx as i32 + gy as i32 * gy as i32) as f32).sqrt();

                        row[x as usize] = if magnitude > high_threshold {
                            255
                        } else if magnitude > low_threshold {
                            128
                        } else {
                            0
                        };
                    }
                }
            }
            
            row
        })
        .collect();
    
    // Copy buffer to result image
    for (i, &pixel) in result_buffer.iter().enumerate() {
        let x = (i % width_usize) as u32;
        let y = (i / width_usize) as u32;
        result.put_pixel(x, y, image::Luma([pixel]));
    }
    
    result
}

fn morphological_close(mut img: GrayImage, size: u8) -> GrayImage {
    if size == 0 { return img; }
    
    let (width, height) = img.dimensions();
    
    // Combined dilate-erode operation with single buffer allocation
    let mut buffer = vec![0u8; (width * height) as usize];
    
    // Dilate
    for _ in 0..size {
        let img_data = img.as_raw();
        buffer.par_chunks_mut(width as usize)
            .enumerate()
            .for_each(|(y, row)| {
                if y > 0 && y < height as usize - 1 {
                    for x in 1..width as usize - 1 {
                        let mut max_val = 0u8;
                        for dy in -1i32..=1 {
                            for dx in -1i32..=1 {
                                let nx = (x as i32 + dx) as usize;
                                let ny = (y as i32 + dy) as usize;
                                let idx = ny * width as usize + nx;
                                max_val = max_val.max(img_data[idx]);
                            }
                        }
                        row[x] = max_val;
                    }
                }
            });
        
        // Copy buffer back to image
        img.as_mut().copy_from_slice(&buffer);
    }
    
    // Erode
    for _ in 0..size {
        let img_data = img.as_raw();
        buffer.par_chunks_mut(width as usize)
            .enumerate()
            .for_each(|(y, row)| {
                if y > 0 && y < height as usize - 1 {
                    for x in 1..width as usize - 1 {
                        let mut min_val = 255u8;
                        for dy in -1i32..=1 {
                            for dx in -1i32..=1 {
                                let nx = (x as i32 + dx) as usize;
                                let ny = (y as i32 + dy) as usize;
                                let idx = ny * width as usize + nx;
                                min_val = min_val.min(img_data[idx]);
                            }
                        }
                        row[x] = min_val;
                    }
                }
            });
        
        // Copy buffer back to image
        img.as_mut().copy_from_slice(&buffer);
    }
    
    img
}

fn find_contours(img: &GrayImage, min_size: usize) -> Vec<Vec<(u32, u32)>> {
    let (width, height) = img.dimensions();
    let mut visited = vec![false; (width * height) as usize];
    let mut contours = Vec::new();
    
    let img_data = img.as_raw();
    
    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if img_data[idx] > 128 && !visited[idx] {
                let contour = flood_fill(img_data, &mut visited, x, y, width, height);
                if contour.len() > min_size {
                    contours.push(contour);
                }
            }
        }
    }
    
    contours
}

fn flood_fill(
    img_data: &[u8],
    visited: &mut [bool],
    start_x: u32,
    start_y: u32,
    width: u32,
    height: u32,
) -> Vec<(u32, u32)> {
    let mut contour = Vec::with_capacity(1000); // Pre-allocate for typical contour size
    let mut stack = Vec::with_capacity(1000); // Pre-allocate for typical contour size
    stack.push((start_x, start_y));
    
    while let Some((x, y)) = stack.pop() {
        let idx = (y * width + x) as usize;
        
        if x >= width || y >= height || visited[idx] || img_data[idx] <= 128 {
            continue;
        }
        
        visited[idx] = true;
        contour.push((x, y));
        
        // Use direct indexing instead of conditional checks
        if x > 0 {
            stack.push((x - 1, y));
        }
        if x + 1 < width {
            stack.push((x + 1, y));
        }
        if y > 0 {
            stack.push((x, y - 1));
        }
        if y + 1 < height {
            stack.push((x, y + 1));
        }
    }
    
    contour
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

    let mut det = Det::from_file(det_model_path).unwrap();
    let mut rec = Rec::from_file(rec_model_path, keys_path).unwrap();

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
