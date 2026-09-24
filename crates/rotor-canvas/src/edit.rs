use crate::{Annotation, Color, ImagePoint};

impl Annotation {
    /// Selection uses a conservative image-space bounding box, including stroke width.
    pub fn bounds(&self) -> (ImagePoint, ImagePoint) {
        if let Some((origin, text, size, _)) = self.text_content() {
            return (
                origin,
                ImagePoint {
                    x: origin.x
                        + text
                            .lines()
                            .map(|line| line.chars().count())
                            .max()
                            .unwrap_or(1) as f64
                            * size,
                    y: origin.y + text.lines().count().max(1) as f64 * size * 1.25,
                },
            );
        }
        let (mut a, mut b, padding) = match self {
            Self::Pen { points, style } => {
                let first = points
                    .first()
                    .copied()
                    .unwrap_or(ImagePoint { x: 0., y: 0. });
                let (a, b) = points.iter().fold((first, first), |(a, b), p| {
                    (
                        ImagePoint {
                            x: a.x.min(p.x),
                            y: a.y.min(p.y),
                        },
                        ImagePoint {
                            x: b.x.max(p.x),
                            y: b.y.max(p.y),
                        },
                    )
                });
                (a, b, style.width / 2.)
            }
            Self::Rectangle { start, end, style } => (*start, *end, style.width / 2.),
            Self::Arrow { start, end, style } => (*start, *end, style.width * 2.),
            Self::Redaction { start, end } | Self::Mosaic { start, end, .. } => (*start, *end, 0.),
            Self::Text { .. } => unreachable!("text handled above"),
        };
        let left = a.x.min(b.x) - padding;
        let top = a.y.min(b.y) - padding;
        b = ImagePoint {
            x: a.x.max(b.x) + padding,
            y: a.y.max(b.y) + padding,
        };
        a = ImagePoint { x: left, y: top };
        (a, b)
    }
    pub fn hit_test(&self, point: ImagePoint, tolerance: f64) -> bool {
        let (a, b) = self.bounds();
        point.x >= a.x - tolerance
            && point.x <= b.x + tolerance
            && point.y >= a.y - tolerance
            && point.y <= b.y + tolerance
    }
    pub fn translated(&self, dx: f64, dy: f64) -> Self {
        let mut next = self.clone();
        let shift = |p: &mut ImagePoint| {
            p.x += dx;
            p.y += dy;
        };
        match &mut next {
            Self::Pen { points, .. } => points.iter_mut().for_each(shift),
            Self::Rectangle { start, end, .. }
            | Self::Arrow { start, end, .. }
            | Self::Redaction { start, end }
            | Self::Mosaic { start, end, .. } => {
                shift(start);
                shift(end);
            }
            Self::Text { origin, .. } => shift(origin),
        }
        next
    }
    pub fn recolored(&self, color: Color) -> Self {
        let mut next = self.clone();
        match &mut next {
            Self::Pen { style, .. } | Self::Rectangle { style, .. } | Self::Arrow { style, .. } => {
                style.color = color
            }
            Self::Text { color: current, .. } => *current = color,
            Self::Redaction { .. } | Self::Mosaic { .. } => {}
        }
        next
    }
    pub fn resized(&self, delta: f64) -> Self {
        let mut next = self.clone();
        match &mut next {
            Self::Pen { style, .. } | Self::Rectangle { style, .. } | Self::Arrow { style, .. } => {
                style.width = (style.width + delta).clamp(0.01, 1024.)
            }
            Self::Text { font_size, .. } => *font_size = (*font_size + delta).clamp(1., 2048.),
            Self::Mosaic { block_size, .. } => {
                *block_size = (*block_size as f64 + delta).clamp(4., 1024.) as u32
            }
            Self::Redaction { start, end } => {
                end.x = start.x
                    + (end.x - start.x).signum() * ((end.x - start.x).abs() + delta).max(1.);
                end.y = start.y
                    + (end.y - start.y).signum() * ((end.y - start.y).abs() + delta).max(1.);
            }
        }
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Document, ImageRect, ImageSize};
    #[test]
    fn editing_and_deleting_preserve_order_and_reject_invalid_history() {
        let mut doc = Document::new(
            ImageSize {
                width: 100,
                height: 100,
            },
            ImageRect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
        )
        .unwrap();
        let original = Annotation::Text {
            origin: ImagePoint { x: 10., y: 10. },
            text: "hello".into(),
            font_size: 16.,
            color: Color::RED,
        };
        doc.add(original.clone()).unwrap();
        doc.add(original.translated(0., 30.)).unwrap();
        let before = doc.scene().annotations.clone();
        let moved = original
            .translated(10., 5.)
            .recolored(Color([0, 0, 255, 255]))
            .resized(2.);
        assert!(moved.hit_test(ImagePoint { x: 22., y: 17. }, 0.));
        doc.replace(0, moved.clone()).unwrap();
        doc.remove(1).unwrap();
        assert_eq!(doc.scene().annotations, vec![moved.clone()]);
        assert!(doc.undo());
        assert!(doc.undo());
        assert_eq!(doc.scene().annotations, before);
        doc.replace(0, moved.clone()).unwrap();
        doc.remove(1).unwrap();
        assert_eq!(doc.scene().annotations, vec![moved.clone()]);
        let revision = doc.revision();
        assert!(doc.replace(0, original.translated(f64::NAN, 0.)).is_err());
        assert!(doc.remove(10).is_err());
        assert_eq!(doc.revision(), revision);
        assert!(!doc.replace(0, moved).unwrap());
        assert_eq!(doc.revision(), revision);
        assert!(doc.undo());
        doc.replace(0, original.clone()).unwrap();
        assert_eq!(doc.scene().annotations[0], original);
    }
}
