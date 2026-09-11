//! Synthetic native configuration and pin persistence roundtrip.
//! Does not capture the desktop or open any windows.
use rotor_common::ConfigService;
use rotor_screenshot::pin_store::{source_crop, PinStore};
use std::{fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("usage: profile_roundtrip <new fixture directory>")?,
    );
    fs::create_dir(&directory)?;
    let directory = directory.canonicalize()?;
    let native = directory.join("native");
    fs::create_dir_all(native.join("shotter/default"))?;
    fs::write(native.join("config.toml"), "theme = '1'\nfuture_key = 'keep me'\nquick_actions = '[]'\nquick_actions_revision = '2'\ncurrent_workspace = '0'\n")?;
    fs::write(
        native.join("shotter/record.toml"),
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
mask_label = "ssmask-1"
minimized = false
"#,
    )?;
    let image = image::RgbaImage::from_fn(8, 8, |x, y| {
        image::Rgba([x as u8 * 20, y as u8 * 20, 90, 255])
    });
    image.save(native.join("shotter/default/7.png"))?;
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
    assert_eq!(
        image::open(native.join("shotter/default/7.png"))?.to_rgba8(),
        image
    );
    assert_eq!(
        fs::read_to_string(native.join("shotter/record.toml"))?,
        native_text
    );
    let (pins, warnings) = PinStore::load_from(&native)?.load_pins();
    assert!(warnings.is_empty());
    assert_eq!(pins[0].config.zoom_factor, 200);
    assert_eq!(pins[0].config.offset, (-1750, 60));
    println!(
        "Native config and pin roundtrip passed at {}",
        directory.display()
    );
    Ok(())
}
