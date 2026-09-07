pub fn recoverable_session_id(
    ready_session_id: u32,
    current_session_id: u32,
    has_capture: bool,
) -> Option<u32> {
    (ready_session_id != 0 && ready_session_id == current_session_id && has_capture)
        .then_some(ready_session_id)
}

#[cfg(test)]
mod tests {
    use super::recoverable_session_id;

    #[test]
    fn only_ready_current_sessions_with_capture_are_recoverable() {
        assert_eq!(recoverable_session_id(0, 0, true), None);
        assert_eq!(recoverable_session_id(4, 5, true), None);
        assert_eq!(recoverable_session_id(5, 5, false), None);
        assert_eq!(recoverable_session_id(5, 5, true), Some(5));
    }
}
