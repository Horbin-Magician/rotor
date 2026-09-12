use crate::{ImagePoint, ImageRect, ImageSize};

/// System family shared by annotation previews and offscreen rendering.
#[cfg(target_os = "windows")]
pub const FONT_FAMILY: &str = "Microsoft YaHei";
#[cfg(target_os = "macos")]
pub const FONT_FAMILY: &str = "PingFang SC";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub const FONT_FAMILY: &str = "DejaVu Sans";
const MAX_MARKS: usize = 4096;

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color(pub [u8; 4]);
impl Color {
    pub const RED: Self = Self([255, 0, 0, 255]);
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct StrokeStyle {
    pub color: Color,
    pub width: f64,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
pub enum Annotation {
    Pen {
        points: Vec<ImagePoint>,
        style: StrokeStyle,
    },
    Rectangle {
        start: ImagePoint,
        end: ImagePoint,
        style: StrokeStyle,
    },
    Arrow {
        start: ImagePoint,
        end: ImagePoint,
        style: StrokeStyle,
    },
    Text {
        origin: ImagePoint,
        text: String,
        font_size: f64,
        color: Color,
    },
}

impl Annotation {
    fn validate(&self, size: ImageSize) -> Result<(), String> {
        let point = |point: &ImagePoint| {
            point.x.is_finite()
                && point.y.is_finite()
                && point.x.abs() <= size.width as f64 + 4096.
                && point.y.abs() <= size.height as f64 + 4096.
        };
        let stroke =
            |style: &StrokeStyle| style.width.is_finite() && (0.01..=1024.).contains(&style.width);
        let valid = match self {
            Self::Pen { points, style } => {
                !points.is_empty()
                    && points.len() <= 65536
                    && points.iter().all(point)
                    && stroke(style)
            }
            Self::Rectangle { start, end, style } => {
                point(start) && point(end) && start.x != end.x && start.y != end.y && stroke(style)
            }
            Self::Arrow { start, end, style } => {
                point(start) && point(end) && start != end && stroke(style)
            }
            Self::Text {
                origin,
                text,
                font_size,
                ..
            } => {
                point(origin)
                    && font_size.is_finite()
                    && (1.0..=2048.).contains(font_size)
                    && !text.trim().is_empty()
                    && text.chars().count() <= 16384
            }
        };
        if valid {
            Ok(())
        } else {
            Err("Invalid or oversized annotation".into())
        }
    }
}

/// One filled silhouette shared by the live overlay and exported pixels.
/// The shaft ends at the head's shoulders, so no rounded stroke protrudes at the tip.
pub fn arrow_outline(start: ImagePoint, end: ImagePoint, width: f64) -> Vec<ImagePoint> {
    let (dx, dy) = (end.x - start.x, end.y - start.y);
    let length = dx.hypot(dy);
    if !length.is_finite() || length <= f64::EPSILON || !width.is_finite() || width <= 0. {
        return Vec::new();
    }
    let (ux, uy) = (dx / length, dy / length);
    let width = width.min(length / 3.);
    let radius = width / 2.;
    let head = (width * 4.5).min(length * 0.6);
    let neck = length - head;
    let wing = head * 0.45;
    let point = |x: f64, y: f64| ImagePoint {
        x: start.x + ux * x - uy * y,
        y: start.y + uy * x + ux * y,
    };
    let mut outline = vec![end, point(neck, wing), point(neck, radius)];
    // A small semicircle forms the tail; fill once to avoid seams and alpha overlap.
    for step in 0..=24 {
        let angle = std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * step as f64 / 24.;
        outline.push(point(radius * angle.cos(), radius * angle.sin()));
    }
    outline.extend([point(neck, -radius), point(neck, -wing)]);
    outline
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub size: ImageSize,
    pub crop: ImageRect,
    pub annotations: Vec<Annotation>,
}
impl Scene {
    pub fn validate(&self) -> Result<(), String> {
        if self.size.width == 0
            || self.size.height == 0
            || self.crop.clipped(self.size) != Some(self.crop)
            || self.annotations.len() > MAX_MARKS
        {
            return Err("Canvas dimensions or crop are invalid".into());
        }
        for annotation in &self.annotations {
            annotation.validate(self.size)?;
        }
        Ok(())
    }
    pub fn has_text(&self) -> bool {
        self.annotations
            .iter()
            .any(|mark| matches!(mark, Annotation::Text { .. }))
    }
}

#[derive(Clone, Debug)]
enum Undo {
    Added,
    Crop(ImageRect),
}
#[derive(Clone, Debug)]
enum Redo {
    Add(Annotation),
    Crop(ImageRect),
}

#[derive(Clone, Debug)]
pub struct Document {
    scene: Scene,
    undo: Vec<Undo>,
    redo: Vec<Redo>,
    revision: u64,
}
impl Document {
    pub fn new(size: ImageSize, crop: ImageRect) -> Result<Self, String> {
        let scene = Scene {
            size,
            crop,
            annotations: Vec::new(),
        };
        scene.validate()?;
        Ok(Self {
            scene,
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
        })
    }
    pub fn scene(&self) -> &Scene {
        &self.scene
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }
    pub fn add(&mut self, annotation: Annotation) -> Result<(), String> {
        annotation.validate(self.scene.size)?;
        if self.scene.annotations.len() >= MAX_MARKS {
            return Err("Canvas annotation limit reached".into());
        }
        self.scene.annotations.push(annotation);
        self.undo.push(Undo::Added);
        self.redo.clear();
        self.changed();
        Ok(())
    }
    pub fn set_crop(&mut self, crop: ImageRect) -> Result<bool, String> {
        if crop.clipped(self.scene.size) != Some(crop) {
            return Err("Crop is outside the source image".into());
        }
        if crop == self.scene.crop {
            return Ok(false);
        }
        self.undo.push(Undo::Crop(self.scene.crop));
        self.scene.crop = crop;
        self.redo.clear();
        self.changed();
        Ok(true)
    }
    pub fn undo(&mut self) -> bool {
        let Some(change) = self.undo.pop() else {
            return false;
        };
        match change {
            Undo::Added => self.redo.push(Redo::Add(
                self.scene
                    .annotations
                    .pop()
                    .expect("undo tracks annotation additions"),
            )),
            Undo::Crop(previous) => {
                self.redo.push(Redo::Crop(self.scene.crop));
                self.scene.crop = previous;
            }
        }
        self.changed();
        true
    }
    pub fn redo(&mut self) -> bool {
        let Some(change) = self.redo.pop() else {
            return false;
        };
        match change {
            Redo::Add(annotation) => {
                self.scene.annotations.push(annotation);
                self.undo.push(Undo::Added);
            }
            Redo::Crop(next) => {
                self.undo.push(Undo::Crop(self.scene.crop));
                self.scene.crop = next;
            }
        }
        self.changed();
        true
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ViewTransform {
    pub crop: ImageRect,
    pub width: f64,
    pub height: f64,
}
impl ViewTransform {
    pub fn to_image(self, point: ImagePoint) -> Option<ImagePoint> {
        if !self.width.is_finite()
            || !self.height.is_finite()
            || self.width <= 0.
            || self.height <= 0.
            || !point.x.is_finite()
            || !point.y.is_finite()
        {
            return None;
        }
        let mapped = ImagePoint {
            x: self.crop.x as f64 + point.x * self.crop.width as f64 / self.width,
            y: self.crop.y as f64 + point.y * self.crop.height as f64 / self.height,
        };
        (mapped.x.is_finite() && mapped.y.is_finite()).then_some(mapped)
    }
    pub fn to_view(self, point: ImagePoint) -> Option<ImagePoint> {
        if self.crop.width == 0
            || self.crop.height == 0
            || !self.width.is_finite()
            || !self.height.is_finite()
            || self.width <= 0.
            || self.height <= 0.
            || !point.x.is_finite()
            || !point.y.is_finite()
        {
            return None;
        }
        let mapped = ImagePoint {
            x: (point.x - self.crop.x as f64) * self.width / self.crop.width as f64,
            y: (point.y - self.crop.y as f64) * self.height / self.crop.height as f64,
        };
        (mapped.x.is_finite() && mapped.y.is_finite()).then_some(mapped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arrows_have_a_single_tip_and_proportional_short_heads() {
        for length in [0.1, 3., 100.] {
            for angle in [0_f64, 0.7, 1.57, 3.8] {
                let (ux, uy) = (angle.cos(), angle.sin());
                let start = ImagePoint { x: 100., y: 100. };
                let end = ImagePoint {
                    x: start.x + ux * length,
                    y: start.y + uy * length,
                };
                let outline = arrow_outline(start, end, 4.);
                assert_eq!(outline[0], end);
                for point in &outline[1..] {
                    let along = (point.x - start.x) * ux + (point.y - start.y) * uy;
                    assert!(along < length, "only the sharp tip may reach the endpoint");
                    let across = -(point.x - start.x) * uy + (point.y - start.y) * ux;
                    assert!(across.abs() <= length * 0.27 + 1e-9);
                }
            }
        }
        let point = ImagePoint { x: 0., y: 0. };
        assert!(arrow_outline(point, point, 4.).is_empty());
    }

    fn full() -> ImageRect {
        ImageRect {
            x: 0,
            y: 0,
            width: 100,
            height: 80,
        }
    }
    #[test]
    fn invalid_edits_do_not_change_history_or_revision() {
        let mut doc = Document::new(
            ImageSize {
                width: 100,
                height: 80,
            },
            full(),
        )
        .unwrap();
        assert!(doc
            .add(Annotation::Pen {
                points: vec![ImagePoint { x: f64::NAN, y: 0. }],
                style: StrokeStyle {
                    color: Color::RED,
                    width: 3.
                }
            })
            .is_err());
        assert!(doc
            .set_crop(ImageRect {
                x: 99,
                y: 0,
                width: 2,
                height: 1
            })
            .is_err());
        assert_eq!(doc.revision(), 0);
        assert!(!doc.can_undo());
    }
    #[test]
    fn crop_and_annotation_undo_redo_preserve_order() {
        let mut doc = Document::new(
            ImageSize {
                width: 100,
                height: 80,
            },
            full(),
        )
        .unwrap();
        doc.add(Annotation::Text {
            origin: ImagePoint { x: 2., y: 3. },
            text: "中文".into(),
            font_size: 16.,
            color: Color::RED,
        })
        .unwrap();
        doc.set_crop(ImageRect {
            x: 10,
            y: 10,
            width: 40,
            height: 30,
        })
        .unwrap();
        assert!(doc.undo());
        assert_eq!(doc.scene().crop, full());
        assert!(doc.undo());
        assert!(doc.scene().annotations.is_empty());
        assert!(doc.redo());
        assert_eq!(doc.scene().annotations.len(), 1);
        assert!(doc.redo());
        assert_eq!(doc.scene().crop.width, 40);
    }
    #[test]
    fn viewport_mapping_round_trips_fractional_zoom() {
        let transform = ViewTransform {
            crop: ImageRect {
                x: 100,
                y: 200,
                width: 640,
                height: 240,
            },
            width: 160.,
            height: 60.,
        };
        let point = ImagePoint { x: 40.5, y: 30.25 };
        let source = transform.to_image(point).unwrap();
        assert_eq!(source, ImagePoint { x: 262., y: 321. });
        assert_eq!(transform.to_view(source), Some(point));
    }
}
