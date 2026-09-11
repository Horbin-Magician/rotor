//! Native pin persistence. PNG completion precedes
//! metadata publication; failed metadata writes never change the in-memory map.
use crate::shotter_record::ShotterConfig;
use image::{ImageEncoder, RgbaImage};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone)]
pub struct StoredPin {
    pub id: u32,
    pub config: ShotterConfig,
    pub image: Arc<RgbaImage>,
}

pub struct PinStore {
    root: PathBuf,
    document: toml::Value,
    next_id: u32,
}

fn table<'a>(document: &'a mut toml::Value, path: &[&str]) -> Result<&'a mut toml::Table, String> {
    let mut value = document;
    for key in path {
        value = value
            .as_table_mut()
            .ok_or("Pin metadata contains an invalid table")?
            .entry((*key).to_string())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    }
    value
        .as_table_mut()
        .ok_or_else(|| "Pin metadata contains an invalid table".into())
}

fn validate(config: &ShotterConfig, width: u32, height: u32) -> Result<(), String> {
    let (x, y, w, h) = source_crop(config, width, height)?;
    rotor_canvas::Scene {
        size: rotor_canvas::ImageSize { width, height },
        crop: rotor_canvas::ImageRect {
            x,
            y,
            width: w,
            height: h,
        },
        annotations: config.annotations.clone(),
    }
    .validate()
}

/// image_rect is the PNG's origin/extent in the original monitor image, not a
/// crop inside the PNG. Every native record supplies this geometry.
pub fn source_crop(
    config: &ShotterConfig,
    width: u32,
    height: u32,
) -> Result<(u32, u32, u32, u32), String> {
    let (origin_x, origin_y, source_width, source_height) = config.image_rect;
    if (source_width, source_height) != (width, height)
        || width == 0
        || height == 0
        || config.zoom_factor == 0
    {
        return Err("Pin PNG dimensions or zoom do not match its source record".into());
    }
    let x = config
        .rect
        .0
        .checked_sub(origin_x)
        .ok_or("Pin crop starts before its PNG origin")?;
    let y = config
        .rect
        .1
        .checked_sub(origin_y)
        .ok_or("Pin crop starts before its PNG origin")?;
    let (w, h) = (config.rect.2, config.rect.3);
    if w == 0
        || h == 0
        || x.checked_add(w).is_none_or(|right| right > width)
        || y.checked_add(h).is_none_or(|bottom| bottom > height)
        || origin_x.checked_add(source_width).is_none()
        || origin_y.checked_add(source_height).is_none()
    {
        return Err("Pin crop is outside its source PNG".into());
    }
    Ok((x, y, w, h))
}

pub fn crop_image(image: &RgbaImage, config: &ShotterConfig) -> Result<RgbaImage, String> {
    let (x, y, width, height) = source_crop(config, image.width(), image.height())?;
    Ok(image::imageops::crop_imm(image, x, y, width, height).to_image())
}

pub fn png_bytes(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    image::codecs::png::PngEncoder::new(&mut bytes)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|error| error.to_string())?;
    Ok(bytes)
}

impl PinStore {
    pub fn load_from(data_directory: &Path) -> Result<Self, String> {
        if !data_directory.is_absolute() {
            return Err("Pin data directory must be absolute".into());
        }
        let root = data_directory.join("pins");
        let mut document = match fs::read_to_string(root.join("record.toml")) {
            Ok(text) => {
                toml::from_str(&text).map_err(|error| format!("Invalid pin metadata: {error}"))?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                toml::Value::Table(toml::Table::new())
            }
            Err(error) => return Err(error.to_string()),
        };
        let records = table(&mut document, &["pins"])?;
        let next_id = records
            .keys()
            .filter_map(|key| key.parse::<u32>().ok())
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or("Pin ID space exhausted")?;
        Ok(Self {
            root,
            document,
            next_id,
        })
    }

    fn image_path(&self, id: u32) -> PathBuf {
        self.root.join("images").join(format!("{id}.png"))
    }

    fn commit(&mut self, candidate: toml::Value) -> Result<(), String> {
        let bytes = toml::to_string_pretty(&candidate).map_err(|error| error.to_string())?;
        rotor_common::persistence::atomic_write_private(
            &self.root.join("record.toml"),
            bytes.as_bytes(),
        )
        .map_err(|error| error.to_string())?;
        self.document = candidate;
        Ok(())
    }

