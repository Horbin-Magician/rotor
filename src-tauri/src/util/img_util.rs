use image::{self, imageops::resize, DynamicImage, RgbaImage};
use imageproc::{contours, edges};
use rust_paddle_ocr::{Det, Rec};

use crate::util::file_util;

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
    let mut edge_image = edges::canny(&gray, 10.0, 30.0);
    
    // Combine morphological operations when possible
    let morph_size = 4 / scale_factor;
    imageproc::morphology::dilate_mut(
        &mut edge_image,
        imageproc::distance_transform::Norm::L1,
        morph_size,
    );
    imageproc::morphology::erode_mut(
        &mut edge_image,
        imageproc::distance_transform::Norm::L1,
        morph_size,
    );

    let contours = contours::find_contours::<u32>(&edge_image); // find contours

    let mut res_rects: Vec<(u32, u32, u32, u32)> = Vec::with_capacity(contours.len() / 4);
    
    // Filter thresholds
    let min_size = (100 / scale_factor) as u32;
    let scale_u32 = scale_factor as u32;
    
    for contour in contours {
        // Early filtering
        if contour.border_type == imageproc::contours::BorderType::Hole { continue; }

        let points = &contour.points;
        if points.len() < 4 { continue; }

        // Use iterator for finding bounds - more efficient
        let (rect_left, rect_right, rect_top, rect_bottom) = points.iter()
            .fold((u32::MAX, 0, u32::MAX, 0), |(min_x, max_x, min_y, max_y), point| {
                (min_x.min(point.x),max_x.max(point.x), min_y.min(point.y), max_y.max(point.y))
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

#[allow(dead_code)]
pub fn img2text(img: DynamicImage) {
    let userdata_path = file_util::get_userdata_path().unwrap();
    let model_path = userdata_path.join("models");
    let det_model_path = model_path.join("PP-OCRv5_mobile_det_fp16.mnn");
    let rec_model_path = model_path.join("PP-OCRv5_mobile_rec_fp16.mnn");
    let keys_path = model_path.join("ppocr_keys_v5.txt");

    let mut det = Det::from_file(det_model_path).unwrap();
    let mut rec = Rec::from_file(rec_model_path, keys_path).unwrap();

    // let det = det
    //     .with_rect_border_size(12)  // PP-OCRv5 推荐参数
    //     .with_merge_boxes(false)    // PP-OCRv5 推荐参数
    //     .with_merge_threshold(1);   // PP-OCRv5 推荐参数

    // // 自定义识别参数（可选）
    // let rec = rec
    //     .with_min_score(0.6)
    //     .with_punct_min_score(0.1);

    let text_images = det.find_text_img(&img).unwrap();

    // 识别每个检测区域中的文本
    for text_img in text_images {
        let text = rec.predict_str(&text_img).unwrap();
        println!("识别的文本: {}", text);
    }
}
