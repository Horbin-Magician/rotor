//! Native pin metadata shared by persistence and UI.
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShotterConfig {
    pub monitor_pos: (i32, i32),
    pub monitor_size: (u32, u32),
    pub rect: (u32, u32, u32, u32),
    pub image_rect: (u32, u32, u32, u32),
    pub offset: (i32, i32),
    pub zoom_factor: u32,
    pub mask_label: String,
    pub minimized: bool,
}
