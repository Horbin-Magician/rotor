use slint::{Rgba8Pixel, SharedPixelBuffer};
use toml;
use std::error::Error;
use std::{collections::HashMap, fs};
use std::sync::{Arc, LazyLock, Mutex};
use serde::{Serialize, Deserialize};
use image;

use crate::util::{file_util, img_util, log_util};


#[derive(Serialize, Deserialize, Debug)]
pub struct ShotterConfig {
    pub pos_x: i32,
    pub pos_y: i32,
    pub rect: (f32, f32, f32, f32),
    pub zoom_factor: i32,
}

#[derive(Serialize, Deserialize, Debug)]
struct Record {
    #[serde(default = "default_hash_map")]
    shotters: HashMap<String, ShotterConfig>,
}

fn default_hash_map() -> HashMap<String, ShotterConfig> { HashMap::<String, ShotterConfig>::default() }

pub struct ShotterRecord {
    record: Record,
}

impl ShotterRecord {
    fn new() -> ShotterRecord {
        let root_path = file_util::get_userdata_path().join("shotter");
        if !root_path.exists() {
            fs::create_dir_all(&root_path)
                .unwrap_or_else(|e| panic!("[ERROR] ShotterRecord create dir: {:?}", e));
        }
        let path = root_path.join("record.toml");

        let record_str = fs::read_to_string(path)
            .unwrap_or_else(|_| String::new());

        let record = match toml::from_str::<Record>(&record_str) {
            Ok(record) => record,
            Err(e) => panic!("[ERROR] AppConfig read config: {:?}", e),
        };

        ShotterRecord {
            record,
        }
    }

    fn save(&self) -> Result<(), Box<dyn Error>> {
        let path = file_util::get_userdata_path().join("shotter").join("record.toml");
        let config_str = toml::to_string_pretty(&self.record)?;
        fs::write(path, config_str)?;
        Ok(())
    }

    pub fn global() -> &'static Mutex<ShotterRecord> {
        &INSTANCE
    }

    pub fn update_shotter(&mut self, id: u32, shotter: ShotterConfig) -> Result<(), Box<dyn Error>> {
        self.record.shotters.insert(id.to_string(), shotter);
        self.save()?;
        Ok(())
    }

    pub fn del_shotter(&mut self, id: u32) -> Result<(), Box<dyn Error>> {
        self.record.shotters.remove(&id.to_string());
        self.save()?;
        Ok(())
    }

    pub fn get_shotters(&self) -> &HashMap<String, ShotterConfig> {
        &self.record.shotters
    }

    pub fn save_record_img(id: u32, img_buffer:Arc<Mutex<SharedPixelBuffer<Rgba8Pixel>>>) {
        let img_buffer = img_buffer.lock().unwrap();
        let img = img_util::shared_pixel_buffer_to_dynamic_image(&img_buffer);
        std::thread::spawn(move || {
            let path = file_util::get_userdata_path().join("shotter").join(format!("{}.png", id));
            img.save(path)
                .unwrap_or_else(|e| log_util::log_error(format!("Failed to save image: {}", e)));
        });
    }

    pub fn del_record_img(id: u32) -> Result<(), Box<dyn Error>> {
        let img_path = file_util::get_userdata_path().join("shotter").join(format!("{}.png", id));
        fs::remove_file(&img_path)?;
        Ok(())
    }

    pub fn load_record_img(id: u32) -> Result<SharedPixelBuffer<Rgba8Pixel>, Box<dyn Error>> {
        let img_path = file_util::get_userdata_path().join("shotter").join(format!("{}.png", id));
        let img = image::open(img_path)?;
        let rgba8_img = img.to_rgba8();
        let img_buffer= SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
            &rgba8_img,
            rgba8_img.width(),
            rgba8_img.height(),
        );
        Ok(img_buffer)
    }
}

static INSTANCE: LazyLock<Mutex<ShotterRecord>> = LazyLock::new(|| {
    Mutex::new(ShotterRecord::new())
});