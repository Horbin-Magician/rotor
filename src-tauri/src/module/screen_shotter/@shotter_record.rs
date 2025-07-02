use slint::{Rgba8Pixel, SharedPixelBuffer};
use toml;
use std::error::Error;
use std::{collections::HashMap, fs};
use std::sync::{Arc, LazyLock, Mutex};
use serde::{Serialize, Deserialize};
use image;

use crate::core::application::app_config::AppConfig;
use crate::util::{file_util, img_util, log_util};


#[derive(Serialize, Deserialize, Debug)]
pub struct ShotterConfig {
    pub pos_x: i32,
    pub pos_y: i32,
    pub rect: (i32, i32, i32, i32),
    pub zoom_factor: i32,
}

#[derive(Serialize, Deserialize, Debug)]
struct WorkSpace {
    #[serde(default = "default_shotter")]
    shotters: HashMap<String, ShotterConfig>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Record {
    #[serde(default = "default_workspace")]
    workspaces: HashMap<String, WorkSpace>,
}

fn default_shotter() -> HashMap<String, ShotterConfig> { HashMap::<String, ShotterConfig>::default() }
fn default_workspace() -> HashMap<String, WorkSpace> { HashMap::<String, WorkSpace>::default() }

pub struct ShotterRecord {
    record: Record,
}

impl ShotterRecord {
    fn get_root_path() -> std::path::PathBuf {
        let root_path = file_util::get_userdata_path().join("shotter");
        if !root_path.exists() {
            fs::create_dir_all(&root_path)
                .unwrap_or_else(|e| panic!("[ERROR] ShotterRecord create dir: {:?}", e));
        }
        return root_path;
    }

    fn get_img_path(id: u32) -> std::path::PathBuf {
        let app_config = AppConfig::global().lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let space_id = app_config.get_current_workspace();
        let space_path = ShotterRecord::get_root_path().join(space_id.to_string());
        if !space_path.exists() {
            fs::create_dir_all(&space_path)
                .unwrap_or_else(|e| panic!("[ERROR] ShotterRecord create dir: {:?}", e));
        }
        space_path.join(format!("{}.png", id))
    }

    pub fn save_record_img(id: u32, img_buffer:Arc<Mutex<SharedPixelBuffer<Rgba8Pixel>>>) {
        let img_buffer = img_buffer.lock().unwrap();
        let img = img_util::shared_pixel_buffer_to_dynamic_image(&img_buffer);
        std::thread::spawn(move || {
            let path = ShotterRecord::get_img_path(id);
            img.save(path)
                .unwrap_or_else(|e| log_util::log_error(format!("Failed to save image: {}", e)));
        });
    }

    fn del_record_img(id: u32) -> Result<(), Box<dyn Error>> {
        let img_path = ShotterRecord::get_img_path(id);
        fs::remove_file(&img_path)?;
        Ok(())
    }

    pub fn load_record_img(id: u32) -> Result<SharedPixelBuffer<Rgba8Pixel>, Box<dyn Error>> {
        let img_path = ShotterRecord::get_img_path(id);
        let img = image::open(img_path)?;
        let rgba8_img = img.to_rgba8();
        let img_buffer= SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
            &rgba8_img,
            rgba8_img.width(),
            rgba8_img.height(),
        );
        Ok(img_buffer)
    }

    fn new() -> ShotterRecord {
        let root_path = ShotterRecord::get_root_path();
        let record_path = root_path.join("record.toml");

        let record_str = fs::read_to_string(record_path)
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
        let path = ShotterRecord::get_root_path().join("record.toml");
        let config_str = toml::to_string_pretty(&self.record)?;
        fs::write(path, config_str)?;
        Ok(())
    }

    pub fn global() -> &'static Mutex<ShotterRecord> {
        &INSTANCE
    }

    pub fn update_shotter(&mut self, id: u32, shotter: ShotterConfig) -> Result<(), Box<dyn Error>> {
        let app_config = AppConfig::global().lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let space_id = app_config.get_current_workspace();
        drop(app_config);

        if let Some(workspace) = self.record.workspaces.get_mut(&space_id.to_string()) {
            workspace.shotters.insert(id.to_string(), shotter);
        } else {
            let mut workspace = WorkSpace {
                shotters: HashMap::new(),
            };
            workspace.shotters.insert(id.to_string(), shotter);
            self.record.workspaces.insert(space_id.to_string(), workspace);
        }
        self.save()?;
        Ok(())
    }

    pub fn del_shotter(&mut self, id: u32) -> Result<(), Box<dyn Error>> {
        let app_config = AppConfig::global().lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let space_id = app_config.get_current_workspace();
        drop(app_config);

        if let Some(workspace) = self.record.workspaces.get_mut(&space_id.to_string()) {
            ShotterRecord::del_record_img(id)?;
            workspace.shotters.remove(&id.to_string());
            self.save()?;
            return Ok(());
        }
        Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "No such workspace")))
    }

    pub fn get_shotters(&self) -> Option<&HashMap<String, ShotterConfig>> {
        let app_config = AppConfig::global().lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let space_id = app_config.get_current_workspace();
        drop(app_config);

        if let Some(workspace) = self.record.workspaces.get(&space_id.to_string()) {
            return Some(&workspace.shotters);
        }
        return None;
    }
}

static INSTANCE: LazyLock<Mutex<ShotterRecord>> = LazyLock::new(|| {
    Mutex::new(ShotterRecord::new())
});