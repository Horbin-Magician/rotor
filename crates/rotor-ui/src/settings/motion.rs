//! Interruptible transitions, sampled only while the settings view is drawing.
use std::time::{Duration, Instant};

pub(super) fn refresh_rotation(
    started: Option<Instant>,
    now: Instant,
    reduce_motion: bool,
) -> (f32, bool) {
    let Some(started) = started.filter(|_| !reduce_motion) else {
        return (0., false);
    };
    let progress = now.saturating_duration_since(started).as_secs_f32() / 0.6;
    if progress >= 1. {
        return (0., false);
    }
    (progress * std::f32::consts::TAU, true)
}

pub(super) struct Transition {
    from: f32,
    target: f32,
    started: Instant,
    duration: Duration,
}

impl Transition {
    pub fn new(value: f32, millis: u64) -> Self {
        Self {
            from: value,
            target: value,
            started: Instant::now(),
            duration: Duration::from_millis(millis),
        }
    }

    fn value(&self, now: Instant) -> f32 {
        let t = (now.duration_since(self.started).as_secs_f32() / self.duration.as_secs_f32())
            .clamp(0., 1.);
        let eased = 1. - (1. - t).powi(3);
        self.from + (self.target - self.from) * eased
    }

    pub fn sample(&mut self, target: f32, now: Instant, reduce_motion: bool) -> (f32, bool) {
        if self.target != target {
            self.from = self.value(now);
            self.target = target;
            self.started = now;
        }
        if reduce_motion {
            self.from = target;
        }
        let running = self.from != self.target && now.duration_since(self.started) < self.duration;
        (if running { self.value(now) } else { target }, running)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_rotates_once_then_stops() {
        let start = Instant::now();
        assert_eq!(refresh_rotation(None, start, false), (0., false));
        assert_eq!(refresh_rotation(Some(start), start, false), (0., true));
        let (angle, running) =
            refresh_rotation(Some(start), start + Duration::from_millis(300), false);
        assert!(running && (angle - std::f32::consts::PI).abs() < 0.001);
        assert_eq!(
            refresh_rotation(Some(start), start + Duration::from_millis(600), false),
            (0., false)
        );
        assert_eq!(refresh_rotation(Some(start), start, true), (0., false));
    }

    #[test]
    fn reversal_starts_at_current_value_and_settles() {
        let mut transition = Transition::new(0., 200);
        let start = Instant::now();
        assert_eq!(transition.sample(1., start, false), (0., true));
        let halfway = start + Duration::from_millis(100);
        let (value, running) = transition.sample(1., halfway, false);
        assert!(running && value > 0. && value < 1.);
        assert_eq!(transition.sample(0., halfway, false), (value, true));
        assert_eq!(
            transition.sample(0., halfway + Duration::from_millis(200), false),
            (0., false)
        );
    }

    #[test]
    fn reduced_motion_snaps_and_stops() {
        let mut transition = Transition::new(0., 200);
        let now = Instant::now();
        assert_eq!(transition.sample(1., now, true), (1., false));
        assert_eq!(transition.sample(1., now, false), (1., false));
    }
}
