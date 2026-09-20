//! Runtime flags that survive a relaunch: elevation, update handoff, recovery
//! and autostart all forward exactly this set, so it is defined once.

/// Flags the desktop executable accepts and re-issues to itself.
pub const RUNTIME_FLAGS: [&str; 4] = [
    "--no-index",
    "--no-hotkeys",
    "--production-shortcuts",
    "--no-elevate",
];

pub fn is_runtime_flag(argument: &str) -> bool {
    RUNTIME_FLAGS.contains(&argument)
}

/// The runtime flags present in `arguments`, in their original order.
pub fn runtime_flags<'a>(arguments: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    arguments
        .into_iter()
        .filter(|argument| is_runtime_flag(argument))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_known_flags_are_forwarded() {
        let arguments: Vec<String> = [
            "rotor",
            "--no-hotkeys",
            "--data-dir",
            "--no-index",
            "/tmp/x",
            "--background",
            "--no-elevate",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(
            runtime_flags(&arguments),
            ["--no-hotkeys", "--no-index", "--no-elevate"].map(String::from)
        );
        assert!(!is_runtime_flag("--background"));
    }
}
