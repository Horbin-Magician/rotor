use anyhow::{Context, Result};
use image::{Rgba, RgbaImage};
use std::io::Cursor;

pub const TEXT: &str = "Rotor 中文标注 Aa 0123";

/// Straight-alpha RGBA source; no desktop capture or PNG round trip for display.
pub fn rgba() -> RgbaImage {
    let mut image = RgbaImage::new(640, 240);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        if y < 100 {
            *pixel = Rgba(match x / 160 {
                0 => [255, 0, 0, 255],
                1 => [0, 255, 0, 255],
                2 => [0, 0, 255, 255],
                _ => [255, 0, 0, 128],
            });
        } else if y == 110 || x == y {
            *pixel = Rgba([255, 255, 255, 255]);
        }
    }
    image
}

/// Candidate offscreen text path. The UI displays this exact bitmap as well as
/// GPUI native text so fallback, metrics and rasterization can be compared.
pub fn annotated() -> Result<RgbaImage> {
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    let family = if cfg!(target_os = "windows") {
        "Microsoft YaHei"
    } else {
        "PingFang SC"
    };
    anyhow::ensure!(
        options
            .fontdb
            .faces()
            .any(|face| face.families.iter().any(|(name, _)| name == family)),
        "Required comparison font {family} unavailable; cannot validate Chinese export"
    );
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="240"><path d="M10 130 L630 130 M10 138 L630 158" stroke="white" stroke-width="1"/><text x="16" y="205" font-family="{family}" font-size="28" fill="white">{TEXT}</text></svg>"#
    );
    let tree = resvg::usvg::Tree::from_str(&svg, &options)?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(640, 240).context("allocate text canvas")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let mut result = rgba();
    for (pixel, text) in result.pixels_mut().zip(pixmap.pixels()) {
        let text = text.demultiply();
        image::Pixel::blend(
            pixel,
            &Rgba([text.red(), text.green(), text.blue(), text.alpha()]),
        );
    }
    Ok(result)
}

pub fn png(image: &RgbaImage) -> Result<Vec<u8>> {
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png)?;
    Ok(bytes.into_inner())
}

pub fn bgra(image: &RgbaImage) -> RgbaImage {
    let mut image = image.clone();
    for pixel in image.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn channel_order_alpha_and_png_round_trip() {
        let source = rgba();
        assert_eq!(bgra(&source).get_pixel(0, 0).0, [0, 0, 255, 255]);
        assert_eq!(bgra(&source).get_pixel(500, 0).0, [0, 0, 255, 128]);
        assert_eq!(source.get_pixel(0, 239).0, [0, 0, 0, 0]);
        assert_eq!(
            image::load_from_memory(&png(&source).unwrap())
                .unwrap()
                .into_rgba8(),
            source
        );
    }

    #[test]
    fn offscreen_text_and_geometry_survive_export() {
        let rendered = annotated().expect("the platform comparison font must be installed");
        let decoded = image::load_from_memory(&png(&rendered).unwrap())
            .unwrap()
            .into_rgba8();
        assert_eq!(decoded.dimensions(), (640, 240));
        assert_eq!(decoded.get_pixel(500, 20).0, [255, 0, 0, 128]);
        assert!(
            decoded.get_pixel(50, 130).0[3] > 0,
            "horizontal stroke missing"
        );
        assert!(
            decoded
                .enumerate_pixels()
                .filter(|(x, y, p)| *x >= 110 && *x < 250 && *y > 170 && *y < 207 && p.0[3] > 0)
                .count()
                > 100,
            "Chinese text region is blank"
        );
    }
}
