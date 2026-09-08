use rotor_runtime::OperationId;
use std::collections::HashMap;

struct Submission {
    id: OperationId,
    revision: u64,
    value: String,
}
struct Edit {
    observed: String,
    revision: u64,
    composing: bool,
    pending: Option<Submission>,
    failed: bool,
}

pub(super) struct FieldSaves {
    fields: HashMap<&'static str, Edit>,
}
pub(super) struct SavedField {
    pub key: &'static str,
    pub submitted: String,
}
pub(super) struct Receipt {
    pub current: bool,
    pub saved: Vec<SavedField>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CloseState {
    Waiting,
    Failed,
    Ready,
}

impl FieldSaves {
    pub fn new(values: impl IntoIterator<Item = (&'static str, String)>) -> Self {
        Self {
            fields: values
                .into_iter()
                .map(|(key, value)| {
                    (
                        key,
                        Edit {
                            observed: value,
                            revision: 0,
                            composing: false,
                            pending: None,
                            failed: false,
                        },
                    )
                })
                .collect(),
        }
    }
    pub fn observe(
        &mut self,
        key: &'static str,
        value: &str,
        composing: bool,
        focused: bool,
        force: bool,
    ) -> bool {
        let Some(edit) = self.fields.get_mut(key) else {
            return false;
        };
        edit.composing = composing;
        // Shortcut text is a complete chord on Enter/blur, while the recorder
        // supplies a complete chord directly. Never persist an IME preedit.
        if composing || (!force && focused && key.starts_with("shortcut_")) {
            return false;
        }
        if edit.observed == value && !(force && edit.failed) {
            return false;
        }
        edit.observed = value.into();
        edit.revision += 1;
        true
    }
    pub fn accepted(&mut self, key: &str, value: &str, id: OperationId) {
        if let Some(edit) = self.fields.get_mut(key) {
            if edit.observed != value {
                edit.observed = value.into();
                edit.revision += 1;
            }
            edit.pending = Some(Submission {
                id,
                revision: edit.revision,
                value: value.into(),
            });
            edit.failed = false;
        }
    }
    pub fn rejected(&mut self, key: &str) {
        if let Some(edit) = self.fields.get_mut(key) {
            edit.failed = true;
        }
    }
    pub fn rejected_value(&mut self, key: &str, value: &str) {
        if let Some(edit) = self.fields.get_mut(key) {
            if edit.observed != value {
                edit.observed = value.into();
                edit.revision += 1;
            }
            edit.failed = true;
        }
    }
    pub fn finish(&mut self, id: OperationId, success: bool) -> Receipt {
        let mut receipt = Receipt {
            current: false,
            saved: Vec::new(),
        };
        for (&key, edit) in &mut self.fields {
            if edit
                .pending
                .as_ref()
                .is_some_and(|pending| pending.id == id)
            {
                let pending = edit.pending.take().unwrap();
                if pending.revision == edit.revision {
                    receipt.current = true;
                    edit.failed = !success;
                    if success {
                        receipt.saved.push(SavedField {
                            key,
                            submitted: pending.value,
                        });
                    }
                }
            }
        }
        receipt
    }
    pub fn normalized(&mut self, key: &str, value: String) {
        if let Some(edit) = self.fields.get_mut(key) {
            edit.observed = value;
        }
    }
    pub fn has_pending(&self) -> bool {
        self.fields.values().any(|field| field.pending.is_some())
    }
    pub fn close_state(&self, manual_pending: bool, manual_failed: bool) -> CloseState {
        if manual_pending || self.has_pending() {
            CloseState::Waiting
        } else if manual_failed || self.has_failures() {
            CloseState::Failed
        } else {
            CloseState::Ready
        }
    }
    pub fn has_failures(&self) -> bool {
        self.fields.values().any(|field| field.failed)
    }
    pub fn has_composition(&self) -> bool {
        self.fields.values().any(|field| field.composing)
    }
    pub fn failed(&self, key: &str) -> bool {
        self.fields.get(key).is_some_and(|field| field.failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fields() -> FieldSaves {
        FieldSaves::new([
            ("text", "old".into()),
            ("other", String::new()),
            ("shortcut_search", "Ctrl+F".into()),
        ])
    }
    #[test]
    fn composition_is_not_saved_but_unmarking_the_same_text_is_observed() {
        let mut fields = fields();
        assert!(!fields.observe("text", "中文", true, true, false));
        assert!(fields.has_composition());
        assert!(fields.observe("text", "中文", false, true, false));
        assert!(!fields.has_composition());
        assert!(!fields.observe("text", "中文", false, true, false));
    }
    #[test]
    fn shortcut_text_waits_for_a_complete_chord() {
        let mut fields = fields();
        assert!(!fields.observe("shortcut_search", "Ctrl+", false, true, false));
        assert!(fields.observe("shortcut_search", "Ctrl+G", false, false, false));
        assert!(fields.observe("shortcut_search", "Ctrl+H", false, true, true));
        assert!(!fields.observe("shortcut_search", "候选", true, true, true));
    }
    #[test]
    fn shared_batch_receipt_only_finishes_fields_without_newer_submissions() {
        let mut fields = fields();
        fields.observe("text", "first", false, true, false);
        fields.accepted("text", "first", OperationId(1));
        fields.accepted("other", "second field", OperationId(1));
        fields.observe("text", "newer", false, true, false);
        fields.accepted("text", "newer", OperationId(2));
        let receipt = fields.finish(OperationId(1), true);
        assert_eq!(receipt.saved.len(), 1);
        assert_eq!(receipt.saved[0].key, "other");
        assert!(fields.has_pending());
        assert_eq!(
            fields.finish(OperationId(2), true).saved[0].submitted,
            "newer"
        );
        assert!(!fields.has_pending());
    }
    #[test]
    fn old_success_cannot_erase_a_rejected_newer_edit_or_retry_it_forever() {
        let mut fields = fields();
        fields.accepted("text", "first", OperationId(1));
        fields.observe("text", "newer", false, true, false);
        fields.rejected("text");
        assert!(!fields.finish(OperationId(1), true).current);
        assert!(fields.has_failures());
        assert!(!fields.observe("text", "newer", false, true, false));
        assert!(fields.observe("text", "newer", false, true, true));
        fields.accepted("text", "newer", OperationId(2));
        assert!(!fields.has_failures());
        assert!(fields.finish(OperationId(2), false).current);
        assert!(fields.failed("text"));
    }
    #[test]
    fn normalized_receipt_does_not_generate_another_save() {
        let mut fields = fields();
        fields.accepted("text", " raw ", OperationId(1));
        fields.finish(OperationId(1), true);
        fields.normalized("text", "raw".into());
        assert!(!fields.observe("text", "raw", false, true, false));
    }

    #[test]
    fn a_rejected_explicit_value_is_not_hidden_by_an_older_automatic_success() {
        let mut fields = fields();
        fields.accepted("text", "first", OperationId(1));
        fields.rejected_value("text", "explicit newer");
        assert!(!fields.finish(OperationId(1), true).current);
        assert!(fields.has_failures());
        assert!(!fields.observe("text", "explicit newer", false, true, false));
    }

    #[test]
    fn close_waits_for_receipts_and_a_failure_requires_retry_or_explicit_discard() {
        let mut fields = fields();
        fields.accepted("text", "draft", OperationId(1));
        assert_eq!(fields.close_state(false, false), CloseState::Waiting);
        fields.finish(OperationId(1), false);
        assert_eq!(fields.close_state(false, false), CloseState::Failed);
        fields.observe("text", "draft", false, false, true);
        fields.accepted("text", "draft", OperationId(2));
        assert_eq!(fields.close_state(false, false), CloseState::Waiting);
        fields.finish(OperationId(2), true);
        assert_eq!(fields.close_state(false, false), CloseState::Ready);
        assert_eq!(fields.close_state(true, false), CloseState::Waiting);
        assert_eq!(fields.close_state(false, true), CloseState::Failed);
    }

    #[test]
    fn successful_text_save_does_not_hide_a_failed_choice_transaction() {
        let mut fields = FieldSaves::new([("theme", "0".into()), ("text", "old".into())]);
        fields.accepted("theme", "2", OperationId(1));
        fields.finish(OperationId(1), false);
        fields.accepted("text", "new", OperationId(2));
        fields.finish(OperationId(2), true);
        assert_eq!(fields.close_state(false, false), CloseState::Failed);
        fields.accepted("theme", "2", OperationId(3));
        fields.finish(OperationId(3), true);
        assert_eq!(fields.close_state(false, false), CloseState::Ready);
    }
}
