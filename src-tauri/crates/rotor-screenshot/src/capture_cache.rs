use image::RgbaImage;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone, Default)]
pub struct CaptureCache {
    images: Arc<Mutex<HashMap<String, Arc<RgbaImage>>>>,
}

impl CaptureCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&self) {
        self.lock_images().clear();
    }

    pub fn replace_all(&self, images: HashMap<String, RgbaImage>) {
        *self.lock_images() = images
            .into_iter()
            .map(|(label, image)| (label, Arc::new(image)))
            .collect();
    }

    pub fn get(&self, label: &str) -> Option<Arc<RgbaImage>> {
        self.lock_images().get(label).cloned()
    }

    fn lock_images(&self) -> MutexGuard<'_, HashMap<String, Arc<RgbaImage>>> {
        self.images
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
