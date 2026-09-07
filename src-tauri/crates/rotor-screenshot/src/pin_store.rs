//! Native pin persistence with the legacy record shape. PNG completion precedes
//! metadata publication; failed metadata writes never change the in-memory map.
use crate::shotter_record::ShotterConfig;
use image::{ImageEncoder, RgbaImage};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

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
    let (x, y, w, h) = config
        .image_rect
        .unwrap_or((0, 0, config.rect.2, config.rect.3));
    if w == 0
        || h == 0
        || x.checked_add(w).is_none_or(|right| right > width)
        || y.checked_add(h).is_none_or(|bottom| bottom > height)
        || config.zoom_factor == 0
    {
        return Err("Pin crop or zoom is outside its source image".into());
    }
    Ok(())
}

pub fn crop_image(image: &RgbaImage, config: &ShotterConfig) -> Result<RgbaImage, String> {
    validate(config, image.width(), image.height())?;
    let (x, y, width, height) = config
        .image_rect
        .unwrap_or((0, 0, config.rect.2, config.rect.3));
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
        let root = data_directory.join("shotter");
        let mut document = match fs::read_to_string(root.join("record.toml")) {
            Ok(text) => {
                toml::from_str(&text).map_err(|error| format!("Invalid pin metadata: {error}"))?
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                toml::Value::Table(toml::Table::new())
            }
            Err(error) => return Err(error.to_string()),
        };
        let records = table(&mut document, &["workspaces", "default", "shotters"])?;
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
        self.root.join("default").join(format!("{id}.png"))
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
        let mut pins = Vec::new();
        let mut warnings = Vec::new();
        let records = self.document["workspaces"]["default"]["shotters"]
            .as_table()
            .expect("validated at load");
        for (key, value) in records {
            let load = || -> Result<StoredPin, String> {
                let id = key.parse::<u32>().map_err(|error| error.to_string())?;
                let config: ShotterConfig = value
                    .clone()
                    .try_into()
                    .map_err(|error| error.to_string())?;
                let image = image::open(self.image_path(id))
                    .map_err(|error| error.to_string())?
                    .into_rgba8();
                validate(&config, image.width(), image.height())?;
                Ok(StoredPin {
                    id,
                    config,
                    image: Arc::new(image),
                })
            };
            match load() {
                Ok(pin) => pins.push(pin),
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
        fs::create_dir_all(self.root.join("default")).map_err(|error| error.to_string())?;
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
            table(&mut candidate, &["workspaces", "default", "shotters"])?.insert(
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
        let record = table(&mut candidate, &["workspaces", "default", "shotters"])?
            .get_mut(&id.to_string())
            .and_then(toml::Value::as_table_mut)
            .ok_or("Pin record does not exist")?;
        let known = toml::Value::try_from(config).map_err(|error| error.to_string())?;
        // None is omitted by serde; remove the old known optional value before
        // merging so clearing a crop cannot retain a previous image_rect.
        record.remove("image_rect");
        record.extend(known.as_table().ok_or("Invalid pin configuration")?.clone());
        self.commit(candidate)
    }

    pub fn delete(&mut self, id: u32) -> Result<(), String> {
        let mut candidate = self.document.clone();
        table(&mut candidate, &["workspaces", "default", "shotters"])?.remove(&id.to_string());
        self.commit(candidate)?;
        if let Err(error) = fs::remove_file(self.image_path(id)) {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!("Orphan image for deleted pin {id}: {error}");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config() -> ShotterConfig {
        ShotterConfig {
            monitor_pos: (-1920, 0),
            monitor_size: (1920, 1080),
            rect: (10, 20, 2, 3),
            image_rect: None,
            offset: (0, 0),
            zoom_factor: 100,
            mask_label: "ssmask-1".into(),
            minimized: false,
        }
    }
    #[test]
    fn pins_round_trip_and_unknown_workspaces_survive() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("shotter");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("record.toml"),
            "future_key = 'keep'\n[workspaces.archive]\nfuture = 7\n",
        )
        .unwrap();
        let mut store = PinStore::load_from(directory.path()).unwrap();
        let image = RgbaImage::from_pixel(2, 3, image::Rgba([1, 2, 3, 128]));
        let mut initial = config();
        initial.image_rect = Some((0, 0, 2, 3));
        let id = store.create(&image, initial).unwrap();
        let mut extended = store.document.clone();
        table(&mut extended, &["workspaces", "default", "shotters"])
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
        assert_eq!(pins[0].config.image_rect, None);
        assert_eq!(
            restored.document["workspaces"]["default"]["shotters"][id.to_string()]["future_pin"]
                .as_str(),
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
        assert_eq!(fs::read_dir(store.root.join("default")).unwrap().count(), 0);
    }
    #[test]
    fn corrupt_metadata_is_not_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("shotter");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("record.toml");
        fs::write(&path, "broken = [").unwrap();
        assert!(PinStore::load_from(directory.path()).is_err());
        assert_eq!(fs::read_to_string(path).unwrap(), "broken = [");
    }
}
