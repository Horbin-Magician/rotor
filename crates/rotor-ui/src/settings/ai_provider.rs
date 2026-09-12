use super::*;

#[derive(Default)]
pub(super) struct ConfigurationTest {
    pub pending: Option<OperationId>,
    draft: Option<Config>,
    result: Option<Result<(), String>>,
}

impl SettingsView {
    fn ai_draft(&self, cx: &App) -> Config {
        let mut draft: Config = self
            .config
            .iter()
            .filter(|(key, _)| key.starts_with("ai_"))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        for field in self
            .fields
            .iter()
            .filter(|field| field.section == Section::AiProvider)
        {
            draft.insert(field.key.into(), field.state.read(cx).value().to_string());
        }
        draft
    }

    pub(super) fn invalidate_ai_test(&mut self, cx: &App) {
        if self
            .ai_test
            .draft
            .as_ref()
            .is_some_and(|draft| *draft != self.ai_draft(cx))
        {
            if let Some(id) = self.ai_test.pending {
                self.services.cancel_ai_provider_test(Some(id));
            }
            self.ai_test = ConfigurationTest::default();
        }
    }

    pub(super) fn finish_ai_test(&mut self, id: OperationId, result: Result<(), String>, cx: &App) {
        self.invalidate_ai_test(cx);
        if self.ai_test.pending == Some(id) {
            self.ai_test.pending = None;
            self.ai_test.result = Some(result);
        }
    }

    pub(super) fn ai_test_controls(&self, cx: &mut Context<Self>) -> Div {
        let testing = self.ai_test.pending.is_some();
        let (zh, en) = if testing {
            ("测试中…", "Testing…")
        } else {
            match &self.ai_test.result {
                Some(Ok(())) => ("测试成功", "Test succeeded"),
                Some(Err(_)) => ("测试失败", "Test failed"),
                None => ("测试配置", "Test configuration"),
            }
        };
        let tooltip = match &self.ai_test.result {
            Some(Err(error)) => error.clone(),
            _ => self
                .t(
                    "发送简短请求，验证当前配置",
                    "Send a short request to verify the current configuration",
                )
                .to_owned(),
        };
        div().flex().flex_col().mt(px(10.)).child(
            Button::new("test-ai-provider")
                .label(self.t(zh, en))
                .when(
                    !testing && matches!(self.ai_test.result, Some(Ok(()))),
                    |button| button.success(),
                )
                .when(
                    !testing && matches!(self.ai_test.result, Some(Err(_))),
                    |button| button.danger(),
                )
                .disabled(testing || self.controls_locked())
                .tooltip(tooltip)
                .on_click(cx.listener(|this, _, _, cx| {
                    let draft = this.ai_draft(cx);
                    this.ai_test = ConfigurationTest {
                        draft: Some(draft.clone()),
                        ..Default::default()
                    };
                    match this.services.test_ai_provider(draft) {
                        Ok(id) => this.ai_test.pending = Some(id),
                        Err(error) => this.ai_test.result = Some(Err(error)),
                    }
                    cx.notify();
                })),
        )
    }
}

#[cfg(test)]
impl SettingsView {
    pub(super) fn check_ai_test_result_identity(&mut self, cx: &App) {
        let current = OperationId(101);
        self.ai_test = ConfigurationTest {
            pending: Some(current),
            draft: Some(self.ai_draft(cx)),
            result: None,
        };
        self.finish_ai_test(OperationId(100), Ok(()), cx);
        assert_eq!(self.ai_test.pending, Some(current));
        assert!(self.ai_test.result.is_none());
        self.finish_ai_test(current, Ok(()), cx);
        assert!(matches!(self.ai_test.result, Some(Ok(()))));
        self.config.insert("ai_provider".into(), "openai".into());
        self.invalidate_ai_test(cx);
        assert!(self.ai_test.result.is_none());
        self.finish_ai_test(current, Ok(()), cx);
        assert!(self.ai_test.result.is_none());
    }
}
