use super::*;
use rotor_common::{ai_provider::AiProviderConfig, Config};

fn provider(name: &str) -> AiProviderConfig {
    AiProviderConfig::from_config(&Config::from([
        ("ai_provider".into(), name.into()),
        (format!("ai_{name}_api_key"), "fixture-secret".into()),
        (format!("ai_{name}_model"), "fixture-model".into()),
    ]))
}

#[test]
fn provider_requests_use_correct_protocol_and_endpoint() {
    for (name, endpoint) in [
        ("deepseek", "https://api.deepseek.com/chat/completions"),
        ("openai", "https://api.openai.com/v1/chat/completions"),
        ("anthropic", "https://api.anthropic.com/v1/messages"),
    ] {
        let ai = provider(name);
        let (url, body) = ai_request(&ai, "translate me", "ja").unwrap();
        assert_eq!(url.as_str(), endpoint);
        assert_eq!(body["model"], "fixture-model");
        assert_eq!(body["stream"], true);
        if name == "anthropic" {
            assert!(body["system"].as_str().unwrap().contains("Japanese"));
            assert_eq!(body["messages"][0]["role"], "user");
            assert_eq!(body["max_tokens"], 4096);
        } else {
            assert_eq!(body["messages"][0]["role"], "system");
            assert_eq!(body["messages"][1]["content"], "translate me");
        }
        assert_eq!(body.get("thinking").is_some(), name == "deepseek");
        if name == "openai" {
            assert_eq!(body["max_completion_tokens"], 4096);
            assert!(body.get("max_tokens").is_none());
        }
    }
}

#[test]
fn custom_endpoints_and_configuration_validation() {
    let mut ai = provider("custom");
    ai.api_key.clear();
    for (protocol, path) in [("openai", "chat/completions"), ("anthropic", "messages")] {
        ai.protocol = protocol.into();
        for base in [
            "http://localhost:1234/v1/".to_owned(),
            format!("http://localhost:1234/v1/{path}"),
        ] {
            ai.base_url = base;
            let (url, _) = ai_request(&ai, "hello", "en").unwrap();
            assert_eq!(url.as_str(), format!("http://localhost:1234/v1/{path}"));
        }
    }
    for base in [
        "",
        "invalid",
        "file:///tmp/file",
        "https://user:secret@example.com",
        "https://example.com?key=secret",
    ] {
        ai.base_url = base.into();
        assert!(ai_request(&ai, "hello", "en").is_err());
    }
    ai.base_url = "https://example.com/v1".into();
    for tokens in ["0", "-1", "invalid", "4294967296"] {
        ai.max_tokens = tokens.into();
        assert!(ai_request(&ai, "hello", "en").is_err());
    }
    ai.max_tokens = "4096".into();
    ai.model.clear();
    assert!(ai_request(&ai, "hello", "en").is_err());
}

#[tokio::test]
async fn ai_http_roundtrip_uses_provider_auth_and_streaming() {
    use std::io::{Read, Write};
    for name in ["deepseek", "openai", "anthropic", "custom"] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let mut ai = provider(name);
        ai.base_url = format!("http://{}/v1", listener.local_addr().unwrap());
        let anthropic = name == "anthropic";
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            let mut socket = loop {
                match listener.accept() {
                    Ok((socket, _)) => break socket,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("fixture accept failed: {error}"),
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let (headers, body) = loop {
                let mut buffer = [0; 4096];
                let count = socket.read(&mut buffer).unwrap();
                assert!(count > 0 && request.len() < 65536);
                request.extend_from_slice(&buffer[..count]);
                if let Some(end) = request.windows(4).position(|part| part == b"\r\n\r\n") {
                    let headers = String::from_utf8(request[..end].to_vec())
                        .unwrap()
                        .to_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if request.len() >= end + 4 + length {
                        break (
                            headers,
                            serde_json::from_slice::<serde_json::Value>(
                                &request[end + 4..end + 4 + length],
                            )
                            .unwrap(),
                        );
                    }
                }
            };
            assert_eq!(body["stream"], true);
            assert_eq!(body["model"], "fixture-model");
            let response = if anthropic {
                assert!(headers.starts_with("post /v1/messages "));
                assert!(headers.contains("x-api-key: fixture-secret"));
                assert!(headers.contains("anthropic-version: 2023-06-01"));
                assert!(!headers.contains("authorization:"));
                "data: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"你好\"}}\n\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\ndata: {\"type\":\"message_stop\"}\n\n"
            } else {
                assert!(headers.starts_with("post /v1/chat/completions "));
                assert!(headers.contains("authorization: bearer fixture-secret"));
                assert!(!headers.contains("x-api-key:"));
                "data: {\"choices\":[{\"delta\":{\"content\":\"你好\"},\"finish_reason\":\"stop\"}]}\n\ndata: [DONE]\n\n"
            };
            write!(socket, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}", response.len()).unwrap();
        });
        let mut engine = EngineConfig::from_config(&Config::new());
        engine.engine = "ai".into();
        engine.ai = ai;
        let events = std::sync::Mutex::new(Vec::new());
        let result =
            translate_with_config(&engine, "hello", |event| events.lock().unwrap().push(event))
                .await
                .unwrap();
        assert_eq!(result.translated, "你好");
        assert!(
            matches!(events.lock().unwrap().as_slice(), [TranslateStreamEvent::Started { .. }, TranslateStreamEvent::Delta { content }] if content == "你好")
        );
        assert_eq!(
            engine.redact_error("invalid fixture-secret".into()),
            "invalid [redacted]"
        );
        server.join().unwrap();
    }
}

#[test]
fn chat_requests_preserve_turns_without_translation_instructions() {
    let messages = vec![
        ChatMessage {
            assistant: false,
            content: "Explain Rust".into(),
        },
        ChatMessage {
            assistant: true,
            content: "Rust is a programming language.".into(),
        },
        ChatMessage {
            assistant: false,
            content: "Show an example".into(),
        },
    ];
    for name in ["deepseek", "openai", "anthropic", "custom"] {
        let mut ai = provider(name);
        if name == "custom" {
            ai.base_url = "http://localhost:1234/v1".into();
        }
        let (_, body) = chat_request(&ai, &messages).unwrap();
        let turns = body["messages"].as_array().unwrap();
        let offset = usize::from(name != "anthropic");
        assert_eq!(turns.len(), 3 + offset);
        assert_eq!(turns[offset]["content"], "Explain Rust");
        assert_eq!(turns[offset + 1]["role"], "assistant");
        assert_eq!(turns[offset + 2]["content"], "Show an example");
        let system = if name == "anthropic" {
            &body["system"]
        } else {
            &turns[0]["content"]
        };
        assert!(system.as_str().unwrap().contains("helpful assistant"));
        assert!(!system.as_str().unwrap().contains("translation engine"));
    }
}
