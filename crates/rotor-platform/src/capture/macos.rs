use core_graphics::{
    display::{CGDisplay, CGPoint, CGRect, CGSize},
    image::CGImage,
    window::{create_image, kCGNullWindowID, kCGWindowImageDefault, kCGWindowListOptionAll},
};

/// Capture the display with Quartz, retaining native BGRA bytes for rendering. All native resources are local
/// to this call; only the owned, tightly packed pixel buffer leaves the worker.
pub fn capture_display_bgra(display_id: u32, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let image = create_image(
        CGDisplay::new(display_id).bounds(),
        kCGWindowListOptionAll,
        kCGNullWindowID,
        kCGWindowImageDefault,
    )
    .ok_or("Could not capture display; check Screen Recording permission")?;
    image_bgra(&image, width, height)
}

/// Capture a physical pixel rectangle on `display_id` in top-down BGRA format.
/// Captures only the minimal Quartz point rect covering the requested pixels,
/// falling back to the full display if fractional scaling shifts image dimensions.
#[allow(clippy::too_many_arguments)]
pub fn capture_region_bgra(
    display_id: u32,
    display_width: u32,
    display_height: u32,
    scale_factor: f32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    if width == 0
        || height == 0
        || x.checked_add(width)
            .is_none_or(|right| right > display_width)
        || y.checked_add(height)
            .is_none_or(|bottom| bottom > display_height)
    {
        return Err("Capture region is outside the display".into());
    }
    let scale = scale_factor as f64;
    if scale.is_finite() && scale > 0. {
        let display_bounds = CGDisplay::new(display_id).bounds();
        let left_pt = (x as f64 / scale).floor();
        let top_pt = (y as f64 / scale).floor();
        let right_pt = ((x + width) as f64 / scale)
            .ceil()
            .min(display_bounds.size.width);
        let bottom_pt = ((y + height) as f64 / scale)
            .ceil()
            .min(display_bounds.size.height);
        let rect_w_pt = right_pt - left_pt;
        let rect_h_pt = bottom_pt - top_pt;
        if rect_w_pt > 0. && rect_h_pt > 0. {
            let capture_rect = CGRect::new(
                &CGPoint::new(
                    display_bounds.origin.x + left_pt,
                    display_bounds.origin.y + top_pt,
                ),
                &CGSize::new(rect_w_pt, rect_h_pt),
            );
            if let Some(image) = create_image(
                capture_rect,
                kCGWindowListOptionAll,
                kCGNullWindowID,
                kCGWindowImageDefault,
            ) {
                let offset_x = x.saturating_sub((left_pt * scale).round() as u32);
                let offset_y = y.saturating_sub((top_pt * scale).round() as u32);
                if image.bits_per_component() == 8
                    && image.bits_per_pixel() == 32
                    && offset_x as usize + width as usize <= image.width()
                    && offset_y as usize + height as usize <= image.height()
                {
                    let data = image.data();
                    return copy_bgra_subrect(
                        data.bytes(),
                        image.bytes_per_row(),
                        image.height(),
                        offset_x,
                        offset_y,
                        width,
                        height,
                    );
                }
            }
        }
    }
    let full = capture_display_bgra(display_id, display_width, display_height)?;
    copy_bgra_subrect(
        &full,
        display_width as usize * 4,
        display_height as usize,
        x,
        y,
        width,
        height,
    )
}

fn image_bgra(image: &CGImage, width: u32, height: u32) -> Result<Vec<u8>, String> {
    // Width and height are physical capture pixels, not Quartz desktop points.
    if (image.width(), image.height()) != (width as usize, height as usize) {
        return Err("Capture dimensions changed".into());
    }
    if image.bits_per_component() != 8 || image.bits_per_pixel() != 32 {
        return Err("Unsupported capture pixel format".into());
    }
    let data = image.data();
    // Borrow CFData directly, avoiding an intermediate full-image Vec.
    // Preserve all four native bytes, including alpha, exactly as the former
    // BGRA -> RGBA -> BGRA path did. Do not unpremultiply or recolor them.
    copy_bgra_rows(data.bytes(), width, height, image.bytes_per_row())
}

fn copy_bgra_rows(data: &[u8], width: u32, height: u32, stride: usize) -> Result<Vec<u8>, String> {
    let row_bytes = (width as usize)
        .checked_mul(4)
        .filter(|length| *length > 0)
        .ok_or("Invalid capture dimensions")?;
    let length = row_bytes
        .checked_mul(height as usize)
        .filter(|length| *length > 0 && *length <= isize::MAX as usize)
        .ok_or("Invalid capture dimensions")?;
    let source_length = stride
        .checked_mul(height as usize)
        .ok_or("Invalid capture row stride")?;
    if stride < row_bytes || source_length > data.len() {
        return Err("Invalid capture row data".into());
    }

    // Quartz may pad each scanline. Copy just the pixels, in their original
    // top-down order, into the buffer that becomes GPUI's render storage.
    let mut bytes = Vec::with_capacity(length);
    for row in data[..source_length].chunks_exact(stride) {
        bytes.extend_from_slice(&row[..row_bytes]);
    }
    Ok(bytes)
}

