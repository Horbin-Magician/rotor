use image::{self, DynamicImage};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::{collections::HashMap, fs, io, thread};
use toml;

use rotor_platform::file_util;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShotterConfig {
    pub monitor_pos: (i32, i32),
    pub monitor_size: (u32, u32),
    pub rect: (u32, u32, u32, u32),
    pub offset: (i32, i32),
    pub zoom_factor: u32,
    pub mask_label: String,
    pub minimized: bool,
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

fn default_shotter() -> HashMap<String, ShotterConfig> {
    HashMap::<String, ShotterConfig>::default()
}

fn default_workspace() -> HashMap<String, WorkSpace> {
    HashMap::<String, WorkSpace>::default()
}

const DEFAULT_SPACE_ID: &str = "default";
static EMPTY_SHOTTERS: LazyLock<HashMap<String, ShotterConfig>> = LazyLock::new(HashMap::new);

pub struct ShotterRecord {
    record: Record,
}

impl ShotterRecord {
    fn get_root_path() -> Result<PathBuf, Box<dyn Error>> {
        let root_path = file_util::get_userdata_path()
            .ok_or_else(|| io::Error::other("Unable to resolve user data path"))?
            .join("shotter");
        fs::create_dir_all(&root_path)?;
        Ok(root_path)
    }

    fn get_img_path(id: u32) -> Result<PathBuf, Box<dyn Error>> {
        let space_path = ShotterRecord::get_root_path()?.join(DEFAULT_SPACE_ID);
        fs::create_dir_all(&space_path)?;
        Ok(space_path.join(format!("{}.png", id)))
    }

    fn del_record_img(id: u32) -> Result<(), Box<dyn Error>> {
        let img_path = ShotterRecord::get_img_path(id)?;
        match fs::remove_file(&img_path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(Box::new(error)),
        }
        Ok(())
    }

    pub fn save_record_img(id: u32, img: DynamicImage) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let path = match ShotterRecord::get_img_path(id) {
                Ok(path) => path,
                Err(error) => {
                    log::error!("Failed to resolve record image path: {error}");
                    return;
                }
            };

            if let Err(error) = img.save(path) {
                log::error!("Failed to save image: {error}");
            }
        })
    }

    pub fn load_record_img(id: u32) -> Result<DynamicImage, Box<dyn Error>> {
        let img_path = ShotterRecord::get_img_path(id)?;
        let img = image::open(img_path)?;
        Ok(img)
    }

    pub fn new() -> ShotterRecord {
        let record_path = match ShotterRecord::get_root_path() {
            Ok(root_path) => root_path.join("record.toml"),
            Err(error) => {
                log::error!("Failed to initialize shotter record path: {error}");
                return ShotterRecord {
                    record: Record {
                        workspaces: HashMap::new(),
                    },
                };
            }
        };

        let record_str = fs::read_to_string(&record_path).unwrap_or_else(|_| String::new());

        let mut record = match toml::from_str::<Record>(&record_str) {
            Ok(record) => record,
            Err(e) => {
                log::warn!("Failed to parse config file, creating default: {:?}", e);
                let default_record = Record {
                    workspaces: HashMap::new(),
                };
                if let Ok(config_str) = toml::to_string_pretty(&default_record) {
                    let _ = fs::write(&record_path, config_str);
                }
                default_record
            }
        };
        record
            .workspaces
            .entry(DEFAULT_SPACE_ID.to_string())
            .or_insert_with(|| WorkSpace {
                shotters: HashMap::new(),
            });

        ShotterRecord { record }
    }

    fn save(&self) -> Result<(), Box<dyn Error>> {
        let path = ShotterRecord::get_root_path()?.join("record.toml");
        let config_str = toml::to_string_pretty(&self.record)?;
        fs::write(path, config_str)?;
        Ok(())
    }

    pub fn update_shotter(
        &mut self,
        id: u32,
        shotter: ShotterConfig,
    ) -> Result<(), Box<dyn Error>> {
        self.default_workspace_mut()
            .shotters
            .insert(id.to_string(), shotter);
        self.save()?;
        Ok(())
    }

    pub fn del_shotter(&mut self, id: u32) -> Result<(), Box<dyn Error>> {
        if let Err(error) = ShotterRecord::del_record_img(id) {
            log::warn!("Failed to delete pin image {id}: {error}");
        }
        self.default_workspace_mut()
            .shotters
            .remove(&id.to_string());
        self.save()?;
        Ok(())
    }

    pub fn get_record(&self, id: u32) -> Option<&ShotterConfig> {
        self.record
            .workspaces
            .get(DEFAULT_SPACE_ID)?
            .shotters
            .get(&id.to_string())
    }

    pub fn get_records(&self) -> &HashMap<String, ShotterConfig> {
        self.record
            .workspaces
            .get(DEFAULT_SPACE_ID)
            .map(|workspace| &workspace.shotters)
            .unwrap_or(&EMPTY_SHOTTERS)
    }

    fn default_workspace_mut(&mut self) -> &mut WorkSpace {
        self.record
            .workspaces
            .entry(DEFAULT_SPACE_ID.to_string())
            .or_insert_with(|| WorkSpace {
                shotters: HashMap::new(),
            })
    }
}

impl Default for ShotterRecord {
    fn default() -> Self {
        Self::new()
    }
}
