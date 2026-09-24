use super::*;

impl SettingsView {
    pub(super) fn search_usage_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        appearance::group(self.t("使用记录", "Search usage"), cx)
            .child(
                appearance::caption(
                    self.t(
                        "成功打开的搜索结果会获得有限加分，并随时间衰减。清空后无法恢复，不影响文件和索引。",
                        "Successful result launches receive a limited ranking boost that fades over time. Clearing is permanent and leaves files and the index intact.",
                    ),
                    cx,
                )
                .pl(px(12.)),
            )
            .child(
                div().pl(px(12.)).child(
                    Button::new("clear-search-usage")
                        .label(self.t("清空使用记录", "Clear usage history"))
                        .disabled(self.usage_clear_request.is_some())
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.usage_clear_request.is_some() {
                                return;
                            }
                            match this.services.clear_search_usage() {
                                Ok(id) => this.usage_clear_request = Some(id),
                                Err(error) => this.message = error,
                            }
                            cx.notify();
                        })),
                ),
            )
    }
}
