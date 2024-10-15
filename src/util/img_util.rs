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
    
    let mut edge_image = edges::canny(&gray, 10.0, 30.0);
    imageproc::morphology::dilate_mut(&mut edge_image, imageproc::distance_transform::Norm::L1, 2/scale_factor);
    imageproc::morphology::erode_mut(&mut edge_image, imageproc::distance_transform::Norm::L1, 2/scale_factor);

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

        // let mut right_num = 0;
        // for x in rect_left..rect_right {
        //     let Luma([p1]) = edge_image.get_pixel(x, rect_top);
        //     let Luma([p2]) = edge_image.get_pixel(x, rect_bottom);
        //     if *p1 > 0 {right_num += 1};
        //     if *p2 > 0 {right_num += 1};
        // }
        // for y in rect_top..rect_bottom {
        //     let Luma([p1]) = edge_image.get_pixel(rect_left, y);
        //     let Luma([p2]) = edge_image.get_pixel(rect_right, y);
        //     if *p1 > 0 {right_num += 1};
        //     if *p2 > 0 {right_num += 1};
        // }
        // if (right_num as f32 / ( (width + height) * 2 ) as f32) < 0.3 { continue; }


        res_rects.push((
            rect_left * scale_factor as u32,
            rect_top * scale_factor as u32,
            width * scale_factor as u32,
            height * scale_factor as u32
        ));
    }
    // sort res_rects from small area to large area
    res_rects.sort_by(|a, b| (a.2 * a.3).cmp(&(b.2 * b.3)));
    
    // let mut edge_image = DynamicImage::ImageLuma8(edge_image).to_rgb8();
    // for rect in &res_rects {
    //     let (x, y, width, height) = rect;
    //     imageproc::drawing::draw_hollow_rect_mut(
    //         &mut edge_image,
    //         Rect::at((*x / scale_factor as u32) as i32, (*y / scale_factor as u32) as i32)
    //             .of_size(*width / scale_factor as u32, *height / scale_factor as u32),
    //         Rgb([255, 0, 0]));
    // }
    // edge_image.save("./test.png").unwrap(); // just for debug

    return res_rects;
}

// let v_line_filter = [-1, 0, 1, -2, 0, 2, -1, 0, 1];
// let v_line_filter2 = [1, 0, -1, 2, 0, -2, 1, 0, -1];
// let v_lines = imageproc::filter::filter3x3::<Luma<u8>, i32, u8>(&gray_filter, &v_line_filter);
// let v_lines2 = imageproc::filter::filter3x3::<Luma<u8>, i32, u8>(&gray_filter, &v_line_filter2);
// let h_line_filter = [-1, -2, -1, 0, 0, 0, 1, 2, 1];
// let h_line_filter2 = [1, 2, 1, 0, 0, 0, -1, -2, -1];
// let h_lines = imageproc::filter::filter3x3::<Luma<u8>, i32, u8>(&gray_filter, &h_line_filter);
// let h_lines2 = imageproc::filter::filter3x3::<Luma<u8>, i32, u8>(&gray_filter, &h_line_filter2);

// let (width, height) = gray.dimensions();
// let mut lines_img = GrayImage::new(width, height);

// // let exten_threshold: u32 = 10;
// let threshold: i16 = 1;
// for y in 1..(height-1) {
//     for x in 1..(width-1) {
//         let Luma([center]) = gray_filter.get_pixel(x, y);
//         let Luma([top]) = gray_filter.get_pixel(x, y-1);
//         let Luma([bottom]) = gray_filter.get_pixel(x, y+1);
//         let Luma([left]) = gray_filter.get_pixel(x-1, y);
//         let Luma([right]) = gray_filter.get_pixel(x+1, y);

//         // (*left as i16 - *right as i16).abs() > threshold
//         // (*top as i16 - *bottom as i16).abs() > threshold
//         if top == center && center == bottom && (*left as i16 - *right as i16).abs() > threshold{
//             lines_img.put_pixel(x, y, Luma([255]));
//         } else if left == center && center == right && (*top as i16 - *bottom as i16).abs() > threshold {
//             lines_img.put_pixel(x, y, Luma([255]));
//         } else {
//             lines_img.put_pixel(x, y, Luma([0]));
//         }
//     }
// }