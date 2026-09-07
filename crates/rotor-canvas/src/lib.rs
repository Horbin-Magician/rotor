//! Image-space geometry shared by native views and offscreen composition.
//! These are source pixels, never desktop coordinates or window logical units.

mod crop;
mod document;
mod renderer;
pub use crop::{resize_crop, CropEdges};
pub use document::{
    arrow_head, Annotation, Color, Document, Scene, StrokeStyle, ViewTransform, FONT_FAMILY,
};
pub use renderer::Renderer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImagePoint {
    pub x: f64,
    pub y: f64,
}

impl ImagePoint {
    pub fn from_logical(x: f64, y: f64, scale: f64) -> Option<Self> {
        if !x.is_finite() || !y.is_finite() || !scale.is_finite() || scale <= 0. {
            return None;
        }
        let point = Self {
            x: x * scale,
            y: y * scale,
        };
        (point.x.is_finite() && point.y.is_finite()).then_some(point)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl ImageRect {
    pub fn from_drag(start: ImagePoint, end: ImagePoint, image: ImageSize) -> Option<Self> {
        if ![start.x, start.y, end.x, end.y]
            .iter()
            .all(|value| value.is_finite())
            || start.x == end.x
            || start.y == end.y
        {
            return None;
        }
        let left = start.x.min(end.x).clamp(0., image.width as f64).floor() as u32;
        let top = start.y.min(end.y).clamp(0., image.height as f64).floor() as u32;
        let right = start.x.max(end.x).clamp(0., image.width as f64).ceil() as u32;
        let bottom = start.y.max(end.y).clamp(0., image.height as f64).ceil() as u32;
        Self {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        }
        .clipped(image)
    }

    pub fn contains(self, point: ImagePoint) -> bool {
        point.x >= self.x as f64
            && point.y >= self.y as f64
            && point.x < self.x.saturating_add(self.width) as f64
            && point.y < self.y.saturating_add(self.height) as f64
    }
    pub fn clipped(self, image: ImageSize) -> Option<Self> {
        let right = self.x.saturating_add(self.width).min(image.width);
        let bottom = self.y.saturating_add(self.height).min(image.height);
        if right <= self.x || bottom <= self.y {
            return None;
        }
        Some(Self {
            x: self.x,
            y: self.y,
            width: right - self.x,
            height: bottom - self.y,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_is_clipped_without_overflow_or_empty_export() {
        let image = ImageSize {
            width: 640,
            height: 240,
        };
        assert_eq!(
            ImageRect {
                x: 600,
                y: 200,
                width: u32::MAX,
                height: 200
            }
            .clipped(image),
            Some(ImageRect {
                x: 600,
                y: 200,
                width: 40,
                height: 40
            })
        );
        assert_eq!(
            ImageRect {
                x: 640,
                y: 0,
                width: 1,
                height: 1
            }
            .clipped(image),
            None
        );
        assert_eq!(
            ImageRect {
                x: 0,
                y: 0,
                width: 0,
                height: 1
            }
            .clipped(image),
            None
        );
    }

    #[test]
    fn reversed_drag_keeps_fractional_dpi_source_pixels_and_clips_edges() {
        let start = ImagePoint::from_logical(20., 40., 1.25).unwrap();
        let end = ImagePoint::from_logical(-2., 0.5, 1.25).unwrap();
        assert_eq!(
            ImageRect::from_drag(
                start,
                end,
                ImageSize {
                    width: 100,
                    height: 100
                }
            ),
            Some(ImageRect {
                x: 0,
                y: 0,
                width: 25,
                height: 50
            })
        );
        assert!(ImageRect::from_drag(
            start,
            start,
            ImageSize {
                width: 100,
                height: 100
            }
        )
        .is_none());
        assert!(ImagePoint::from_logical(0., 0., f64::NAN).is_none());
        assert!(ImagePoint::from_logical(0., 0., 0.).is_none());
    }
}
