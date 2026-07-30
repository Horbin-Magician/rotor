use image::{DynamicImage, RgbaImage};
use std::sync::Arc;
use std::time::Duration;

use crate::application::Application;

const IMAGE_RETRY_COUNT: usize = 20;
const IMAGE_RETRY_DELAY: Duration = Duration::from_millis(20);

fn get_screen_img(label: &str) -> Option<Arc<RgbaImage>> {
    Application::lock_global().screenshot.get_capture(label)
}

async fn get_pin_img(label: &str) -> Option<DynamicImage> {
    let id = label.trim_start_matches("sspin-");
    let parsed_id = id.parse::<u32>().ok()?;
    let image_load = {
        Application::lock_global()
            .screenshot
            .prepare_pin_img(parsed_id)
    };

    match tokio::task::spawn_blocking(move || image_load.load()).await {
        Ok(image) => image,
        Err(error) => {
            log::error!("Pin image load task failed for {label}: {error}");
            None
        }
    }
}

async fn retry_image<T, F>(mut load: F) -> Option<T>
where
    F: FnMut() -> Option<T>,
{
    for attempt in 0..IMAGE_RETRY_COUNT {
        if let Some(image) = load() {
            return Some(image);
        }

        if attempt + 1 < IMAGE_RETRY_COUNT {
            tokio::time::sleep(IMAGE_RETRY_DELAY).await;
        }
    }

    None
}

async fn try_get_screen_img(label: &str) -> Option<Arc<RgbaImage>> {
    retry_image(|| get_screen_img(label)).await
}

async fn try_get_pin_img(label: &str) -> Option<DynamicImage> {
    for attempt in 0..IMAGE_RETRY_COUNT {
        if let Some(image) = get_pin_img(label).await {
            return Some(image);
        }

        if attempt + 1 < IMAGE_RETRY_COUNT {
            tokio::time::sleep(IMAGE_RETRY_DELAY).await;
        }
    }

    None
}

pub enum ScreenshotImage {
    Mask(Arc<RgbaImage>),
    Pin(RgbaImage),
}

impl ScreenshotImage {
    pub fn bytes(&self) -> &[u8] {
        match self {
            ScreenshotImage::Mask(image) => image.as_raw(),
            ScreenshotImage::Pin(image) => image.as_raw(),
        }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            ScreenshotImage::Mask(image) => image.dimensions(),
            ScreenshotImage::Pin(image) => image.dimensions(),
        }
    }
}

// Resolves a mask/pin label to its image without cloning the raw bytes
pub async fn resolve_screenshot_image(label: &str) -> Result<ScreenshotImage, String> {
    if label.starts_with("ssmask-") {
        return try_get_screen_img(label)
            .await
            .map(ScreenshotImage::Mask)
            .ok_or_else(|| format!("No image data found for {label}"));
    }

    if label.starts_with("sspin-") {
        return try_get_pin_img(label)
            .await
            .map(|image| ScreenshotImage::Pin(image.to_rgba8()))
            .ok_or_else(|| format!("No image data found for {label}"));
    }

    Err(format!("Unsupported data label: {label}"))
}

pub async fn fetch_screenshot_data(label: &str) -> Result<Vec<u8>, String> {
    if label.starts_with("ssmask-") {
        return try_get_screen_img(label)
            .await
            .map(|image| image.as_raw().clone())
            .ok_or_else(|| format!("No image data found for {label}"));
    }

    if label.starts_with("sspin-") {
        return try_get_pin_img(label)
            .await
            .map(|image| image.to_rgba8().into_raw())
            .ok_or_else(|| format!("No image data found for {label}"));
    }

    Err(format!("Unsupported data label: {label}"))
}
