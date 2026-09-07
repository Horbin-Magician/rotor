use gpui_kit::Keystroke;
use std::str::FromStr;

pub(crate) fn recorded_key(event: &Keystroke, local: bool) -> Result<Option<String>, String> {
    let key = event.key.to_ascii_lowercase();
    if [
        "control", "ctrl", "alt", "shift", "cmd", "super", "win", "fn",
    ]
    .contains(&key.as_str())
    {
        return Ok(None);
    }
    let function_key = key
        .strip_prefix('f')
        .and_then(|key| key.parse::<u8>().ok())
        .is_some_and(|key| (1..=24).contains(&key));
    if event.modifiers.function {
        return Err("Fn combinations cannot be registered globally".into());
    }
    if !local
        && !event.modifiers.control
        && !event.modifiers.platform
        && !event.modifiers.alt
        && !function_key
    {
        return Err("Global shortcuts need Ctrl, Alt, Cmd or a function key".into());
    }
    let name = match key.as_str() {
        "left" => "ArrowLeft",
        "right" => "ArrowRight",
        "up" => "ArrowUp",
        "down" => "ArrowDown",
        " " | "space" => "Space",
        "-" => "Minus",
        "=" | "+" => "Equal",
        "[" => "BracketLeft",
        "]" => "BracketRight",
        ";" => "Semicolon",
        "'" => "Quote",
        "," => "Comma",
        "." => "Period",
        "/" => "Slash",
        "\\" => "Backslash",
        "\u{0060}" => "Backquote",
        _ => key.as_str(),
    };
    let mut parts = Vec::new();
    if event.modifiers.control {
        parts.push("Ctrl");
    }
    if event.modifiers.alt {
        parts.push("Alt");
    }
    if event.modifiers.shift {
        parts.push("Shift");
    }
    if event.modifiers.platform {
        parts.push("Super");
    }
    parts.push(name);
    global_hotkey::hotkey::HotKey::from_str(&parts.join("+"))
        .map(|key| Some(key.to_string()))
        .map_err(|error| error.to_string())
}
