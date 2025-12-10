//! AI Module for Volcano Engine (ByteDance) DeepSeek V3.2 API
//! 
//! This module provides functionality to interact with Volcano Engine's LLM API
//! for AI-powered chat completions using DeepSeek V3.2 model.

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::error::Error;

/// Volcano Engine API endpoint for chat completions
const VOLCANO_API_ENDPOINT: &str = "https://ark.cn-beijing.volces.com/api/v3/chat/completions";

/// Default model ID for DeepSeek V3.2 on Volcano Engine
const DEFAULT_MODEL: &str = "deepseek-v3-241226";

/// Message role types for chat API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// Chat message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

impl ChatMessage {
    /// Create a new system message
    #[allow(dead_code)]
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
        }
    }

    /// Create a new user message
    #[allow(dead_code)]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
        }
    }

    /// Create a new assistant message
    #[allow(dead_code)]
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
        }
    }
}

/// Request body for Volcano Engine chat API
#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

/// Choice in the API response
#[derive(Debug, Deserialize)]
pub struct ChatChoice {
    #[allow(dead_code)]
    pub index: u32,
    #[allow(dead_code)]
    pub message: ChatMessage,
    #[allow(dead_code)]
    pub finish_reason: Option<String>,
}

/// Delta content in streaming response
#[derive(Debug, Deserialize)]
pub struct ChatDelta {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    pub role: Option<String>,
}

/// Streaming choice in the API response
#[derive(Debug, Deserialize)]
pub struct StreamChoice {
    #[allow(dead_code)]
    pub index: u32,
    pub delta: ChatDelta,
    pub finish_reason: Option<String>,
}

/// Usage statistics in the API response
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ChatUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Response from Volcano Engine chat API
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: ChatUsage,
}

/// Streaming response chunk from Volcano Engine chat API
#[derive(Debug, Deserialize)]
pub struct StreamChunk {
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    pub object: String,
    #[allow(dead_code)]
    pub created: u64,
    #[allow(dead_code)]
    pub model: String,
    pub choices: Vec<StreamChoice>,
}

/// Error response from Volcano Engine API
#[derive(Debug, Deserialize)]
pub struct ApiError {
    pub error: ApiErrorDetail,
}

#[derive(Debug, Deserialize)]
pub struct ApiErrorDetail {
    pub message: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    pub error_type: Option<String>,
    #[allow(dead_code)]
    pub code: Option<String>,
}

/// Stream event types for frontend
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum StreamEvent {
    /// New content chunk
    Content { content: String },
    /// Stream finished
    Done,
    /// Error occurred
    Error { message: String },
}

/// AI Chat client for Volcano Engine
pub struct AiChat {
    api_key: String,
    model: String,
    client: reqwest::Client,
}

impl AiChat {
    /// Create a new AI chat client with the given API key
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: DEFAULT_MODEL.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Create a new AI chat client with custom model
    pub fn with_model(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Send a streaming chat completion request to Volcano Engine API
    /// 
    /// # Arguments
    /// * `messages` - The conversation history
    /// * `callback` - Callback function to handle each stream event
    /// 
    /// # Returns
    /// * `Ok(String)` - The complete response text
    /// * `Err(Box<dyn Error>)` - Error if the request fails
    pub async fn chat_stream<F>(
        &self,
        messages: Vec<ChatMessage>,
        mut callback: F,
    ) -> Result<String, Box<dyn Error + Send + Sync>>
    where
        F: FnMut(StreamEvent) + Send,
    {
        let request_body = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            temperature: None,
            max_tokens: None,
            stream: Some(true),
        };

        let response = self
            .client
            .post(VOLCANO_API_ENDPOINT)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_body)
            .send()
            .await?;

        let status = response.status();
        
        if !status.is_success() {
            let error_text = response.text().await?;
            if let Ok(api_error) = serde_json::from_str::<ApiError>(&error_text) {
                let err_msg = format!("API Error ({}): {}", status, api_error.error.message);
                callback(StreamEvent::Error { message: err_msg.clone() });
                return Err(err_msg.into());
            }
            let err_msg = format!("HTTP Error {}: {}", status, error_text);
            callback(StreamEvent::Error { message: err_msg.clone() });
            return Err(err_msg.into());
        }

        let mut full_content = String::new();
        let mut stream = response.bytes_stream();

        let mut buffer = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            let chunk_str = String::from_utf8_lossy(&chunk);
            buffer.push_str(&chunk_str);

            // Process complete SSE lines
            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                // SSE format: "data: {...}"
                if let Some(data) = line.strip_prefix("data: ") {
                    if data.trim() == "[DONE]" {
                        callback(StreamEvent::Done);
                        return Ok(full_content);
                    }

                    match serde_json::from_str::<StreamChunk>(data) {
                        Ok(chunk) => {
                            for choice in chunk.choices {
                                if let Some(content) = choice.delta.content {
                                    if !content.is_empty() {
                                        full_content.push_str(&content);
                                        callback(StreamEvent::Content { content });
                                    }
                                }
                                if choice.finish_reason.is_some() {
                                    callback(StreamEvent::Done);
                                    return Ok(full_content);
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to parse stream chunk: {} - data: {}", e, data);
                        }
                    }
                }
            }
        }

        callback(StreamEvent::Done);
        Ok(full_content)
    }
}
