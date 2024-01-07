use std::env;
use std::error::Error;
use std::io::{self, Write};
use std::fs::{self, File};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::Deserialize;


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

pub fn get_latest_version() -> Result<VersionInfo, Box<dyn Error>> {
    let mut headers = HeaderMap::new();
    headers.insert("User-Agent", HeaderValue::from_static("Rotor"));

    let response = reqwest::blocking::Client::new()
        .get("https://api.github.com/repos/Horbin-Magician/Rotor/releases/latest")
        .headers(headers)
        .send()?;

    let status = response.status();
    if status.is_success() { Ok(response.json().unwrap()) }
    else { Err(format!("Failed to get latest version, status: {status}").into()) }
}

pub fn update_software(version_info: VersionInfo) -> Result<(), Box<dyn Error>> {
    let asset = &version_info.assets[0];
    let download_url = &asset.browser_download_url;
    let file_name = &asset.name;

    let file_response = reqwest::blocking::Client::new()
        .get(download_url)
        .send()?;

    if file_response.status().is_success() {
        let root_path = env::current_exe()?.parent().unwrap().join("tmp/");
        if root_path.exists() { fs::remove_dir_all(root_path.clone())?; } // remove old tmp dir
        fs::create_dir_all(root_path.clone())?;
        let fname = root_path.clone().join(&file_name);

        let mut file = File::create(&fname)?;
        let content = file_response.bytes()?;
        let _ = file.write_all(&content);
        drop(file);

        let file = fs::File::open(&fname)?;
        let mut archive = zip::ZipArchive::new(file)?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let tmppath = match file.enclosed_name() {
                Some(path) => path.to_owned(),
                None => continue,
            };
            let outpath = std::path::PathBuf::from(&root_path).join(&tmppath);

            if (*file.name()).ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() { fs::create_dir_all(p)?; }
                }
                let mut outfile = fs::File::create(&outpath)?;
                io::copy(&mut file, &mut outfile)?;
            }
        }
    }

    return Ok(());
}