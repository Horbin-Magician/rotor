use super::*;

impl Services {
    pub fn chat(&self, messages: Vec<ChatMessage>) -> Result<OperationId, String> {
        self.ensure_running()?;
        if messages
            .last()
            .is_none_or(|message| message.assistant || message.content.trim().is_empty())
        {
            return Err("Enter a message first".into());
        }
        let config = EngineConfig::from_config(&self.settings());
        let events = self.events.clone();
        let runtime = self.runtime();
        self.chat.begin(
            || self.ensure_running(),
            move |id, _| {
                runtime.spawn(async move {
                    let relay = ProgressRelay::start(
                        events.clone(),
                        || true,
                        move |event| RuntimeEvent::Chat { id, event },
                    );
                    let result = engine::chat_with_config(&config, &messages, relay.callback())
                        .await
                        .map_err(|error| config.redact_error(error.to_string()));
                    relay.finish().await;
                    let _ = events.send(RuntimeEvent::ChatFinished { id, result }).await;
                })
            },
        )
    }

    pub fn cancel_chat(&self, id: Option<OperationId>) {
        self.chat.cancel(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_reports_configuration_errors_with_identity_without_saving() {
        let directory = tempfile::tempdir().unwrap();
        let config = ConfigService::load_from(directory.path()).unwrap();
        let (services, events) = Services::new(
            Arc::new(Mutex::new(config)),
            None,
            ServiceOptions { index_files: false },
        )
        .unwrap();
        let saved = services.settings();
        assert!(services.chat(vec![]).is_err());
        let id = services
            .chat(vec![ChatMessage {
                assistant: false,
                content: "Hello".into(),
            }])
            .unwrap();
        // A closing, older window cannot cancel this request.
        services.cancel_chat(Some(OperationId(id.0 + 1)));
        let event = services.runtime().block_on(async {
            tokio::time::timeout(Duration::from_secs(5), events.recv())
                .await
                .unwrap()
                .unwrap()
        });
        match event {
            RuntimeEvent::ChatFinished { id: actual, result } => {
                assert_eq!(id, actual);
                assert!(result.is_err());
            }
            _ => panic!("expected chat result"),
        }
        assert_eq!(services.settings(), saved);
        assert!(!directory.path().join("config.toml").exists());
        services.shutdown();
    }
}
