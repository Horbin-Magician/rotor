use super::*;

impl Services {
    /// Test a draft independently of saved settings and active translations.
    pub fn test_ai_provider(&self, draft: Config) -> Result<OperationId, String> {
        self.ensure_running()?;
        let mut config = EngineConfig::from_config(&draft);
        config.engine = "ai".into();
        config.target_lang = "en".into();
        let id = next_operation();
        let mut task = lock(&self.ai_test);
        self.ensure_running()?;
        if let Some((_, previous)) = task.take() {
            previous.abort();
        }
        let events = self.events.clone();
        *task = Some((
            id,
            self.runtime().spawn(async move {
                // Exercise authentication, model access and the complete streaming
                // protocol with a small fixed input, never user text.
                let result = engine::translate_with_config(&config, "你好", |_| {})
                    .await
                    .map(|_| ())
                    .map_err(|error| config.redact_error(error.to_string()));
                let _ = events
                    .send(RuntimeEvent::AiProviderTested { id, result })
                    .await;
            }),
        ));
        Ok(id)
    }

    pub fn cancel_ai_provider_test(&self, id: Option<OperationId>) {
        let mut task = lock(&self.ai_test);
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
    fn invalid_draft_reports_its_identity_without_saving_settings() {
        let directory = tempfile::tempdir().unwrap();
        let config = ConfigService::load_from(directory.path()).unwrap();
        let (services, events) = Services::new(
            Arc::new(Mutex::new(config)),
            None,
            ServiceOptions { index_files: false },
        )
        .unwrap();
        let saved = services.settings();
        let draft = Config::from([
            ("ai_provider".into(), "openai".into()),
            ("ai_openai_api_key".into(), "fixture-secret".into()),
            ("ai_openai_model".into(), String::new()),
        ]);
        let id = services.test_ai_provider(draft).unwrap();
        let event = services.runtime().block_on(async {
            tokio::time::timeout(Duration::from_secs(5), events.recv())
                .await
                .unwrap()
                .unwrap()
        });
        match event {
            RuntimeEvent::AiProviderTested { id: actual, result } => {
                assert_eq!(actual, id);
                assert!(result.unwrap_err().contains("model ID"));
            }
            _ => panic!("expected provider test result"),
        }
        assert_eq!(services.settings(), saved);
        assert!(!directory.path().join("config.toml").exists());
        services.shutdown();
    }
}