    pub fn load_pins(&self) -> (Vec<StoredPin>, Vec<String>) {
        self.load_pins_for_restore(true, &[])
    }

    /// Filter metadata before opening PNGs so hidden or already-open pins do
    /// not allocate image buffers during background startup/restoration.
    pub fn load_pins_for_restore(
        &self,
        include_hidden: bool,
        excluded_ids: &[u32],
    ) -> (Vec<StoredPin>, Vec<String>) {
        let mut pins = Vec::new();
        let mut warnings = Vec::new();
        let records = self.document["pins"].as_table().expect("validated at load");
        for (key, value) in records {
            let load = || -> Result<Option<StoredPin>, String> {
                let id = key.parse::<u32>().map_err(|error| error.to_string())?;
                if excluded_ids.contains(&id) {
                    return Ok(None);
                }
                let config: ShotterConfig = value
                    .clone()
                    .try_into()
                    .map_err(|error| error.to_string())?;
                if config.minimized && !include_hidden {
                    return Ok(None);
                }
                let image = image::open(self.image_path(id))
                    .map_err(|error| error.to_string())?
                    .into_rgba8();
                validate(&config, image.width(), image.height())?;
                Ok(Some(StoredPin {
                    id,
                    config,
                    image: Arc::new(image),
                }))
            };
            match load() {
                Ok(Some(pin)) => pins.push(pin),
                Ok(None) => {}
                Err(error) => warnings.push(format!("Pin {key}: {error}")),
            }
        }
        (pins, warnings)
    }

