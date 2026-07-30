use rotor_translator::engine::TranslateResult;

#[tauri::command]
pub async fn translator_translate(text: String) -> Result<TranslateResult, String> {
    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Empty text".to_string());
    }

    rotor_translator::engine::translate(&text)
        .await
        .map_err(|error| format!("Translate error: {error}"))
}
