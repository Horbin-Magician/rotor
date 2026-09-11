use crate::{ImagePoint, ImageRect, ImageSize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CropEdges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}
impl CropEdges {
    pub fn any(self) -> bool {
        self.left || self.right || self.top || self.bottom
    }
}

pub fn resize_crop(
    start: ImageRect,
    edges: CropEdges,
    delta: ImagePoint,
    image: ImageSize,
    minimum: u32,
) -> Option<ImageRect> {
    if start.clipped(image) != Some(start)
        || !delta.x.is_finite()
        || !delta.y.is_finite()
        || (edges.left && edges.right)
        || (edges.top && edges.bottom)
    {
        return None;
    }
    let dx = delta
        .x
        .clamp(-(image.width as f64), image.width as f64)
        .round() as i64;
    let dy = delta
        .y
        .clamp(-(image.height as f64), image.height as f64)
        .round() as i64;
    let (mut left, mut top) = (start.x as i64, start.y as i64);
    let (mut right, mut bottom) = (left + start.width as i64, top + start.height as i64);
    let min_width = minimum.max(1).min(image.width) as i64;
    let min_height = minimum.max(1).min(image.height) as i64;
    if edges.left {
        left = (left + dx).clamp(0, (right - min_width).max(0));
    }
    if edges.right {
        right = (right + dx).clamp(
            (left + min_width).min(image.width as i64),
            image.width as i64,
        );
    }
    if edges.top {
        top = (top + dy).clamp(0, (bottom - min_height).max(0));
    }
    if edges.bottom {
        bottom = (bottom + dy).clamp(
            (top + min_height).min(image.height as i64),
            image.height as i64,
        );
    }
    Some(ImageRect {
        x: left as u32,
        y: top as u32,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_edge_combination_stays_inside_source_and_preserves_opposite_edges() {
        let size = ImageSize {
            width: 100,
            height: 80,
        };
        let start = ImageRect {
            x: 20,
            y: 20,
            width: 50,
            height: 40,
        };
        for horizontal in [-1, 0, 1] {
            for vertical in [-1, 0, 1] {
                let edges = CropEdges {
                    left: horizontal == -1,
                    right: horizontal == 1,
                    top: vertical == -1,
                    bottom: vertical == 1,
                };
                for delta in [-1e100, -200., 0., 200., 1e100] {
                    let result =
                        resize_crop(start, edges, ImagePoint { x: delta, y: delta }, size, 12)
                            .unwrap();
                    assert_eq!(result.clipped(size), Some(result));
                    assert!(result.width >= 12 && result.height >= 12);
                    if edges.left {
                        assert_eq!(result.x + result.width, start.x + start.width);
                    }
                    if edges.top {
                        assert_eq!(result.y + result.height, start.y + start.height);
                    }
                }
            }
        }
    }
    #[test]
    fn tiny_sources_do_not_expand_beyond_the_png_to_meet_minimum() {
        let size = ImageSize {
            width: 3,
            height: 4,
        };
        let rect = ImageRect {
            x: 0,
            y: 0,
            width: 3,
            height: 4,
        };
        let edges = CropEdges {
            right: true,
            bottom: true,
            ..Default::default()
        };
        assert_eq!(
            resize_crop(rect, edges, ImagePoint { x: 50., y: 50. }, size, 12),
            Some(rect)
        );
        assert!(resize_crop(rect, edges, ImagePoint { x: f64::NAN, y: 0. }, size, 12).is_none());
    }
}
