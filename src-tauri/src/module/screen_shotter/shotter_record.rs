use toml;
use std::error::Error;
use std::{collections::HashMap, fs};
use serde::{Serialize, Deserialize};
use image::{self, DynamicImage};

use crate::util::file_util;

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

const DEFAULT_SPACE_ID: &str = "default";

pub struct ShotterRecord {
    record: Record,
}

impl ShotterRecord {
    fn get_root_path() -> std::path::PathBuf {
        let root_path = file_util::get_userdata_path().unwrap().join("shotter");
        if !root_path.exists() {
            fs::create_dir_all(&root_path)
                .unwrap_or_else(|e| panic!("[ERROR] ShotterRecord create dir: {:?}", e));
        }
        return root_path;
    }

    fn get_img_path(id: u32) -> std::path::PathBuf {
        let space_path = ShotterRecord::get_root_path().join(DEFAULT_SPACE_ID.to_string());
        if !space_path.exists() {
            fs::create_dir_all(&space_path)
                .unwrap_or_else(|e| panic!("[ERROR] ShotterRecord create dir: {:?}", e));
        }
        space_path.join(format!("{}.png", id))
    }

    fn del_record_img(id: u32) -> Result<(), Box<dyn Error>> {
        let img_path = ShotterRecord::get_img_path(id);
        fs::remove_file(&img_path)?;
        Ok(())
    }

    pub fn save_record_img(id: u32, img: DynamicImage) {
        std::thread::spawn(move || {
            let path = ShotterRecord::get_img_path(id);
            img.save(path)
                .unwrap_or_else(|e| log::error!("Failed to save image: {}", e));
        });
    }

    pub fn load_record_img(id: u32) -> Result<DynamicImage, Box<dyn Error>> {
        let img_path = ShotterRecord::get_img_path(id);
        let img = image::open(img_path)?;
        Ok(img)
    }

    pub fn new() -> ShotterRecord {
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

    pub fn update_shotter(&mut self, id: u32, shotter: ShotterConfig) -> Result<(), Box<dyn Error>> {
        if let Some(workspace) = self.record.workspaces.get_mut(&DEFAULT_SPACE_ID.to_string()) {
            workspace.shotters.insert(id.to_string(), shotter);
        } else {
            let mut workspace = WorkSpace {
                shotters: HashMap::new(),
            };
            workspace.shotters.insert(id.to_string(), shotter);
            self.record.workspaces.insert(DEFAULT_SPACE_ID.to_string(), workspace);
        }
        self.save()?;
        Ok(())
    }

    pub fn del_shotter(&mut self, id: u32) -> Result<(), Box<dyn Error>> {
        if let Some(workspace) = self.record.workspaces.get_mut(&DEFAULT_SPACE_ID.to_string()) {
            ShotterRecord::del_record_img(id)?;
            workspace.shotters.remove(&id.to_string());
            self.save()?;
            return Ok(());
        }
        Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "No such workspace")))
    }

    pub fn get_shotters(&self) -> Option<&HashMap<String, ShotterConfig>> {
        if let Some(workspace) = self.record.workspaces.get(&DEFAULT_SPACE_ID.to_string()) {
            return Some(&workspace.shotters);
        }
        return None;
    }
}