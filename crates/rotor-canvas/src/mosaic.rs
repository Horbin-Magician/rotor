use crate::{Color, ImagePoint, ImageRect, ImageSize};
use image::{GenericImageView, Rgba};

pub const MAX_MOSAIC_TILES: u64 = 16384;

pub fn mosaic_bounds(
    start: ImagePoint,
    end: ImagePoint,
    size: ImageSize,
    block: u32,
) -> Option<ImageRect> {
    if !(4..=1024).contains(&block) {
        return None;
    }
    let rect = ImageRect::from_drag(start, end, size)?;
    (u64::from(rect.width.div_ceil(block)) * u64::from(rect.height.div_ceil(block))
        <= MAX_MOSAIC_TILES)
        .then_some(rect)
}

/// Sample one source pixel per tile, independent of display zoom and crop.
/// Work and retained geometry are bounded; previews never copy the source image.
/// Tiles are opaque, compositing the sampled source alpha against black.
pub fn mosaic_tiles(
    source: &impl GenericImageView<Pixel = Rgba<u8>>,
    start: ImagePoint,
    end: ImagePoint,
    block: u32,
) -> Vec<(ImageRect, Color)> {
    let Some(rect) = mosaic_bounds(
        start,
        end,
        ImageSize {
            width: source.width(),
            height: source.height(),
        },
        block,
    ) else {
        return Vec::new();
    };
    let mut tiles = Vec::new();
    for y in (rect.y..rect.y + rect.height).step_by(block as usize) {
        for x in (rect.x..rect.x + rect.width).step_by(block as usize) {
            let width = block.min(rect.x + rect.width - x);
            let height = block.min(rect.y + rect.height - y);
            let pixel = source.get_pixel(x + width / 2, y + height / 2).0;
            let mut color = [0, 0, 0, 255];
            for i in 0..3 {
                color[i] = ((u16::from(pixel[i]) * u16::from(pixel[3]) + 127) / 255) as u8;
            }
            tiles.push((
                ImageRect {
                    x,
                    y,
                    width,
                    height,
                },
                Color(color),
            ));
        }
    }
    tiles
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounds_limit_geometry_and_sample_partial_edge_tiles() {
        let source = image::RgbaImage::from_fn(11, 9, |x, y| Rgba([x as u8, y as u8, 80, 255]));
        let start = ImagePoint { x: 9.9, y: 8.1 };
        let end = ImagePoint { x: 1.2, y: 1.2 };
        let tiles = mosaic_tiles(&source, start, end, 4);
        assert_eq!(tiles.len(), 6);
        assert_eq!(
            tiles[0],
            (
                ImageRect {
                    x: 1,
                    y: 1,
                    width: 4,
                    height: 4
                },
                Color([3, 3, 80, 255])
            )
        );
        assert_eq!(
            tiles[5],
            (
                ImageRect {
                    x: 9,
                    y: 5,
                    width: 1,
                    height: 4
                },
                Color([9, 7, 80, 255])
            )
        );
        assert!(mosaic_bounds(
            ImagePoint { x: 0., y: 0. },
            ImagePoint {
                x: 16384.,
                y: 16384.
            },
            ImageSize {
                width: 16384,
                height: 16384
            },
            4
        )
        .is_none());
        assert!(mosaic_tiles(&source, start, end, 0).is_empty());
    }
}
