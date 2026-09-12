use super::TranslateStreamEvent;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;
const MAX_LINE_BYTES: usize = 1024 * 1024;
const MAX_TRANSLATION_BYTES: usize = 16 * 1024 * 1024;

#[derive(Default)]
pub(super) struct AiStream {
    pending: Vec<u8>,
    translated: String,
    done: bool,
    anthropic: bool,
}

impl AiStream {
    pub(super) fn new(anthropic: bool) -> Self {
        Self {
            anthropic,
            ..Self::default()
        }
    }

    pub(super) fn push<F>(&mut self, chunk: &[u8], on_event: &F) -> Result<bool>
    where
        F: Fn(TranslateStreamEvent) + Send + Sync,
    {
        // Process each line once; draining the front of a large network chunk
        // for every event repeatedly moved the entire remaining buffer.
        for part in chunk.split_inclusive(|byte| *byte == b'\n') {
            if self.done {
                break;
            }
            if part.len() > MAX_LINE_BYTES.saturating_sub(self.pending.len()) {
                return Err("AI stream event exceeds the 1 MiB limit".into());
            }
            self.pending.extend_from_slice(part);
            if part.last() == Some(&b'\n') {
                self.done = consume_stream_line(
                    &self.pending,
                    &mut self.translated,
                    on_event,
                    self.anthropic,
                )?;
                self.pending.clear();
            }
        }
        Ok(self.done)
    }

    pub(super) fn finish<F>(mut self, on_event: &F) -> Result<String>
    where
        F: Fn(TranslateStreamEvent) + Send + Sync,
    {
        if !self.done && !self.pending.is_empty() {
            self.done = consume_stream_line(
                &self.pending,
                &mut self.translated,
                on_event,
                self.anthropic,
            )?;
        }
        if !self.done {
            return Err(
                "AI translation stream ended before completion; translation is incomplete".into(),
            );
        }
        let translated = self.translated.trim().to_owned();
        if translated.is_empty() {
            return Err("Unexpected AI response format".into());
        }
        Ok(translated)
    }
}

#[cfg(test)]
pub(super) fn consume_openai_stream_line<F>(
    line: &[u8],
    translated: &mut String,
    on_event: &F,
) -> Result<bool>
where
    F: Fn(TranslateStreamEvent) + Send + Sync,
{
    consume_stream_line(line, translated, on_event, false)
}

