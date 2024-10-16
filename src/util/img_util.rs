use image::{self, imageops::resize, DynamicImage, RgbaImage};
use imageproc::{contours, edges};

// use image::Rgb;
// use imageproc::rect::Rect;


// return all rect in the image, (x, y, width, height)
pub fn detect_rect(original_img: &RgbaImage) -> Vec<(u32, u32, u32, u32)> {
    let scale_factor: u8 = 2;
    let small_original = resize(
        original_img,
        original_img.width() / 2,
        original_img.height() / 2,
        image::imageops::FilterType::Nearest,
    );

    let gray = DynamicImage::ImageRgba8(small_original).into_luma8(); // convert to gray
    // let gray_filter = imageproc::filter::gaussian_blur_f32(&gray, 2.0 / scale_factor as f32); // gaussian blur
    
    let mut edge_image = edges::canny(&gray, 10.0, 30.0);
    imageproc::morphology::dilate_mut(&mut edge_image, imageproc::distance_transform::Norm::L1, 4/scale_factor);
    imageproc::morphology::erode_mut(&mut edge_image, imageproc::distance_transform::Norm::L1, 4/scale_factor);

    let contours = contours::find_contours::<u32>(&edge_image); // find contours

    let mut res_rects = vec![];
    for contour in contours {
        if contour.border_type == imageproc::contours::BorderType::Hole { continue; }
        let points = contour.points;
        if points.len() < 4 { continue; }

        let mut rect_top = points[0].y;
        let mut rect_bottom = points[0].y;
        let mut rect_left = points[0].x;
        let mut rect_right = points[0].x;
        for point in points {
            if point.y < rect_top { rect_top = point.y; }
            if point.y > rect_bottom { rect_bottom = point.y; }
            if point.x < rect_left { rect_left = point.x; }
            if point.x > rect_right { rect_right = point.x; }
        }

        let width = rect_right - rect_left;
        let height = rect_bottom - rect_top;
        if height < (100/scale_factor) as u32 || width < (100/scale_factor) as u32 { continue; } // filter small rect

        res_rects.push((
            rect_left * scale_factor as u32,
            rect_top * scale_factor as u32,
            width * scale_factor as u32,
            height * scale_factor as u32
        ));
    }
    // sort res_rects from small area to large area
    res_rects.sort_by(|a, b| (a.2 * a.3).cmp(&(b.2 * b.3)));
    
    // let mut plot_img = DynamicImage::ImageLuma8(edge_image).to_rgb8();
    // for rect in &res_rects {
    //     let (x, y, width, height) = rect;
    //     imageproc::drawing::draw_hollow_rect_mut(
    //         &mut plot_img,
    //         Rect::at((*x / scale_factor as u32) as i32, (*y / scale_factor as u32) as i32)
    //             .of_size(*width / scale_factor as u32, *height / scale_factor as u32),
    //         Rgb([255, 0, 0]));
    // }
    // plot_img.save("./test.png").unwrap(); // just for debug

    return res_rects;
}