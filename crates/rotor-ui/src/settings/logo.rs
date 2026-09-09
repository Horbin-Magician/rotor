use gpui_kit::{Image, ImageFormat};
use image::{Rgba32FImage, RgbaImage, imageops::FilterType};
use std::{io::Cursor, sync::Arc};

/// Keep only the current physical size; resizing or moving between displays
/// replaces the thumbnail without accumulating textures for every window size.
pub(super) struct Logo {
    source: Rgba32FImage,
    cached: Option<(u32, Arc<Image>)>,
}

impl Logo {
    pub(super) fn new() -> Self {
        let source = image::load_from_memory(include_bytes!("../../../../assets/icons/icon.png"))
            .expect("bundled settings logo must be a valid image")
            .into_rgba32f();
        Self {
            source: premultiply(source),
            cached: None,
        }
    }

    pub(super) fn image(&mut self, logical_size: f32, scale_factor: f32) -> Arc<Image> {
        let pixels = (logical_size * scale_factor).round().max(1.) as u32;
        if let Some((size, image)) = &self.cached
            && *size == pixels
        {
            return image.clone();
        }
        let thumbnail = thumbnail(&self.source, pixels);
        let mut encoded = Cursor::new(Vec::new());
        thumbnail
            .write_to(&mut encoded, image::ImageFormat::Png)
            .expect("encoding settings logo in memory must succeed");
        let image = Arc::new(Image::from_bytes(ImageFormat::Png, encoded.into_inner()));
        self.cached = Some((pixels, image.clone()));
        image
    }
}

fn premultiply(mut source: Rgba32FImage) -> Rgba32FImage {
    for pixel in source.pixels_mut() {
        for channel in 0..3 {
            pixel[channel] *= pixel[3];
        }
    }
    source
}

fn thumbnail(source: &Rgba32FImage, pixels: u32) -> RgbaImage {
    // Filter color with alpha so invisible RGB values cannot create fringes.
    // Encode straight alpha again, as expected by GPUI's PNG decoder.
    let resized = image::imageops::resize(source, pixels, pixels, FilterType::Lanczos3);
    RgbaImage::from_fn(pixels, pixels, |x, y| {
        let pixel = resized.get_pixel(x, y);
        let alpha = pixel[3].clamp(0., 1.);
        let mut output = [0; 4];
        if alpha > 0. {
            for channel in 0..3 {
                output[channel] = ((pixel[channel] / alpha).clamp(0., 1.) * 255.).round() as u8;
            }
        }
        output[3] = (alpha * 255.).round() as u8;
        image::Rgba(output)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downsampling_preserves_edge_color_without_transparent_rgb_bleed() {
        let source = Rgba32FImage::from_fn(64, 64, |x, _| {
            if x < 32 {
                image::Rgba([1., 0., 0., 1.])
            } else {
                image::Rgba([0., 0., 1., 0.])
            }
        });
        let result = thumbnail(&premultiply(source), 5);
        assert!(result.pixels().any(|pixel| pixel[3] > 0 && pixel[3] < 255));
        for pixel in result.pixels().filter(|pixel| pixel[3] > 0) {
            assert_eq!([pixel[0], pixel[1], pixel[2]], [255, 0, 0]);
        }
    }

    #[test]
    fn cache_tracks_physical_size_and_reuses_unchanged_images() {
        let mut logo = Logo::new();
        for scale in [1., 1.25, 1.5, 2.] {
            let first = logo.image(40., scale);
            assert_eq!(logo.cached.as_ref().unwrap().0, (40. * scale) as u32);
            assert!(Arc::ptr_eq(&first, &logo.image(40., scale)));
            assert!(!Arc::ptr_eq(&first, &logo.image(48., scale)));
            assert_eq!(logo.cached.as_ref().unwrap().0, (48. * scale) as u32);
        }
    }
}
