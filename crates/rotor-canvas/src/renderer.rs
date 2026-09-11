use crate::{arrow_outline, Annotation, Color, ImageRect, ImageSize, Scene, FONT_FAMILY};
use image::{GenericImageView, Rgba, RgbaImage};
use resvg::{
    tiny_skia::{ColorU8, FilterQuality, Pixmap, PixmapPaint, Transform},
    usvg,
};
use std::{fmt::Write, sync::Arc};

pub struct Renderer {
    fonts: Arc<usvg::fontdb::Database>,
}
impl Renderer {
    pub fn without_fonts() -> Self {
        Self {
            fonts: Arc::new(usvg::fontdb::Database::new()),
        }
    }
    /// Load installed fonts once; no application font resources are required.
    pub fn with_system_fonts() -> Result<Self, String> {
        let mut fonts = usvg::fontdb::Database::new();
        fonts.load_system_fonts();
        let fallback = fonts
            .faces()
            .flat_map(|face| &face.families)
            .map(|(family, _)| family.clone())
            .next()
            .ok_or("No system fonts are available for text annotations")?;
        let preferred_available = fonts.faces().any(|face| {
            face.families
                .iter()
                .any(|(family, _)| family == FONT_FAMILY)
        });
        fonts.set_sans_serif_family(if preferred_available {
            FONT_FAMILY.to_owned()
        } else {
            fallback
        });
        Ok(Self {
            fonts: Arc::new(fonts),
        })
    }

    /// Render the current crop at its requested physical viewport/export size.
    /// Vectors are rasterized at output resolution, after resizing the base.
    pub fn render(
        &self,
        source: &RgbaImage,
        scene: &Scene,
        output: ImageSize,
    ) -> Result<RgbaImage, String> {
        scene.validate()?;
        if source.dimensions() != (scene.size.width, scene.size.height) {
            return Err("Canvas source dimensions differ from its document".into());
        }
        if output.width == 0
            || output.height == 0
            || output.width > 16384
            || output.height > 16384
            || output.width as u64 * output.height as u64 > 64 * 1024 * 1024
        {
            return Err("Canvas output dimensions are unsupported".into());
        }
        let mut result = resize_crop(source, scene.crop, output)?;
        if scene.annotations.is_empty() {
            return Ok(result);
        }
        if scene.has_text() && self.fonts.faces().next().is_none() {
            return Err("Annotation font is unavailable".into());
        }
        let options = usvg::Options {
            font_family: FONT_FAMILY.into(),
            fontdb: self.fonts.clone(),
            ..Default::default()
        };
        let tree =
            usvg::Tree::from_str(&svg(scene), &options).map_err(|error| error.to_string())?;
        let sx = output.width as f32 / scene.crop.width as f32;
        let sy = output.height as f32 / scene.crop.height as f32;
        let tx = -(scene.crop.x as f32) * sx;
        let ty = -(scene.crop.y as f32) * sy;
        let transform = Transform::from_row(sx, 0., 0., sy, tx, ty);
        let Some(bounds) = tree.root().abs_layer_bounding_box().transform(transform) else {
            return Ok(result);
        };
        let left = (bounds.x().floor() - 1.).max(0.).min(output.width as f32) as u32;
        let top = (bounds.y().floor() - 1.).max(0.).min(output.height as f32) as u32;
        let right = (bounds.right().ceil() + 1.)
            .max(0.)
            .min(output.width as f32) as u32;
        let bottom = (bounds.bottom().ceil() + 1.)
            .max(0.)
            .min(output.height as f32) as u32;
        if right <= left || bottom <= top {
            return Ok(result);
        }
        // Limit temporary annotation memory to the actual painted bounds.
        let mut layer =
            Pixmap::new(right - left, bottom - top).ok_or("Cannot allocate annotation layer")?;
        resvg::render(
            &tree,
            Transform::from_row(sx, 0., 0., sy, tx - left as f32, ty - top as f32),
            &mut layer.as_mut(),
        );
        for (index, pixel) in layer.pixels().iter().enumerate() {
            let alpha = pixel.alpha() as u32;
            if alpha == 0 {
                continue;
            }
            let x = index as u32 % layer.width() + left;
            let y = index as u32 / layer.width() + top;
            let destination = result.get_pixel_mut(x, y);
            let old_alpha = destination[3] as u32;
            let inverse = 255 - alpha;
            let denominator = alpha * 255 + old_alpha * inverse;
            for (channel, foreground) in [pixel.red(), pixel.green(), pixel.blue()]
                .into_iter()
                .enumerate()
            {
                let numerator =
                    foreground as u32 * 65025 + destination[channel] as u32 * old_alpha * inverse;
                destination[channel] = ((numerator + denominator / 2) / denominator).min(255) as u8;
            }
            destination[3] = ((denominator + 127) / 255) as u8;
        }
        Ok(result)
    }
}

