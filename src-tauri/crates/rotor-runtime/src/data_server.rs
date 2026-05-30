use futures_util::{SinkExt, StreamExt};
use image::{DynamicImage, RgbaImage};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;

use crate::application::Application;

const IMAGE_RETRY_COUNT: usize = 20;
const IMAGE_RETRY_DELAY: Duration = Duration::from_millis(20);

#[derive(Debug)]
enum DataRequest {
    Correlated { request_id: u32, label: String },
    Legacy { label: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CorrelatedDataRequest {
    request_id: u32,
    label: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DataErrorResponse<'a> {
    request_id: u32,
    ok: bool,
    error: &'a str,
}

impl DataRequest {
    fn label(&self) -> &str {
        match self {
            DataRequest::Correlated { label, .. } | DataRequest::Legacy { label } => label,
        }
    }

    fn success_message(&self, data: Vec<u8>) -> Message {
        match self {
            DataRequest::Correlated { request_id, .. } => {
                let mut payload = Vec::with_capacity(std::mem::size_of::<u32>() + data.len());
                payload.extend_from_slice(&request_id.to_be_bytes());
                payload.extend_from_slice(&data);
                Message::Binary(payload.into())
            }
            DataRequest::Legacy { .. } => Message::Binary(data.into()),
        }
    }

    fn error_message(&self, error: &str) -> Message {
        match self {
            DataRequest::Correlated { request_id, .. } => {
                let response = DataErrorResponse {
                    request_id: *request_id,
                    ok: false,
                    error,
                };
                let message = serde_json::to_string(&response).unwrap_or_else(|serialize_error| {
                    format!(
                        r#"{{"requestId":{},"ok":false,"error":"Failed to serialize websocket error: {}"}}"#,
                        request_id, serialize_error
                    )
                });
                Message::Text(message.into())
            }
            DataRequest::Legacy { .. } => Message::Text(error.to_string().into()),
        }
    }
}

fn get_screen_img(label: &str) -> Option<Arc<RgbaImage>> {
    Application::lock_global().screenshot.get_capture(label)
}

fn get_pin_img(label: &str) -> Option<DynamicImage> {
    let id = label.trim_start_matches("sspin-");
    let parsed_id = id.parse::<u32>().ok()?;

    Application::lock_global().screenshot.get_pin_img(parsed_id)
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

fn parse_data_request(msg: Message) -> Result<Option<DataRequest>, String> {
    if msg.is_close() {
        return Ok(None);
    }

    if !msg.is_text() {
        return Err("Only text websocket data requests are supported".to_string());
    }

    let text = msg
        .into_text()
        .map_err(|err| format!("Failed to read websocket request text: {err}"))?
        .to_string();

    if let Ok(request) = serde_json::from_str::<CorrelatedDataRequest>(&text) {
        return Ok(Some(DataRequest::Correlated {
            request_id: request.request_id,
            label: request.label,
        }));
    }

    if text.starts_with("ssmask-") || text.starts_with("sspin-") {
        return Ok(Some(DataRequest::Legacy { label: text }));
    }

    Err("Invalid websocket data request".to_string())
}

async fn handle_data_request(label: &str) -> Result<Vec<u8>, String> {
    if label.starts_with("ssmask-") {
        return try_get_screen_img(label)
            .await
            .map(|image| image.as_raw().clone())
            .ok_or_else(|| format!("No image data found for {label}"));
    }

    if label.starts_with("sspin-") {
        return try_get_pin_img(label)
            .await
            .map(|image| image.to_rgba8().to_vec())
            .ok_or_else(|| format!("No image data found for {label}"));
    }

    Err(format!("Unsupported data label: {label}"))
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

    let Some(listener) = listener else {
        log::error!("Failed to bind any data server port in range 10000-48137");
        return;
    };

    {
        let mut rotor_app = Application::lock_global();
        rotor_app.ws_port = bound_port;
    }

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
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
                            Ok(msg) => match parse_data_request(msg) {
                                Ok(Some(request)) => {
                                    let message = match handle_data_request(request.label()).await {
                                        Ok(data) => request.success_message(data),
                                        Err(error) => request.error_message(&error),
                                    };

                                    if let Err(e) = write.send(message).await {
                                        log::error!("Failed to send websocket data: {}", e);
                                        break;
                                    }
                                }
                                Ok(None) => break,
                                Err(e) => {
                                    log::warn!("Invalid websocket data request: {}", e);
                                }
                            },
                            Err(e) => {
                                log::error!("Error processing websocket message: {}", e);
                                break;
                            }
                        }
                    }
                });
            }
            Err(e) => {
                log::error!("Failed to accept websocket connection: {}", e);
                break;
            }
        }
    }
}
