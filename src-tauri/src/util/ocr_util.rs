use image::DynamicImage;
use rust_paddle_ocr::{Det, Rec};

use crate::util::file_util;

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
