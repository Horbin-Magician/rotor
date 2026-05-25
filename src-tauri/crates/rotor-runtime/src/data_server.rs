use futures_util::{SinkExt, StreamExt};
use image::{DynamicImage, RgbaImage};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::application::Application;

const IMAGE_RETRY_COUNT: usize = 20;
const IMAGE_RETRY_DELAY: Duration = Duration::from_millis(20);

fn get_screen_img(label: &str) -> Option<Arc<RgbaImage>> {
    Application::global()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .screenshot
        .get_capture(label)
}

fn get_pin_img(label: &str) -> Option<DynamicImage> {
    let id = label.trim_start_matches("sspin-");
    let parsed_id = id.parse::<u32>().ok()?;

    Application::global()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .screenshot
        .get_pin_img(parsed_id)
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
    retry_image(|| get_pin_img(label)).await
}

async fn handle_data_request(label: String) -> Option<Vec<u8>> {
    if label.starts_with("ssmask-") {
        return Some(
            try_get_screen_img(&label)
                .await
                .map(|image| image.as_raw().clone())
                .unwrap_or_default(),
        );
    }

    if label.starts_with("sspin-") {
        return Some(
            try_get_pin_img(&label)
                .await
                .map(|image| image.to_rgba8().to_vec())
                .unwrap_or_default(),
        );
    }

    None
}

pub async fn run() {
    let mut listener = None;
    let mut bound_port = 10000;

    for port in 10000..=48137 {
        match TcpListener::bind(format!("localhost:{}", port)).await {
            Ok(l) => {
                bound_port = port;
                listener = Some(l);
                log::info!("Successfully bound data server to port {}", port);
                break;
            }
            Err(e) => {
                log::debug!("Failed to bind data server port {}: {}", port, e);
            }
        }
    }

    let listener = listener.expect("Failed to bind any data server port in range 10000-48137");

    {
        let mut rotor_app = Application::global()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        rotor_app.ws_port = bound_port;
    }

    while let Ok((stream, _)) = listener.accept().await {
        tauri::async_runtime::spawn(async move {
            let ws_stream = match accept_async(stream).await {
                Ok(stream) => stream,
                Err(e) => {
                    log::error!("Error during the websocket handshake occurred: {}", e);
                    return;
                }
            };

            let (mut write, mut read) = ws_stream.split();

            while let Some(msg) = read.next().await {
                match msg {
                    Ok(msg) => {
                        let label = msg.to_string();
                        if let Some(data) = handle_data_request(label).await {
                            if let Err(e) = write.send(Message::Binary(data.into())).await {
                                log::error!("Failed to send websocket data: {}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Error processing websocket message: {}", e);
                        break;
                    }
                }
            }
        });
    }
}
