//! Retained synthetic fixture: old record shape -> native write -> legacy reader.
//! Does not capture the desktop or open any windows.
use rotor_common::{profile_migration, ConfigService};
use rotor_screenshot::{
    pin_store::{source_crop, PinStore},
    shotter_record::ShotterConfig,
};
use serde::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf};

// Keep the old reader shape independent of PinStore so this example verifies
// that native writes remain readable by the legacy format.
#[derive(Deserialize)]
struct LegacyWorkspace {
    #[serde(default)]
    shotters: HashMap<String, ShotterConfig>,
}

#[derive(Deserialize)]
struct LegacyRecord {
    #[serde(default)]
    workspaces: HashMap<String, LegacyWorkspace>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("usage: profile_roundtrip <new fixture directory>")?,
    );
    fs::create_dir(&directory)?;
    let directory = directory.canonicalize()?;
    let source = directory.join("legacy");
    fs::create_dir_all(source.join("shotter/default"))?;
    fs::write(source.join("config.toml"), "theme = '1'\nfuture_key = 'keep me'\nquick_actions = '[]'\nquick_actions_revision = '2'\ncurrent_workspace = '0'\n")?;
    fs::write(
        source.join("shotter/record.toml"),
        r#"
future_top = "keep me"
[workspaces.other]
future_field = "keep me"
[workspaces.other.shotters]
[workspaces.default.shotters.7]
monitor_pos = [-1920, 0]
monitor_size = [1920, 1080]
rect = [102, 201, 4, 6]
image_rect = [100, 200, 8, 8]
offset = [-1800, 40]
zoom_factor = 100
mask_label = "ssmask-legacy"
minimized = false
"#,
    )?;
    let image = image::RgbaImage::from_fn(8, 8, |x, y| {
        image::Rgba([x as u8 * 20, y as u8 * 20, 90, 255])
    });
    image.save(source.join("shotter/default/7.png"))?;
    let native = directory.join("native");
    let backup = directory.join("legacy-backup");
    profile_migration::import(&source, &native, &backup, "2.6.0 fixture")?;
    let mut settings = ConfigService::load_from(&native)?;
    settings.set("theme".into(), "2".into())?;
    assert_eq!(
        settings.get_all().get("future_key").map(String::as_str),
        Some("keep me")
    );
    let mut store = PinStore::load_from(&native)?;
    let (pins, warnings) = store.load_pins();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(pins.len(), 1);
    let mut config = pins[0].config.clone();
    assert_eq!(source_crop(&config, 8, 8)?, (2, 1, 4, 6));
    config.zoom_factor = 200;
    config.offset = (-1750, 60);
    store.update(7, config)?;
    let native_text = fs::read_to_string(native.join("shotter/record.toml"))?;
    let document: toml::Value = toml::from_str(&native_text)?;
    assert_eq!(document["future_top"].as_str(), Some("keep me"));
    assert_eq!(
        document["workspaces"]["other"]["future_field"].as_str(),
        Some("keep me")
    );
    let legacy_reader: LegacyRecord = toml::from_str(&native_text)?;
    let record = legacy_reader
        .workspaces
        .get("default")
        .and_then(|workspace| workspace.shotters.get("7"))
        .ok_or("legacy reader lost pin")?;
    assert_eq!(record.zoom_factor, 200);
    assert_eq!(record.offset, (-1750, 60));
    assert_eq!(record.image_rect, Some((100, 200, 8, 8)));
    assert_eq!(
        image::open(native.join("shotter/default/7.png"))?.to_rgba8(),
        image
    );
    assert_eq!(
        fs::read_to_string(native.join("shotter/record.toml"))?,
        native_text
    );
    profile_migration::verify(&backup)?;
    assert_eq!(
        fs::read(source.join("config.toml"))?,
        fs::read(backup.join("config.toml"))?
    );
    let second = directory.join("next-native");
    profile_migration::import(
        &native,
        &second,
        &directory.join("native-backup"),
        "native fixture",
    )?;
    let (pins, warnings) = PinStore::load_from(&second)?.load_pins();
    assert!(warnings.is_empty());
    assert_eq!(pins[0].config.zoom_factor, 200);
    println!("Legacy shape -> native write -> legacy reader and second import passed. Fixtures and backups retained at {}", directory.display());
    Ok(())
}
