//! Read-only validation before a native profile acceptance run.
use rotor_screenshot::pin_store::{source_crop, PinStore};
use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let directory = PathBuf::from(arguments.next().ok_or("expected a profile directory")?);
    if arguments.next().is_some() {
        return Err("expected exactly one profile directory".into());
    }
    let directory = if directory.is_absolute() {
        directory
    } else {
        std::env::current_dir()?.join(directory)
    };
    // Refuse a missing record instead of reporting a nonexistent profile as valid.
    if !directory.join("pins/record.toml").is_file() {
        return Err("profile has no pins/record.toml".into());
    }
    let store = PinStore::load_from(&directory).map_err(std::io::Error::other)?;
    let (pins, warnings) = store.load_pins();
    println!("Valid pins: {}; warnings: {}", pins.len(), warnings.len());
    for pin in pins {
        let crop = source_crop(&pin.config, pin.image.width(), pin.image.height())
            .map_err(std::io::Error::other)?;
        println!(
            "Pin {}: PNG {}x{}, crop {:?}, zoom {}%",
            pin.id,
            pin.image.width(),
            pin.image.height(),
            crop,
            pin.config.zoom_factor
        );
    }
    for warning in &warnings {
        eprintln!("{warning}");
    }
    if !warnings.is_empty() {
        return Err("profile contains invalid pins; nothing was modified".into());
    }
    Ok(())
}