fn resize_crop(
    source: &RgbaImage,
    crop: ImageRect,
    output: ImageSize,
) -> Result<RgbaImage, String> {
    let source = image::imageops::crop_imm(source, crop.x, crop.y, crop.width, crop.height);
    if (crop.width, crop.height) == (output.width, output.height) {
        return Ok(source.to_image());
    }
    let mut premultiplied =
        Pixmap::new(crop.width, crop.height).ok_or("Cannot allocate canvas sampling buffer")?;
    for ((_, _, pixel), target) in source.pixels().zip(premultiplied.pixels_mut()) {
        *target = ColorU8::from_rgba(pixel[0], pixel[1], pixel[2], pixel[3]).premultiply();
    }
    let mut resized =
        Pixmap::new(output.width, output.height).ok_or("Cannot allocate canvas output")?;
    resized.draw_pixmap(
        0,
        0,
        premultiplied.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..Default::default()
        },
        Transform::from_scale(
            output.width as f32 / crop.width as f32,
            output.height as f32 / crop.height as f32,
        ),
        None,
    );
    Ok(RgbaImage::from_fn(output.width, output.height, |x, y| {
        let pixel = resized.pixels()[(y * output.width + x) as usize].demultiply();
        Rgba([pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()])
    }))
}

fn paint(color: Color) -> (String, f64) {
    let [r, g, b, a] = color.0;
    (format!("#{r:02x}{g:02x}{b:02x}"), a as f64 / 255.)
}
fn escaped(value: &str) -> String {
    let mut result = String::new();
    for character in value.chars() {
        match character {
            '&' => result.push_str("&amp;"),
            '<' => result.push_str("&lt;"),
            '>' => result.push_str("&gt;"),
            '"' => result.push_str("&quot;"),
            '\'' => result.push_str("&apos;"),
            '\u{0}'..='\u{8}'
            | '\u{b}'
            | '\u{c}'
            | '\u{e}'..='\u{1f}'
            | '\u{fffe}'
            | '\u{ffff}' => result.push('\u{fffd}'),
            _ => result.push(character),
        }
    }
    result
}
fn svg(scene: &Scene) -> String {
    let mut svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">", scene.size.width, scene.size.height, scene.size.width, scene.size.height);
    for annotation in &scene.annotations {
        match annotation {
            Annotation::Pen { points, style } => {
                let (color, opacity) = paint(style.color);
                if points.iter().all(|point| point == &points[0]) {
                    write!(svg, "<circle cx=\"{:.4}\" cy=\"{:.4}\" r=\"{:.4}\" fill=\"{color}\" fill-opacity=\"{opacity:.8}\"/>", points[0].x, points[0].y, style.width / 2.).unwrap();
                } else {
                    write!(svg, "<path d=\"M{:.4},{:.4}", points[0].x, points[0].y).unwrap();
                    for point in &points[1..] {
                        write!(svg, " L{:.4},{:.4}", point.x, point.y).unwrap();
                    }
                    write!(svg, "\" fill=\"none\" stroke=\"{color}\" stroke-opacity=\"{opacity:.8}\" stroke-width=\"{:.4}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>", style.width).unwrap();
                }
            }
            Annotation::Rectangle { start, end, style } => {
                let (color, opacity) = paint(style.color);
                write!(svg, "<rect x=\"{:.4}\" y=\"{:.4}\" width=\"{:.4}\" height=\"{:.4}\" fill=\"none\" stroke=\"{color}\" stroke-opacity=\"{opacity:.8}\" stroke-width=\"{:.4}\"/>", start.x.min(end.x), start.y.min(end.y), (start.x-end.x).abs(), (start.y-end.y).abs(), style.width).unwrap();
            }
            Annotation::Arrow { start, end, style } => {
                let (color, opacity) = paint(style.color);
                let outline = arrow_outline(*start, *end, style.width);
                if let Some(tip) = outline.first() {
                    write!(svg, "<path d=\"M{:.4},{:.4}", tip.x, tip.y).unwrap();
                    for point in &outline[1..] {
                        write!(svg, " L{:.4},{:.4}", point.x, point.y).unwrap();
                    }
                    write!(svg, " Z\" fill=\"{color}\" fill-opacity=\"{opacity:.8}\"/>").unwrap();
                }
            }
            Annotation::Text {
                origin,
                text,
                font_size,
                color,
            } => {
                let (color, opacity) = paint(*color);
                for (line, text) in text.lines().enumerate() {
                    write!(svg, "<text x=\"{:.4}\" y=\"{:.4}\" font-family=\"{FONT_FAMILY}\" font-size=\"{font_size:.4}\" dominant-baseline=\"text-before-edge\" xml:space=\"preserve\" fill=\"{color}\" fill-opacity=\"{opacity:.8}\">{}</text>", origin.x, origin.y + line as f64 * font_size * 1.25, escaped(text)).unwrap();
                }
            }
        }
    }
    svg.push_str("</svg>");
    svg
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ImagePoint, StrokeStyle};
    fn scene(width: u32, height: u32) -> Scene {
        Scene {
            size: ImageSize { width, height },
            crop: ImageRect {
                x: 0,
                y: 0,
                width,
                height,
            },
            annotations: Vec::new(),
        }
    }
    #[test]
    fn arrow_tip_has_no_round_protrusion_or_double_opacity() {
        let image = RgbaImage::new(80, 40);
        let mut scene = crate::Document::new(
            ImageSize {
                width: 80,
                height: 40,
            },
            ImageRect {
                x: 0,
                y: 0,
                width: 80,
                height: 40,
            },
        )
        .unwrap()
        .scene()
        .clone();
        scene.annotations.push(Annotation::Arrow {
            start: crate::ImagePoint { x: 10., y: 20. },
            end: crate::ImagePoint { x: 65., y: 20. },
            style: crate::StrokeStyle {
                color: Color([255, 0, 0, 128]),
                width: 6.,
            },
        });
        let rendered = Renderer::without_fonts()
            .render(&image, &scene, scene.size)
            .unwrap();
        assert!(rendered
            .enumerate_pixels()
            .filter(|(x, _, _)| *x >= 65)
            .all(|(_, _, pixel)| pixel[3] == 0));
        assert!(rendered.pixels().all(|pixel| pixel[3] <= 128));
        assert_eq!(rendered.get_pixel(25, 20)[3], 128);
    }

    #[test]
    fn unchanged_pixels_and_hidden_rgb_survive_unscaled_export() {
        let image = RgbaImage::from_fn(8, 8, |x, _| {
            Rgba([200, 100, 50, if x == 0 { 0 } else { 128 }])
        });
        assert_eq!(
            Renderer::without_fonts()
                .render(
                    &image,
                    &scene(8, 8),
                    ImageSize {
                        width: 8,
                        height: 8
                    }
                )
                .unwrap(),
            image
        );
    }
    #[test]
    fn source_over_alpha_is_correct_and_outside_pixels_are_untouched() {
        let image = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 255, 128]));
        let mut scene = scene(10, 10);
        scene.annotations.push(Annotation::Rectangle {
            start: ImagePoint { x: 2., y: 2. },
            end: ImagePoint { x: 8., y: 8. },
            style: StrokeStyle {
                color: Color([255, 0, 0, 128]),
                width: 2.,
            },
        });
        let rendered = Renderer::without_fonts()
            .render(
                &image,
                &scene,
                ImageSize {
                    width: 10,
                    height: 10,
                },
            )
            .unwrap();
        assert_eq!(rendered.get_pixel(2, 5).0, [170, 0, 85, 192]);
        assert_eq!(rendered.get_pixel(9, 9), image.get_pixel(9, 9));
    }
    #[test]
    fn resizing_transparency_does_not_bleed_hidden_blue_into_red() {
        let image = RgbaImage::from_fn(2, 1, |x, _| {
            if x == 0 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 0])
            }
        });
        let rendered = Renderer::without_fonts()
            .render(
                &image,
                &scene(2, 1),
                ImageSize {
                    width: 4,
                    height: 2,
                },
            )
            .unwrap();
        assert_eq!(rendered.dimensions(), (4, 2));
        assert!(rendered
            .pixels()
            .filter(|pixel| pixel[3] > 0)
            .all(|pixel| pixel[0] >= 254 && pixel[2] == 0));
    }

    #[test]
    fn vectors_are_rasterized_at_export_resolution() {
        let image = RgbaImage::from_pixel(8, 8, Rgba([255, 255, 255, 255]));
        let mut scene = scene(8, 8);
        scene.annotations.push(Annotation::Pen {
            points: vec![ImagePoint { x: 4., y: 1. }, ImagePoint { x: 4., y: 7. }],
            style: StrokeStyle {
                color: Color::RED,
                width: 1.,
            },
        });
        let rendered = Renderer::without_fonts()
            .render(
                &image,
                &scene,
                ImageSize {
                    width: 16,
                    height: 16,
                },
            )
            .unwrap();
        assert_eq!(rendered.get_pixel(7, 8).0, [255, 0, 0, 255]);
        assert_eq!(rendered.get_pixel(8, 8).0, [255, 0, 0, 255]);
        assert_eq!(rendered.get_pixel(6, 8).0, [255, 255, 255, 255]);
    }
    #[test]
    fn system_text_renders_and_markup_is_literal() {
        let renderer = Renderer::with_system_fonts().unwrap();
        let image = RgbaImage::new(128, 64);
        let mut scene = scene(128, 64);
        scene.annotations.push(Annotation::Text {
            origin: ImagePoint { x: 5., y: 5. },
            text: "Rotor".into(),
            font_size: 20.,
            color: Color::RED,
        });
        let rendered = renderer.render(&image, &scene, scene.size).unwrap();
        assert!(rendered.pixels().filter(|pixel| pixel[3] > 0).count() > 50);
        scene.annotations.push(Annotation::Text {
            origin: ImagePoint { x: 5., y: 30. },
            text: "中<&>\"'".into(),
            font_size: 12.,
            color: Color::RED,
        });
        assert!(renderer.render(&image, &scene, scene.size).is_ok());
    }
}
