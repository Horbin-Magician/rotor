//! Image-space geometry shared by native views and offscreen composition.
//! These are source pixels, never desktop coordinates or window logical units.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl ImageRect {
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
}
