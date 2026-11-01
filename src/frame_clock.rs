use std::time::Instant;

/// Advances animation once per egui frame, even when layout needs more than
/// one UI pass. Tiny real intervals stay tiny; hidden/startup loops cannot
/// manufacture animation time through a minimum timestep.
pub struct FrameClock {
    last_frame_id: Option<u64>,
    last_time: Instant,
    render_delta: f32,
}

impl FrameClock {
    pub fn new(now: Instant) -> Self {
        Self {
            last_frame_id: None,
            last_time: now,
            render_delta: 0.0,
        }
    }

    /// Returns animation time only on the first pass of a new frame.
    pub fn step(&mut self, frame_id: u64, now: Instant) -> f32 {
        if self.last_frame_id == Some(frame_id) {
            return 0.0;
        }
        self.last_frame_id = Some(frame_id);
        self.render_delta = now
            .saturating_duration_since(self.last_time)
            .as_secs_f32()
            .min(0.1);
        // Ignore a backward observation instead of rewinding the reference
        // and counting that interval a second time on the following frame.
        self.last_time = self.last_time.max(now);
        self.render_delta
    }

    /// The current frame's real render interval, retained across UI passes.
    pub fn delta(&self) -> f32 {
        self.render_delta
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn tiny_intervals_sum_actual_time_without_a_minimum_floor() {
        let start = Instant::now();
        let mut clock = FrameClock::new(start);
        let mut total = 0.0;
        for frame in 1..=1_000 {
            let dt = clock.step(frame, start + Duration::from_micros(frame * 100));
            assert!((dt - 0.000_1).abs() < 0.000_000_1);
            total += dt;
        }
        assert!((total - 0.1).abs() < 0.000_002);
    }

    #[test]
    fn duplicate_ui_passes_neither_advance_nor_move_the_clock_reference() {
        let start = Instant::now();
        let mut clock = FrameClock::new(start);
        let first = clock.step(10, start + Duration::from_millis(16));
        assert!((first - 0.016).abs() < 0.000_001);
        assert_eq!(clock.step(10, start + Duration::from_millis(21)), 0.0);
        assert_eq!(clock.delta(), first);
        let second = clock.step(11, start + Duration::from_millis(32));
        assert!((second - 0.016).abs() < 0.000_001);
    }

    #[test]
    fn long_pause_is_capped_and_the_next_frame_resumes_normally() {
        let start = Instant::now();
        let mut clock = FrameClock::new(start);
        assert_eq!(clock.step(1, start + Duration::from_secs(30)), 0.1);
        let resumed = clock.step(
            2,
            start + Duration::from_secs(30) + Duration::from_millis(16),
        );
        assert!((resumed - 0.016).abs() < 0.000_001);
        assert_eq!(clock.delta(), resumed);
    }

    #[test]
    fn backward_observations_stay_finite_and_do_not_recount_time() {
        let start = Instant::now();
        let mut clock = FrameClock::new(start);
        clock.step(1, start + Duration::from_millis(10));
        let backward = clock.step(2, start + Duration::from_millis(5));
        assert!(backward.is_finite());
        assert_eq!(backward, 0.0);
        let forward = clock.step(3, start + Duration::from_millis(20));
        assert!((forward - 0.01).abs() < 0.000_001);
    }
}
