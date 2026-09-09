use gpui_kit::{Image, ImageFormat};
use image::{Rgba32FImage, RgbaImage, imageops::FilterType};
use std::{io::Cursor, sync::Arc};

/// Keep only the current physical size; resizing or moving between displays
/// replaces the thumbnail without accumulating textures for every window size.
pub(super) struct Logo {
    source: Rgba32FImage,
    cached: Option<(u32, Arc<Image>)>,
    glow: Option<Arc<Image>>,
}

impl Logo {
    pub(super) fn new() -> Self {
        let source = image::load_from_memory(include_bytes!("../../../../assets/icons/icon.png"))
            .expect("bundled settings logo must be a valid image")
            .into_rgba32f();
        Self {
            source: premultiply(source),
            cached: None,
            glow: None,
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
        self.glow = Some(encode(&contour_glow(&thumbnail)));
        let image = encode(&thumbnail);
        self.cached = Some((pixels, image.clone()));
        image
    }

    pub(super) fn glow(&self) -> Arc<Image> {
        self.glow
            .as_ref()
            .expect("logo image initializes glow")
            .clone()
    }
}

fn encode(image: &RgbaImage) -> Arc<Image> {
    let mut encoded = Cursor::new(Vec::new());
    image
        .write_to(&mut encoded, image::ImageFormat::Png)
        .expect("encoding settings logo in memory must succeed");
    Arc::new(Image::from_bytes(ImageFormat::Png, encoded.into_inner()))
}

pub(super) fn glow_padding(pixels: u32) -> u32 {
    (pixels as f32 * 0.1).ceil() as u32
}

fn contour_glow(logo: &RgbaImage) -> RgbaImage {
    // The padded alpha mask follows both the outside contour and the cutouts.
    // All dimensions scale with the thumbnail's physical size for Retina displays.
    let padding = glow_padding(logo.width());
    let size = logo.width() + padding * 2;
    let mut mask = image::GrayImage::new(size, size);
    for (x, y, pixel) in logo.enumerate_pixels() {
        mask.put_pixel(x + padding, y + padding, image::Luma([pixel[3]]));
    }
    let blurred = image::imageops::blur(&mask, logo.width() as f32 * 0.025);
    RgbaImage::from_fn(size, size, |x, y| {
        let alpha = blurred.get_pixel(x, y)[0] as f32;
        let outside = 1. - mask.get_pixel(x, y)[0] as f32 / 255.;
        image::Rgba([70, 190, 245, (alpha * outside * 1.8).min(255.) as u8])
    })
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
    fn glow_follows_outer_and_cutout_edges() {
        let logo = RgbaImage::from_fn(50, 50, |x, y| {
            let outer = (5..45).contains(&x) && (5..45).contains(&y);
            let hole = (15..35).contains(&x) && (15..35).contains(&y);
            image::Rgba([41, 167, 215, if outer && !hole { 255 } else { 0 }])
        });
        let glow = contour_glow(&logo);
        let padding = (glow.width() - logo.width()) / 2;
        let alpha = |x: u32, y: u32| glow.get_pixel(x + padding, y + padding)[3];
        assert!(alpha(4, 25) > 0, "outer edge glows");
        assert!(alpha(15, 25) > 0, "cutout edge glows");
        assert_eq!(alpha(10, 25), 0, "opaque logo stays unchanged");
        assert_eq!(alpha(25, 25), 0, "cutout center stays clear");
        assert_eq!(glow.get_pixel(0, 0)[3], 0);
    }

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