fn copy_bgra_subrect(
    data: &[u8],
    stride: usize,
    source_height: usize,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    let x_bytes = (x as usize)
        .checked_mul(4)
        .ok_or("Invalid capture region offset")?;
    let row_bytes = (width as usize)
        .checked_mul(4)
        .filter(|length| *length > 0)
        .ok_or("Invalid capture region width")?;
    let end_x_bytes = x_bytes
        .checked_add(row_bytes)
        .filter(|end| *end <= stride)
        .ok_or("Capture region exceeds source row stride")?;
    let end_y = (y as usize)
        .checked_add(height as usize)
        .filter(|end| *end <= source_height)
        .ok_or("Capture region exceeds source height")?;
    let source_length = stride
        .checked_mul(end_y)
        .filter(|len| *len <= data.len())
        .ok_or("Invalid capture row data")?;
    let length = row_bytes
        .checked_mul(height as usize)
        .filter(|length| *length > 0 && *length <= isize::MAX as usize)
        .ok_or("Invalid capture region dimensions")?;
    let mut bytes = Vec::with_capacity(length);
    for row in data[..source_length]
        .chunks_exact(stride)
        .skip(y as usize)
        .take(height as usize)
    {
        bytes.extend_from_slice(&row[x_bytes..end_x_bytes]);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_graphics::{
        color_space::CGColorSpace,
        data_provider::CGDataProvider,
        image::{CGImageAlphaInfo, CGImageByteOrderInfo},
    };
    use std::sync::Arc;

    fn synthetic_image() -> CGImage {
        // Distinct channels, alpha values and scanlines expose channel swaps,
        // alpha changes, row inversion and accidentally copied padding.
        let provider = CGDataProvider::from_buffer(Arc::new(vec![
            10, 20, 30, 255, 40, 50, 60, 128, 99, 98, 97, 96, 70, 80, 90, 192, 0, 0, 0, 0, 95, 94,
            93, 92,
        ]));
        CGImage::new(
            2,
            2,
            8,
            32,
            12,
            &CGColorSpace::create_device_rgb(),
            CGImageAlphaInfo::CGImageAlphaPremultipliedFirst as u32
                | CGImageByteOrderInfo::CGImageByteOrder32Little as u32,
            &provider,
            false,
            0,
        )
    }

    #[test]
    fn native_image_preserves_bgra_alpha_and_rows_without_padding() {
        // This is an in-memory CGImage, never a capture of the user's desktop.
        let bytes = {
            let image = synthetic_image();
            image_bgra(&image, 2, 2).unwrap()
        };
        // The returned allocation remains valid after the native image is gone.
        assert_eq!(
            bytes,
            [10, 20, 30, 255, 40, 50, 60, 128, 70, 80, 90, 192, 0, 0, 0, 0]
        );
    }

    #[test]
    fn packed_and_padded_rows_produce_identical_pixels() {
        let packed = [1, 2, 3, 4, 5, 6, 7, 8];
        let padded = [1, 2, 3, 4, 99, 99, 99, 99, 5, 6, 7, 8, 99, 99, 99, 99];
        assert_eq!(copy_bgra_rows(&packed, 1, 2, 4).unwrap(), packed);
        assert_eq!(copy_bgra_rows(&padded, 1, 2, 8).unwrap(), packed);
    }

    #[test]
    fn changed_dimensions_and_non_32_bit_images_are_rejected() {
        let image = synthetic_image();
        assert_eq!(
            image_bgra(&image, 4, 4).unwrap_err(),
            "Capture dimensions changed"
        );
        let provider = CGDataProvider::from_buffer(Arc::new(vec![1u8, 2, 3, 4]));
        let gray = CGImage::new(
            2,
            2,
            8,
            8,
            2,
            &CGColorSpace::create_device_gray(),
            CGImageAlphaInfo::CGImageAlphaNone as u32,
            &provider,
            false,
            0,
        );
        assert_eq!(
            image_bgra(&gray, 2, 2).unwrap_err(),
            "Unsupported capture pixel format"
        );
    }

    #[test]
    fn invalid_row_layouts_are_rejected_without_panicking() {
        for (width, height, stride) in [
            (0, 1, 4),
            (1, 0, 4),
            (1, 1, 0),
            (1, 1, 3),
            (1, 2, 8),
            (1, 2, usize::MAX),
            (u32::MAX, u32::MAX, usize::MAX),
        ] {
            assert!(copy_bgra_rows(&[0; 8], width, height, stride).is_err());
        }
    }
}
