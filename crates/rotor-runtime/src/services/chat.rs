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
        let id = next_operation();
        let mut task = lock(&self.chat);
        self.ensure_running()?;
        if let Some((_, previous)) = task.take() {
            previous.abort();
        }
        let events = self.events.clone();
        *task = Some((
            id,
            self.runtime().spawn(async move {
                let progress = events.clone();
                let result = engine::chat_with_config(&config, &messages, move |event| {
                    let _ = progress.send_blocking(RuntimeEvent::Chat { id, event });
                })
                .await
                .map_err(|error| config.redact_error(error.to_string()));
                let _ = events.send(RuntimeEvent::ChatFinished { id, result }).await;
            }),
        ));
        Ok(id)
    }

    pub fn cancel_chat(&self, id: Option<OperationId>) {
        let mut task = lock(&self.chat);
        if task
            .as_ref()
            .is_some_and(|(current, _)| id.is_none_or(|id| id == *current))
        {
            if let Some((_, task)) = task.take() {
                task.abort();
            }
        }
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
