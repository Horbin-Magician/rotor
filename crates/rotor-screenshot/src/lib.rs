pub mod capture_cache;
pub mod img_util;
pub mod monitor;
pub mod shotter_record;

pub mod pin_store;
pub mod session;

pub fn capture_images() -> Result<std::collections::HashMap<String, image::RgbaImage>, String> {
    monitor::capture_all(xcap::Monitor::all().map_err(|error| error.to_string())?)
}
