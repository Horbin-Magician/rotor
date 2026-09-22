//! Close and quit arbitration for the settings window.
//!
//! Kept free of GPUI so the two rules that matter are unit-tested: a pending
//! application quit is never downgraded by a later window close, and a close
//! only proceeds once every accepted save has a receipt.
use super::autosave::CloseState;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum CloseTarget {
    Window,
    Application,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum CloseDecision {
    /// Receipts are still outstanding, or nothing asked to close.
    Wait,
    /// A save failed; keep the window open with the draft visible.
    Failed,
    Close(CloseTarget),
}

#[derive(Debug)]
pub(super) struct CloseIntent {
    request: Option<CloseTarget>,
    last: CloseTarget,
}

impl Default for CloseIntent {
    fn default() -> Self {
        Self {
            request: None,
            last: CloseTarget::Window,
        }
    }
}

impl CloseIntent {
    pub fn is_pending(&self) -> bool {
        self.request.is_some()
    }

    pub fn waiting_to_quit(&self) -> bool {
        self.request == Some(CloseTarget::Application)
    }

    /// A later window-close event must not downgrade an application quit
    /// already waiting for its save receipts (including updater handoff).
    pub fn effective_target(&self, target: CloseTarget) -> CloseTarget {
        if self.waiting_to_quit() {
            CloseTarget::Application
        } else {
            target
        }
    }

    pub fn begin(&mut self, target: CloseTarget) {
        let target = self.effective_target(target);
        self.request = Some(target);
        self.last = target;
    }

    /// Drop the request; `resume` restores the same target later.
    pub fn clear(&mut self) {
        self.request = None;
    }

    pub fn resume(&mut self) {
        self.request = Some(self.last);
    }

    pub fn last(&self) -> CloseTarget {
        self.last
    }

    pub fn decide(&self, state: CloseState) -> CloseDecision {
        match (self.request, state) {
            (None, _) | (Some(_), CloseState::Waiting) => CloseDecision::Wait,
            (Some(_), CloseState::Failed) => CloseDecision::Failed,
            (Some(target), CloseState::Ready) => CloseDecision::Close(target),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_quit_is_never_downgraded_by_a_later_window_close() {
        let mut intent = CloseIntent::default();
        assert!(!intent.is_pending());
        assert_eq!(intent.decide(CloseState::Ready), CloseDecision::Wait);

        intent.begin(CloseTarget::Application);
        assert!(intent.waiting_to_quit());
        assert_eq!(
            intent.effective_target(CloseTarget::Window),
            CloseTarget::Application
        );
        intent.begin(CloseTarget::Window);
        assert!(intent.waiting_to_quit());
        assert_eq!(
            intent.decide(CloseState::Ready),
            CloseDecision::Close(CloseTarget::Application)
        );

        // A window close that fails and is retried keeps its own target.
        let mut intent = CloseIntent::default();
        intent.begin(CloseTarget::Window);
        assert_eq!(intent.decide(CloseState::Waiting), CloseDecision::Wait);
        assert_eq!(intent.decide(CloseState::Failed), CloseDecision::Failed);
        intent.clear();
        assert!(!intent.is_pending());
        assert_eq!(intent.decide(CloseState::Ready), CloseDecision::Wait);
        intent.resume();
        assert_eq!(intent.last(), CloseTarget::Window);
        assert_eq!(
            intent.decide(CloseState::Ready),
            CloseDecision::Close(CloseTarget::Window)
        );
    }
}
