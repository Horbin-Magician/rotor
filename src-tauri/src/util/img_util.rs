use image::{self, imageops::resize, DynamicImage, RgbaImage, GrayImage};
// use rust_paddle_ocr::{Det, Rec};

// use crate::util::file_util;

// return all rect in the image, (x, y, width, height)
#[allow(dead_code)]
pub fn detect_rect(original_img: &RgbaImage) -> Vec<(u32, u32, u32, u32)> {
    let scale_factor: u8 = 2;
    let small_original = resize(
        original_img,
        original_img.width() / scale_factor as u32,
        original_img.height() / scale_factor as u32,
        image::imageops::FilterType::Nearest,
    );

    let gray = DynamicImage::ImageRgba8(small_original).into_luma8(); // convert to gray
    let mut edge_image = canny_edge_detection(&gray, 10.0, 30.0);
    
    // Combine morphological operations when possible
    let morph_size = 4 / scale_factor;
    dilate(&mut edge_image, morph_size);
    erode(&mut edge_image, morph_size);

    let contours = find_contours(&edge_image); // find contours

    let mut res_rects: Vec<(u32, u32, u32, u32)> = Vec::with_capacity(contours.len() / 4);
    
    // Filter thresholds
    let min_size = (100 / scale_factor) as u32;
    let scale_u32 = scale_factor as u32;
    
    for contour in contours {
        if contour.len() < 4 { continue; }

        // Use iterator for finding bounds - more efficient
        let (rect_left, rect_right, rect_top, rect_bottom) = contour.iter()
            .fold((u32::MAX, 0, u32::MAX, 0), |(min_x, max_x, min_y, max_y), &(x, y)| {
                (min_x.min(x), max_x.max(x), min_y.min(y), max_y.max(y))
            });

        let width = rect_right - rect_left;
        let height = rect_bottom - rect_top;
        
        // Early size filtering
        if height < min_size || width < min_size { continue; }

        res_rects.push((
            rect_left * scale_u32,
            rect_top * scale_u32,
            width * scale_u32,
            height * scale_u32,
        ));
    }

    res_rects
}

// Simple Canny edge detection implementation
fn canny_edge_detection(img: &GrayImage, low_threshold: f32, high_threshold: f32) -> GrayImage {
    let (width, height) = img.dimensions();
    let mut result = GrayImage::new(width, height);
    
    // Simple Sobel edge detection as approximation
    for y in 1..height-1 {
        for x in 1..width-1 {
            let gx = (img.get_pixel(x+1, y-1)[0] as i16 + 2*img.get_pixel(x+1, y)[0] as i16 + img.get_pixel(x+1, y+1)[0] as i16)
                   - (img.get_pixel(x-1, y-1)[0] as i16 + 2*img.get_pixel(x-1, y)[0] as i16 + img.get_pixel(x-1, y+1)[0] as i16);
            
            let gy = (img.get_pixel(x-1, y+1)[0] as i16 + 2*img.get_pixel(x, y+1)[0] as i16 + img.get_pixel(x+1, y+1)[0] as i16)
                   - (img.get_pixel(x-1, y-1)[0] as i16 + 2*img.get_pixel(x, y-1)[0] as i16 + img.get_pixel(x+1, y-1)[0] as i16);
            
            let magnitude = ((gx as i32 * gx as i32 + gy as i32 * gy as i32) as f32).sqrt();
            
            let pixel_val = if magnitude > high_threshold {
                255
            } else if magnitude > low_threshold {
                128
            } else {
                0
            };
            
            result.put_pixel(x, y, image::Luma([pixel_val]));
        }
    }
    
    result
}

// Simple morphological dilation
fn dilate(img: &mut GrayImage, size: u8) {
    let (width, height) = img.dimensions();
    let mut temp = img.clone();
    
    for _ in 0..size {
        for y in 1..height-1 {
            for x in 1..width-1 {
                let mut max_val = 0u8;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let nx = (x as i32 + dx) as u32;
                        let ny = (y as i32 + dy) as u32;
                        if nx < width && ny < height {
                            max_val = max_val.max(temp.get_pixel(nx, ny)[0]);
                        }
                    }
                }
                img.put_pixel(x, y, image::Luma([max_val]));
            }
        }
        temp = img.clone();
    }
}

// Simple morphological erosion
fn erode(img: &mut GrayImage, size: u8) {
    let (width, height) = img.dimensions();
    let mut temp = img.clone();
    
    for _ in 0..size {
        for y in 1..height-1 {
            for x in 1..width-1 {
                let mut min_val = 255u8;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let nx = (x as i32 + dx) as u32;
                        let ny = (y as i32 + dy) as u32;
                        if nx < width && ny < height {
                            min_val = min_val.min(temp.get_pixel(nx, ny)[0]);
                        }
                    }
                }
                img.put_pixel(x, y, image::Luma([min_val]));
            }
        }
        temp = img.clone();
    }
}

// Simple contour finding using connected components
fn find_contours(img: &GrayImage) -> Vec<Vec<(u32, u32)>> {
    let (width, height) = img.dimensions();
    let mut visited = vec![vec![false; width as usize]; height as usize];
    let mut contours = Vec::new();
    
    for y in 0..height {
        for x in 0..width {
            if img.get_pixel(x, y)[0] > 128 && !visited[y as usize][x as usize] {
                let mut contour = Vec::new();
                flood_fill(img, &mut visited, x, y, &mut contour);
                if contour.len() > 10 { // Filter small contours
                    contours.push(contour);
                }
            }
        }
    }
    
    contours
}

// Flood fill to find connected components
fn flood_fill(img: &GrayImage, visited: &mut [Vec<bool>], start_x: u32, start_y: u32, contour: &mut Vec<(u32, u32)>) {
    let (width, height) = img.dimensions();
    let mut stack = vec![(start_x, start_y)];
    
    while let Some((x, y)) = stack.pop() {
        if x >= width || y >= height || visited[y as usize][x as usize] || img.get_pixel(x, y)[0] <= 128 {
            continue;
        }
        
        visited[y as usize][x as usize] = true;
        contour.push((x, y));
        
        // Add neighbors to stack
        if x > 0 { stack.push((x - 1, y)); }
        if x < width - 1 { stack.push((x + 1, y)); }
        if y > 0 { stack.push((x, y - 1)); }
        if y < height - 1 { stack.push((x, y + 1)); }
    }
}

// #[allow(dead_code)]
// pub fn img2text(img: DynamicImage) {
//     let userdata_path = file_util::get_userdata_path().unwrap();
//     let model_path = userdata_path.join("models");
//     let det_model_path = model_path.join("PP-OCRv5_mobile_det_fp16.mnn");
//     let rec_model_path = model_path.join("PP-OCRv5_mobile_rec_fp16.mnn");
//     let keys_path = model_path.join("ppocr_keys_v5.txt");

//     let mut det = Det::from_file(det_model_path).unwrap();
//     let mut rec = Rec::from_file(rec_model_path, keys_path).unwrap();

//     // let det = det
//     //     .with_rect_border_size(12)  // PP-OCRv5 推荐参数
//     //     .with_merge_boxes(false)    // PP-OCRv5 推荐参数
//     //     .with_merge_threshold(1);   // PP-OCRv5 推荐参数

//     // // 自定义识别参数（可选）
//     // let rec = rec
//     //     .with_min_score(0.6)
//     //     .with_punct_min_score(0.1);

//     let text_images = det.find_text_img(&img).unwrap();

//     // 识别每个检测区域中的文本
//     for text_img in text_images {
//         let text = rec.predict_str(&text_img).unwrap();
//         println!("识别的文本: {}", text);
//     }
// }