    pub fn create(&mut self, image: &RgbaImage, config: ShotterConfig) -> Result<u32, String> {
        validate(&config, image.width(), image.height())?;
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|error| error.to_string())?;
        fs::create_dir_all(self.root.join("images")).map_err(|error| error.to_string())?;
        let (id, path, mut file) = loop {
            let id = self.next_id;
            self.next_id = self
                .next_id
                .checked_add(1)
                .ok_or("Pin ID space exhausted")?;
            let path = self.image_path(id);
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(file) => break (id, path, file),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.to_string()),
            }
        };
        let write = file
            .write_all(&png)
            .and_then(|_| file.sync_all())
            .map_err(|error| error.to_string());
        drop(file);
        let result = write.and_then(|_| {
            let mut candidate = self.document.clone();
            table(&mut candidate, &["pins"])?.insert(
                id.to_string(),
                toml::Value::try_from(config).map_err(|error| error.to_string())?,
            );
            self.commit(candidate)
        });
        if let Err(error) = result {
            if let Err(cleanup) = fs::remove_file(path) {
                log::warn!("Orphan pin image after failed metadata write: {cleanup}");
            }
            return Err(error);
        }
        Ok(id)
    }

    pub fn update(&mut self, id: u32, config: ShotterConfig) -> Result<(), String> {
        let (width, height) =
            image::image_dimensions(self.image_path(id)).map_err(|error| error.to_string())?;
        validate(&config, width, height)?;
        let mut candidate = self.document.clone();
        let record = table(&mut candidate, &["pins"])?
            .get_mut(&id.to_string())
            .and_then(toml::Value::as_table_mut)
            .ok_or("Pin record does not exist")?;
        let known = toml::Value::try_from(config).map_err(|error| error.to_string())?;
        record.extend(known.as_table().ok_or("Invalid pin configuration")?.clone());
        self.commit(candidate)
    }

    pub fn delete(&mut self, id: u32) -> Result<(), String> {
        let mut candidate = self.document.clone();
        table(&mut candidate, &["pins"])?.remove(&id.to_string());
        self.commit(candidate)?;
        if let Err(error) = fs::remove_file(self.image_path(id)) {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!("Orphan image for deleted pin {id}: {error}");
            }
        }
        Ok(())
    }

    pub fn update_existing_batch(
        &mut self,
        updates: Vec<(u32, ShotterConfig)>,
    ) -> Result<Vec<String>, String> {
        let mut candidate = self.document.clone();
        let mut warnings = Vec::new();
        let mut changed = false;
        for (id, config) in updates {
            let Some(record) = table(&mut candidate, &["pins"])?
                .get_mut(&id.to_string())
                .and_then(toml::Value::as_table_mut)
            else {
                continue;
            };
            let prepare = || -> Result<toml::Table, String> {
                let (width, height) = image::image_dimensions(self.image_path(id))
                    .map_err(|error| error.to_string())?;
                validate(&config, width, height)?;
                toml::Value::try_from(&config)
                    .map_err(|error| error.to_string())?
                    .as_table()
                    .cloned()
                    .ok_or_else(|| "Invalid pin configuration".into())
            };
            match prepare() {
                Ok(known) => {
                    record.extend(known);
                    changed = true;
                }
                Err(error) => warnings.push(format!("Pin {id}: {error}")),
            }
        }
        if changed {
            self.commit(candidate)?;
        }
        Ok(warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> ShotterConfig {
        ShotterConfig {
            annotations: Vec::new(),
            monitor_pos: (-1920, 0),
            monitor_size: (2, 3),
            rect: (0, 0, 2, 3),
            image_rect: (0, 0, 2, 3),
            offset: (0, 0),
            zoom_factor: 100,
            mask_label: "ssmask-1".into(),
            minimized: false,
        }
    }

    #[test]
    fn annotations_survive_shutdown_and_restore() {
        use rotor_canvas::{
            Annotation, Color, Document, ImagePoint, ImageRect, ImageSize, StrokeStyle,
        };
        let directory = tempfile::tempdir().unwrap();
        let mut store = PinStore::load_from(directory.path()).unwrap();
        let id = store.create(&RgbaImage::new(2, 3), config()).unwrap();
        let mut document = Document::new(
            ImageSize {
                width: 2,
                height: 3,
            },
            ImageRect {
                x: 0,
                y: 0,
                width: 2,
                height: 3,
            },
        )
        .unwrap();
        document
            .add(Annotation::Arrow {
                start: ImagePoint { x: 0., y: 0. },
                end: ImagePoint { x: 1., y: 2. },
                style: StrokeStyle {
                    color: Color::RED,
                    width: 1.,
                },
            })
            .unwrap();
        document
            .add(Annotation::Text {
                origin: ImagePoint { x: 0., y: 1. },
                text: "标注".into(),
                font_size: 12.,
                color: Color::RED,
            })
            .unwrap();
        let mut record = config();
        record.annotations = document.scene().annotations.clone();
        assert!(store
            .update_existing_batch(vec![(id, record)])
            .unwrap()
            .is_empty());
        drop(store);
        let store = PinStore::load_from(directory.path()).unwrap();
        let (pins, warnings) = store.load_pins_for_restore(true, &[]);
        assert!(warnings.is_empty());
        assert_eq!(pins[0].config.annotations, document.scene().annotations);
        assert_eq!(*pins[0].image, RgbaImage::new(2, 3));
    }

    #[test]
    fn startup_skips_hidden_png_and_explicit_restore_can_load_it_later() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = PinStore::load_from(directory.path()).unwrap();
        let image = RgbaImage::new(2, 3);
        let visible = store.create(&image, config()).unwrap();
        let mut hidden_config = config();
        hidden_config.minimized = true;
        let hidden = store.create(&image, hidden_config).unwrap();
        let hidden_path = store.image_path(hidden);
        let original_png = fs::read(&hidden_path).unwrap();
        let original_record = fs::read(store.root.join("record.toml")).unwrap();
        // An unreadable hidden image must neither be decoded nor warn at startup.
        fs::write(&hidden_path, b"not a PNG").unwrap();
        let (pins, warnings) = store.load_pins_for_restore(false, &[]);
        assert!(warnings.is_empty());
        assert_eq!(
            pins.iter().map(|pin| pin.id).collect::<Vec<_>>(),
            vec![visible]
        );
        let (pins, warnings) = store.load_pins_for_restore(true, &[visible]);
        assert!(pins.is_empty());
        assert_eq!(warnings.len(), 1);

        fs::write(&hidden_path, &original_png).unwrap();
        // Existing windows are filtered before opening their images too.
        fs::write(store.image_path(visible), b"not a PNG").unwrap();
        let (pins, warnings) = store.load_pins_for_restore(true, &[visible]);
        assert!(warnings.is_empty());
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].id, hidden);
        assert!(pins[0].config.minimized);
        assert_eq!(*pins[0].image, image);
        assert_eq!(
            fs::read(store.root.join("record.toml")).unwrap(),
            original_record
        );
        assert_eq!(fs::read(hidden_path).unwrap(), original_png);
    }
    #[test]
    fn pins_round_trip_and_unknown_workspaces_survive() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("pins");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("record.toml"),
            "future_key = 'keep'\n[workspaces.archive]\nfuture = 7\n",
        )
        .unwrap();
        let mut store = PinStore::load_from(directory.path()).unwrap();
        let image = RgbaImage::from_pixel(2, 3, image::Rgba([1, 2, 3, 128]));
        let mut initial = config();
        initial.image_rect = (0, 0, 2, 3);
        let id = store.create(&image, initial).unwrap();
        let mut extended = store.document.clone();
        table(&mut extended, &["pins"])
            .unwrap()
            .get_mut(&id.to_string())
            .unwrap()
            .as_table_mut()
            .unwrap()
            .insert("future_pin".into(), toml::Value::String("keep".into()));
        store.commit(extended).unwrap();
        let mut updated = config();
        updated.offset = (21, -45);
        store.update(id, updated).unwrap();
        let restored = PinStore::load_from(directory.path()).unwrap();
        let (pins, warnings) = restored.load_pins();
        assert!(warnings.is_empty());
        assert_eq!(pins.len(), 1);
        assert_eq!(*pins[0].image, image);
        assert_eq!(pins[0].config.offset, (21, -45));
        assert_eq!(pins[0].config.image_rect, (0, 0, 2, 3));
        assert_eq!(
            restored.document["pins"][id.to_string()]["future_pin"].as_str(),
            Some("keep")
        );
        assert_eq!(restored.document["future_key"].as_str(), Some("keep"));
        assert_eq!(
            restored.document["workspaces"]["archive"]["future"].as_integer(),
            Some(7)
        );
        store.delete(id).unwrap();
        assert!(store.load_pins().0.is_empty());
        assert!(!store.image_path(id).exists());
    }
    #[test]
    fn failed_metadata_write_removes_only_the_new_image() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = PinStore::load_from(directory.path()).unwrap();
        fs::create_dir_all(store.root.join("record.toml")).unwrap();
        assert!(store.create(&RgbaImage::new(2, 3), config()).is_err());
        assert!(store.load_pins().0.is_empty());
        assert_eq!(fs::read_dir(store.root.join("images")).unwrap().count(), 0);
    }
    #[test]
    fn corrupt_metadata_is_not_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("pins");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("record.toml");
        fs::write(&path, "broken = [").unwrap();
        assert!(PinStore::load_from(directory.path()).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "broken = [");
    }

    #[test]
    fn cropped_png_uses_image_rect_as_source_origin() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = PinStore::load_from(directory.path()).unwrap();
        let image = RgbaImage::from_fn(4, 3, |x, y| {
            image::Rgba([x as u8 * 10, y as u8 * 10, 7, 128])
        });
        let mut record = config();
        record.monitor_size = (1920, 1080);
        record.rect = (100, 200, 4, 3);
        record.image_rect = (100, 200, 4, 3);
        let id = store.create(&image, record.clone()).unwrap();
        let (pins, warnings) = PinStore::load_from(directory.path()).unwrap().load_pins();
        assert!(warnings.is_empty());
        assert_eq!(crop_image(&pins[0].image, &pins[0].config).unwrap(), image);
        record.rect = (101, 201, 2, 2);
        store.update(id, record.clone()).unwrap();
        assert_eq!(
            crop_image(&image, &record).unwrap(),
            image::imageops::crop_imm(&image, 1, 1, 2, 2).to_image()
        );
        record.image_rect = (0, 0, 1920, 1080);
        assert!(store.update(id, record).is_err());
    }

    #[test]
    fn full_monitor_png_uses_explicit_source_origin() {
        let image = RgbaImage::new(4, 3);
        let mut record = config();
        record.monitor_size = (4, 3);
        record.image_rect = (0, 0, 4, 3);
        record.rect = (1, 1, 2, 2);
        assert_eq!(source_crop(&record, 4, 3).unwrap(), (1, 1, 2, 2));
        assert_eq!(crop_image(&image, &record).unwrap().dimensions(), (2, 2));
    }

    #[test]
    fn final_batch_does_not_resurrect_deleted_records() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = PinStore::load_from(directory.path()).unwrap();
        let image = RgbaImage::new(2, 3);
        let kept = store.create(&image, config()).unwrap();
        let deleted = store.create(&image, config()).unwrap();
        store.delete(deleted).unwrap();
        let mut latest = config();
        latest.offset = (99, -42);
        assert!(store
            .update_existing_batch(vec![(kept, latest.clone()), (deleted, latest)])
            .unwrap()
            .is_empty());
        let (pins, warnings) = PinStore::load_from(directory.path()).unwrap().load_pins();
        assert!(warnings.is_empty());
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].config.offset, (99, -42));
    }
}
