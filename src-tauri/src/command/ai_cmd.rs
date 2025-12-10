//! AI Commands for Tauri frontend
//!
//! This module provides Tauri commands for AI chat functionality
//! using Volcano Engine (ByteDance) DeepSeek V3.2 API.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::core::config::AppConfig;
use crate::module::ai::{AiChat, ChatMessage, MessageRole, StreamEvent};

/// Chat message structure for frontend communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendChatMessage {
    /// Role of the message sender: "system", "user", or "assistant"
    pub role: String,
    /// Content of the message
    pub content: String,
}

impl From<FrontendChatMessage> for ChatMessage {
    fn from(msg: FrontendChatMessage) -> Self {
        let role = match msg.role.to_lowercase().as_str() {
            "system" => MessageRole::System,
            "assistant" => MessageRole::Assistant,
            _ => MessageRole::User,
        };
        ChatMessage {
            role,
            content: msg.content,
        }
    }
}

/// Helper function to get AI client from config
fn get_ai_client() -> Result<AiChat, String> {
    let api_key = {
        let config = AppConfig::global().lock().unwrap();
        config.get(&"ai_api_key".to_string()).cloned().unwrap_or_default()
    };

    if api_key.is_empty() {
        return Err("API key not configured. Please set ai_api_key in settings.".to_string());
    }

    let model = {
        let config = AppConfig::global().lock().unwrap();
        config.get(&"ai_model".to_string()).cloned()
    };

    let client = if let Some(model_id) = model {
        if !model_id.is_empty() {
            AiChat::with_model(&api_key, &model_id)
        } else {
            AiChat::new(&api_key)
        }
    } else {
        AiChat::new(&api_key)
    };

    Ok(client)
}

/// Streaming AI chat command
///
/// Send a conversation history to the AI and stream the response.
/// Response chunks are emitted as 'ai-stream' events to the frontend.
///
/// # Arguments
/// * `app` - Tauri app handle for emitting events
/// * `messages` - The conversation history as a list of messages
///
/// # Returns
/// * `Result<(), String>` - Ok if streaming started successfully
#[tauri::command]
pub async fn ai_chat_stream(
    app: AppHandle,
    messages: Vec<FrontendChatMessage>,
) -> Result<(), String> {
    let client = get_ai_client()?;

    // Convert frontend messages to internal format
    let chat_messages: Vec<ChatMessage> = messages.into_iter().map(|m| m.into()).collect();

    let app_clone = app.clone();
    
    tokio::spawn(async move {
        let result = client.chat_stream(chat_messages, |event: StreamEvent| {
            // Emit event to frontend
            if let Err(e) = app_clone.emit("ai-stream", &event) {
                log::error!("Failed to emit ai-stream event: {}", e);
            }
        }).await;

        if let Err(e) = result {
            log::error!("AI stream error: {}", e);
            let _ = app_clone.emit("ai-stream", &StreamEvent::Error {
                message: e.to_string(),
            });
        }
    });

    Ok(())
}