fn consume_stream_line<F>(
    line: &[u8],
    translated: &mut String,
    on_event: &F,
    anthropic: bool,
) -> Result<bool>
where
    F: Fn(TranslateStreamEvent) + Send + Sync,
{
    let line = std::str::from_utf8(line)?.trim_end_matches(['\r', '\n']);
    let Some(data) = line.strip_prefix("data:").map(str::trim) else {
        return Ok(false);
    };
    if data.is_empty() {
        return Ok(false);
    }
    if !anthropic && data == "[DONE]" {
        return Ok(true);
    }
    let payload: serde_json::Value = serde_json::from_str(data)?;
    if let Some(message) = payload
        .pointer("/error/message")
        .and_then(|value| value.as_str())
    {
        return Err(format!("AI translation stream failed: {message}").into());
    }
    if anthropic {
        if payload["type"] == "message_stop" {
            return Ok(true);
        }
        if let Some(reason) = payload
            .pointer("/delta/stop_reason")
            .and_then(|value| value.as_str())
        {
            if reason != "end_turn" {
                return Err(format!("AI translation did not complete: {reason}").into());
            }
        }
    }
    if let Some(reason) = payload
        .pointer("/choices/0/finish_reason")
        .and_then(|value| value.as_str())
    {
        if reason != "stop" {
            return Err(format!("AI translation did not complete: {reason}").into());
        }
    }
    let content = if anthropic {
        if payload["type"] == "content_block_delta" && payload["delta"]["type"] == "text_delta" {
            payload.pointer("/delta/text")
        } else {
            None
        }
    } else {
        payload.pointer("/choices/0/delta/content")
    };
    if let Some(content) = content
        .and_then(|value| value.as_str())
        .filter(|text| !text.is_empty())
    {
        if content.len() > MAX_TRANSLATION_BYTES.saturating_sub(translated.len()) {
            return Err("AI translation exceeds the 16 MiB limit".into());
        }
        translated.push_str(content);
        on_event(TranslateStreamEvent::Delta {
            content: content.into(),
        });
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    const DELTA: &str = "data: {\"choices\":[{\"delta\":{\"content\":\"你好🦀\"}}]}\r\n";

    #[test]
    fn every_utf8_chunk_boundary_preserves_exactly_once_deltas() {
        let wire = format!(": heartbeat\r\n{DELTA}data: {{\"choices\":[{{\"delta\":{{}},\"finish_reason\":\"stop\"}}]}}\n\ndata: [DONE]");
        for split in 0..=wire.len() {
            let events = Mutex::new(Vec::new());
            let sink = |event| events.lock().unwrap().push(event);
            let mut stream = AiStream::default();
            stream.push(&wire.as_bytes()[..split], &sink).unwrap();
            stream.push(&wire.as_bytes()[split..], &sink).unwrap();
            assert_eq!(stream.finish(&sink).unwrap(), "你好🦀");
            assert!(
                matches!(events.lock().unwrap().as_slice(), [TranslateStreamEvent::Delta { content }] if content == "你好🦀")
            );
        }
    }

    #[test]
    fn early_eof_never_publishes_a_successful_partial_translation() {
        for tail in ["", "data: {\"choices\":[{\"finish_reason\":\"stop\"}]}\n"] {
            let mut stream = AiStream::default();
            stream
                .push(format!("{DELTA}{tail}").as_bytes(), &|_| {})
                .unwrap();
            assert!(stream
                .finish(&|_| {})
                .unwrap_err()
                .to_string()
                .contains("incomplete"));
        }
    }

    #[test]
    fn abnormal_finish_and_provider_error_are_not_success() {
        for reason in [
            "length",
            "content_filter",
            "tool_calls",
            "insufficient_system_resource",
            "unknown",
        ] {
            let mut stream = AiStream::default();
            stream.push(DELTA.as_bytes(), &|_| {}).unwrap();
            let error = stream.push(format!("data: {{\"choices\":[{{\"finish_reason\":\"{reason}\"}}]}}\ndata: [DONE]\n").as_bytes(), &|_| {}).unwrap_err();
            assert!(error.to_string().contains(reason));
        }
        let mut stream = AiStream::default();
        assert!(stream
            .push(
                b"data: {\"error\":{\"message\":\"fixture failure\"}}\n",
                &|_| {}
            )
            .unwrap_err()
            .to_string()
            .contains("fixture failure"));
    }

    #[test]
    fn completed_stream_ignores_trailing_data_and_rejects_empty_output() {
        let mut stream = AiStream::default();
        stream
            .push(
                format!("{DELTA}data: [DONE]\ngarbage\n{DELTA}").as_bytes(),
                &|_| {},
            )
            .unwrap();
        assert_eq!(stream.finish(&|_| {}).unwrap(), "你好🦀");
        let mut empty = AiStream::default();
        empty.push(b"data: [DONE]\n", &|_| {}).unwrap();
        assert!(empty.finish(&|_| {}).is_err());
    }

    #[test]
    fn oversized_events_and_outputs_fail_before_growth_or_callback() {
        let mut stream = AiStream::default();
        assert!(stream
            .push(&vec![b'x'; MAX_LINE_BYTES + 1], &|_| panic!(
                "unexpected callback"
            ))
            .is_err());
        assert!(stream.pending.is_empty());
        let mut output = "x".repeat(MAX_TRANSLATION_BYTES);
        assert!(
            consume_openai_stream_line(DELTA.as_bytes(), &mut output, &|_| panic!(
                "unexpected callback"
            ))
            .is_err()
        );
        assert_eq!(output.len(), MAX_TRANSLATION_BYTES);
    }
}

#[cfg(test)]
mod anthropic_tests {
    use super::*;
    const DELTA: &str = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"你好🦀\"}}\n\n";
    const END: &str = "data: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\"}}\n\ndata: {\"type\":\"message_stop\"}\n\n";

    #[test]
    fn anthropic_handles_fragmented_utf8_and_requires_completion() {
        let wire = format!("{DELTA}{END}");
        for split in 0..=wire.len() {
            let mut stream = AiStream::new(true);
            stream.push(&wire.as_bytes()[..split], &|_| {}).unwrap();
            stream.push(&wire.as_bytes()[split..], &|_| {}).unwrap();
            assert_eq!(stream.finish(&|_| {}).unwrap(), "你好🦀");
        }
        let mut stream = AiStream::new(true);
        stream.push(DELTA.as_bytes(), &|_| {}).unwrap();
        assert!(stream
            .finish(&|_| {})
            .unwrap_err()
            .to_string()
            .contains("incomplete"));
    }

    #[test]
    fn anthropic_rejects_truncation_refusal_and_errors() {
        for reason in ["max_tokens", "tool_use", "refusal", "pause_turn"] {
            let mut stream = AiStream::new(true);
            stream.push(DELTA.as_bytes(), &|_| {}).unwrap();
            let wire = format!("data: {{\"type\":\"message_delta\",\"delta\":{{\"stop_reason\":\"{reason}\"}}}}\n{END}");
            assert!(stream
                .push(wire.as_bytes(), &|_| {})
                .unwrap_err()
                .to_string()
                .contains(reason));
        }
        let mut stream = AiStream::new(true);
        assert!(stream
            .push(
                b"data: {\"type\":\"error\",\"error\":{\"message\":\"overloaded\"}}\n",
                &|_| {}
            )
            .is_err());
    }
}
