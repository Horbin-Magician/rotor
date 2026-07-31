use rotor_translator::engine::{TranslateResult, TranslateStreamEvent};
use tauri::ipc::Channel;

#[tauri::command]
pub async fn translator_translate(
    text: String,
    on_event: Channel<TranslateStreamEvent>,
) -> Result<TranslateResult, String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Empty text".to_string());
    }

    rotor_translator::engine::translate(&text, |event| {
        let _ = on_event.send(event);
    })
    .await
    .map_err(|error| format!("Translate error: {error}"))
}
