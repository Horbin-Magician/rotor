use slint;
use std::env;
use std::error::Error;
use std::io::Write;
use std::fs::{self, File};
use once_cell::sync::Lazy;
use std::sync::Mutex;
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;
use std::process::Command;
use std::os::windows::process::CommandExt;
use crate::core::util::file_util::unzip;

#[derive(Deserialize)]
pub struct VersionInfo {
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

#[derive(Deserialize)]
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
            let version = self.version_info.as_ref().unwrap().tag_name[1..].to_string();
            Ok(version)
        }
        else { Err(format!("Failed to get latest version, status: {status}").into()) }
    }

    pub fn update_software(&self) -> Result<(), Box<dyn Error>> {
        if let Some(version_info) = &self.version_info { 
            let version = &version_info.tag_name;
            let asset = &version_info.assets[0];
            let download_url = &asset.browser_download_url;
            let zip_name = &asset.name;
        
            let file_response = reqwest::blocking::Client::new()
                .get(download_url)
                .send()?;
        
            if file_response.status().is_success() {
                let exe_path = env::current_exe()?;
                let app_path = exe_path.parent().unwrap();
                let tmp_path = app_path.join("tmp/");
                let zip_path = tmp_path.clone().join(&zip_name);
                let zip_out_path = tmp_path.clone().join(version);
                let script_path = tmp_path.clone().join("update.bat");
        
                if tmp_path.exists() { fs::remove_dir_all(tmp_path.clone())?; } // remove old tmp dir
                fs::create_dir_all(zip_out_path.clone())?;
        
                let mut zip_file = File::create(&zip_path)?;
                let content = file_response.bytes()?;
                zip_file.write_all(&content)?;
                drop(zip_file);
        
                unzip(&zip_path, &zip_out_path)?;
        
                let old_dic = app_path.to_str().unwrap();
                let new_dic = zip_out_path.to_str().unwrap();
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
                    .args(&["/C", script_path.to_str().unwrap()])
                    .creation_flags(0x08000000) // CREATE_NO_WINDOW
                    .spawn()?;
        
                slint::quit_event_loop().unwrap();
            }
            return Ok(());
        } else {
            return Err("Failed to update software, version info is None".into());
        }
    }
}

static UPDATER_INS: Lazy<Mutex<Updater>> = Lazy::new(|| {
    Mutex::new(Updater::new())
});