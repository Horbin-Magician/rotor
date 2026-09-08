pub fn recoverable_session_id(
    ready_session_id: u32,
    current_session_id: u32,
    has_capture: bool,
) -> Option<u32> {
    (ready_session_id != 0 && ready_session_id == current_session_id && has_capture)
        .then_some(ready_session_id)
}

/// Native capture generations are supplied by the runtime's unique operation
/// IDs. Images remain owned by the shell/cache, not by a window entity here.
#[derive(Default)]
pub struct NativeSession {
    phase: NativePhase,
}

#[derive(Default)]
enum NativePhase {
    #[default]
    Idle,
    Capturing(u64),
    Ready {
        id: u64,
        monitors: Vec<crate::monitor::MonitorConfig>,
    },
}

impl NativeSession {
    pub fn generation(&self) -> Option<u64> {
        match self.phase {
            NativePhase::Idle => None,
            NativePhase::Capturing(id) | NativePhase::Ready { id, .. } => Some(id),
        }
    }
    pub fn begin(&mut self, id: u64) {
        self.phase = NativePhase::Capturing(id);
    }
    pub fn cancel(&mut self) {
        self.phase = NativePhase::Idle;
    }
    pub fn is_capturing(&self, id: u64) -> bool {
        matches!(self.phase, NativePhase::Capturing(current) if current == id)
    }

    pub fn complete(
        &mut self,
        id: u64,
        mut monitors: Vec<crate::monitor::MonitorConfig>,
    ) -> Result<bool, String> {
        if !self.is_capturing(id) {
            return Ok(false);
        }
        monitors.sort_by_key(|monitor| monitor.id);
        if monitors.is_empty()
            || monitors.iter().any(|monitor| {
                monitor.width == 0
                    || monitor.height == 0
                    || !monitor.scale_factor.is_finite()
                    || monitor.scale_factor <= 0.
            })
            || monitors.windows(2).any(|pair| pair[0].id == pair[1].id)
        {
            self.cancel();
            return Err("Invalid capture monitor topology".into());
        }
        self.phase = NativePhase::Ready { id, monitors };
        Ok(true)
    }

    pub fn is_ready(&self, id: u64, monitor_id: u32) -> bool {
        matches!(&self.phase, NativePhase::Ready { id: current, monitors } if *current == id && monitors.iter().any(|monitor| monitor.id == monitor_id))
    }

    pub fn consume(&mut self, id: u64, monitor_id: u32) -> bool {
        if !self.is_ready(id, monitor_id) {
            return false;
        }
        self.cancel();
        true
    }

    pub fn recoverable(&self, id: u64, mut current: Vec<crate::monitor::MonitorConfig>) -> bool {
        current.sort_by_key(|monitor| monitor.id);
        matches!(&self.phase, NativePhase::Ready { id: ready, monitors } if *ready == id && monitors == &current)
    }
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

    fn monitor(id: u32) -> crate::monitor::MonitorConfig {
        crate::monitor::MonitorConfig {
            id,
            x: -1920,
            y: 0,
            width: 1920,
            height: 1080,
            scale_factor: 1.25,
        }
    }

    #[test]
    fn cancelled_or_superseded_native_capture_cannot_become_ready() {
        let mut session = super::NativeSession::default();
        session.begin(1);
        session.begin(2);
        assert!(!session.complete(1, vec![monitor(1)]).unwrap());
        assert!(session.is_capturing(2));
        session.cancel();
        assert!(!session.complete(2, vec![monitor(1)]).unwrap());
    }

    #[test]
    fn native_selection_is_consumed_once_and_requires_a_captured_monitor() {
        let mut session = super::NativeSession::default();
        session.begin(3);
        assert!(session.complete(3, vec![monitor(1), monitor(2)]).unwrap());
        assert!(!session.consume(3, 99));
        assert!(session.consume(3, 2));
        assert!(!session.consume(3, 1));
    }

    #[test]
    fn native_recovery_rejects_dpi_changes_and_duplicate_monitors() {
        let mut session = super::NativeSession::default();
        session.begin(4);
        session.complete(4, vec![monitor(1)]).unwrap();
        assert!(session.recoverable(4, vec![monitor(1)]));
        let mut changed = monitor(1);
        changed.scale_factor = 2.;
        assert!(!session.recoverable(4, vec![changed]));
        session.begin(5);
        assert!(session.complete(5, vec![monitor(1), monitor(1)]).is_err());
        assert!(!session.is_ready(5, 1));
    }
}
