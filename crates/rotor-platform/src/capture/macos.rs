use core_graphics::{
    display::CGDisplay,
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
