use image::{self, imageops::resize, DynamicImage, RgbaImage};
use imageproc::{contours, edges};
use slint::{Rgb8Pixel, SharedPixelBuffer};


// return all rect in the image, (x, y, width, height)
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
    imageproc::morphology::dilate_mut(&mut edge_image, imageproc::distance_transform::Norm::L1, 4/scale_factor);
    imageproc::morphology::erode_mut(&mut edge_image, imageproc::distance_transform::Norm::L1, 4/scale_factor);

    let contours = contours::find_contours::<u32>(&edge_image); // find contours

    let mut res_rects: Vec<(u32, u32, u32, u32)> = contours.into_iter().filter_map(|contour| {
        if contour.border_type == imageproc::contours::BorderType::Hole { return None; }

        let points = contour.points;
        if points.len() < 4 { return None; }

        let (mut rect_top, mut rect_bottom) = (points[0].y, points[0].y);
        let (mut rect_left, mut rect_right) = (points[0].x, points[0].x);

        for point in points {
            if point.y < rect_top { rect_top = point.y; }
            if point.y > rect_bottom { rect_bottom = point.y; }
            if point.x < rect_left { rect_left = point.x; }
            if point.x > rect_right { rect_right = point.x; }
        }

        let width = rect_right - rect_left;
        let height = rect_bottom - rect_top;
        if height < (100 / scale_factor) as u32 || width < (100 / scale_factor) as u32 { return None; } // filter small rect

        Some((
            rect_left * scale_factor as u32,
            rect_top * scale_factor as u32,
            width * scale_factor as u32,
            height * scale_factor as u32,
        ))
    }).collect();

    // sort res_rects from small area to large area
    res_rects.sort_by(|a, b| (a.2 * a.3).cmp(&(b.2 * b.3)));
    
    // just for debug
    // let mut plot_img = DynamicImage::ImageLuma8(edge_image).to_rgb8();
    // for rect in &res_rects {
    //     let (x, y, width, height) = rect;
    //     imageproc::drawing::draw_hollow_rect_mut(
    //         &mut plot_img,
    //         imageproc::rect::Rect::at((*x / scale_factor as u32) as i32, (*y / scale_factor as u32) as i32)
    //             .of_size(*width / scale_factor as u32, *height / scale_factor as u32),
    //         image::Rgb([255, 0, 0]));
    // }
    // plot_img.save("./test.png").unwrap();

    return res_rects;
}

pub fn shared_pixel_buffer_to_dynamic_image(buffer: &SharedPixelBuffer<Rgb8Pixel>) -> image::DynamicImage {
    image::DynamicImage::ImageRgb8(
        image::RgbImage::from_vec(
            buffer.width(), buffer.height(), buffer.as_bytes().to_vec()
        ).unwrap_or_default()
    )
}