use std::error::Error;

pub(super) fn open_file(file_path: &str) -> Result<(), Box<dyn Error>> {
    std::process::Command::new("open").arg(file_path).spawn()?;
    Ok(())
}

pub(super) fn open_file_as_admin(file_path: &str) -> Result<(), Box<dyn Error>> {
    log::info!("macOS does not support elevated open; opening normally instead");
    open_file(file_path)
}
