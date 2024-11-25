use slint;
use std::env;
use std::error::Error;
use std::io::Write;
use std::fs::{self, File};
use std::sync::{LazyLock, Mutex};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use std::process::Command;
use std::os::windows::process::CommandExt;

use crate::util::file_util;

#[derive(Deserialize, Debug)]
pub struct VersionInfo {
    pub tag_name: String,
    pub body: String,
    pub assets: Vec<Asset>,
}

#[derive(Deserialize, Debug)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}

pub struct Updater {
    version_info: Option<VersionInfo>,
}

impl Updater {
    fn new() -> Updater {
        Updater {
            version_info: None,
        }
    }

    pub fn global() -> &'static Mutex<Updater> {
        &UPDATER_INS
    }

    pub fn get_latest_version(&mut self) -> Result<String, Box<dyn Error>> {
        let mut headers = HeaderMap::new();
        headers.insert("User-Agent", HeaderValue::from_static("Rotor"));
        
        let response = reqwest::blocking::Client::new()
            .get("https://api.github.com/repos/Horbin-Magician/Rotor/releases/latest")
            .headers(headers)
            .send()?;
        
        let status = response.status();
        if status.is_success() { 
            self.version_info = Some(response.json()?);
            let version = self.version_info
                .as_ref()
                .ok_or("Failed to get version_info ref")?
                .tag_name[1..].to_string();
            Ok(version)
        }
        else { Err(format!("Failed to get latest version, status: {status}").into()) }
    }

    pub fn get_update_info(&self) -> Option<String> {
        self.version_info.as_ref().map(|info| info.body.clone())
    }

    pub fn update_software(&self) -> Result<(), Box<dyn Error>> {
        if let Some(version_info) = &self.version_info { 
            let version = &version_info.tag_name;
            let asset = &version_info.assets[0];
            let download_url = &asset.browser_download_url;
            let zip_name = &asset.name;
        
            let file_response = reqwest::blocking::Client::new()
                .get(format!("https://mirror.ghproxy.com/{}", download_url)).send()
                .or_else( |_| {
                    reqwest::blocking::Client::new().get(format!("https://ghp.ci/{}", download_url)).send()
                })
                .or_else( |_| {
                    reqwest::blocking::Client::new().get(download_url).send()
                })?;
        
            if file_response.status().is_success() {
                let exe_path = env::current_exe()?;
                let app_path = exe_path.parent().ok_or("Failed to get app path")?;
                let tmp_path = app_path.join("tmp/");
                let zip_path = tmp_path.clone().join(zip_name);
                let zip_out_path = tmp_path.clone().join(version);
                let script_path = tmp_path.clone().join("update.bat");
                
                if tmp_path.exists() { fs::remove_dir_all(tmp_path.clone())?; } // remove old tmp dir
                fs::create_dir_all(zip_out_path.clone())?;
                
                let mut zip_file = File::create(&zip_path)?;
                let content = file_response.bytes()?;
                zip_file.write_all(&content)?;
                drop(zip_file);
                
                file_util::unzip(&zip_path, &zip_out_path)?;
                
                let old_dic = app_path.to_str().ok_or("Failed to get app path")?;
                let new_dic = zip_out_path.to_str().ok_or("Failed to get zip out path")?;
                let app_name = "rotor.exe";
                
                let bat_content = format!(r#"
                setlocal
                set "OLD_DIC={old_dic}"
                set "NEW_DIC={new_dic}"
                set "APP_NAME={app_name}"
                :waitloop
                timeout /t 1 /nobreak >nul
                tasklist | findstr /I "%APP_NAME%"
                if not errorlevel 1 ( goto waitloop )
                xcopy "%NEW_DIC%\*" "%OLD_DIC%\" /E /I /H /Y
                start "" "%OLD_DIC%\%APP_NAME%"
                endlocal
                "#);
                let mut file = File::create(&script_path)?;
                file.write_all(bat_content.as_bytes())?;
        
                Command::new("cmd")
                    .args(["/C", script_path.to_str().ok_or("Failed to get script path")?])
                    .creation_flags(0x08000000)
                    .spawn()?;
        
                slint::quit_event_loop()?;
            } else {
                return Err("Failed to update software, file response is not success".into());
            }
            Ok(())
        } else {
            Err("Failed to update software, version info is None".into())
        }
    }
}

static UPDATER_INS: LazyLock<Mutex<Updater>> = LazyLock::new(|| {
    Mutex::new(Updater::new())
});