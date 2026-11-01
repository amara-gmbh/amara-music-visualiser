//! Slow, independent colour changes for the worlds' material. The user's hue
//! remains the base; this journey supplies an offset only while Auto advances.
use std::time::{SystemTime, UNIX_EPOCH};

// Unevenly spaced offsets keep related colours together within each world,
// while visiting warm, cool, complementary and near-original palettes.
const HUES: [f64; 8] = [0.0, 0.11, 0.24, 0.36, 0.51, 0.64, 0.77, 0.89];

pub struct MaterialColourJourney {
    from: f64,
    to: f64,
    elapsed: f64,
    melt_seconds: f64,
    hold_remaining: f64,
    remaining: Vec<usize>,
    recent: [Option<usize>; 2],
    random: u64,
}

impl MaterialColourJourney {
    pub fn new(auto: bool) -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self::seeded(auto, seed)
    }

    fn seeded(auto: bool, seed: u64) -> Self {
        let mut journey = Self {
            from: 0.0,
            to: 0.0,
            elapsed: 10.0,
            melt_seconds: 10.0,
            hold_remaining: 0.0,
            remaining: Vec::new(),
            recent: [None; 2],
            random: seed.max(1),
        };
        // A restored manual profile opens at its exact chosen hue. Auto gets
        // a fresh starting material instead of repeating the same first look.
        if auto {
            journey.to = journey.choose();
            journey.from = journey.to;
        }
        journey.hold_remaining = 15.0 + 10.0 * journey.draw();
        journey
    }

    fn draw(&mut self) -> f64 {
        self.random ^= self.random >> 12;
        self.random ^= self.random << 25;
        self.random ^= self.random >> 27;
        (self.random.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 11) as f64 / 9_007_199_254_740_992.0
    }

    fn choose(&mut self) -> f64 {
        if self.remaining.is_empty() {
            self.remaining.extend(0..HUES.len());
            for index in (1..self.remaining.len()).rev() {
                let other = (self.draw() * (index + 1) as f64) as usize;
                self.remaining.swap(index, other);
            }
        }
        let position = self
            .remaining
            .iter()
            .rposition(|index| !self.recent.contains(&Some(*index)))
            .unwrap_or(self.remaining.len() - 1);
        let next = self.remaining.remove(position);
        self.recent = [Some(next), self.recent[0]];
        HUES[next]
    }

    fn current(&self) -> f64 {
        let t = (self.elapsed / self.melt_seconds).clamp(0.0, 1.0);
        let blend = t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
        self.from + (self.to - self.from) * blend
    }

    fn begin(&mut self) {
        self.from = self.current().rem_euclid(1.0);
        self.to = self.from + shortest_arc(self.from, self.choose());
        self.melt_seconds = 8.0 + 4.0 * self.draw();
        self.hold_remaining = 15.0 + 10.0 * self.draw();
        self.elapsed = 0.0;
    }

    pub fn update(&mut self, auto: bool, dt: f32) -> f32 {
        let mut remaining = if auto && dt.is_finite() {
            f64::from(dt.clamp(0.0, 3600.0))
        } else {
            0.0
        };
        // Keep physical time across boundaries and frame rates. Turning Auto
        // off freezes both the visible hue and its timer, even during a melt.
        while remaining > 0.0 {
            if self.elapsed < self.melt_seconds {
                let step = remaining.min(self.melt_seconds - self.elapsed);
                self.elapsed += step;
                remaining -= step;
            } else {
                let step = remaining.min(self.hold_remaining);
                self.hold_remaining -= step;
                remaining -= step;
                if self.hold_remaining <= 1e-9 {
                    self.begin();
                }
            }
        }
        self.current().rem_euclid(1.0) as f32
    }
}

fn shortest_arc(from: f64, to: f64) -> f64 {
    (to - from + 0.5).rem_euclid(1.0) - 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bag_visits_every_material_without_repeats_or_backtracks() {
        let mut first_colours = Vec::new();
        for seed in 1..32 {
            let mut journey = MaterialColourJourney::seeded(false, seed);
            let mut previous = [None; 2];
            for _ in 0..8 {
                let mut cycle = Vec::new();
                for _ in HUES {
                    let colour = journey.choose();
                    assert!(!previous.contains(&Some(colour)));
                    assert!(!cycle.contains(&colour));
                    previous = [Some(colour), previous[0]];
                    cycle.push(colour);
                }
            }
            first_colours.push(MaterialColourJourney::seeded(true, seed).to);
        }
        assert!(first_colours.iter().any(|hue| *hue != first_colours[0]));
    }

    #[test]
    fn hue_crosses_zero_by_the_shortest_arc_with_eased_endpoints() {
        let mut journey = MaterialColourJourney::seeded(false, 17);
        journey.from = 0.96;
        journey.to = journey.from + shortest_arc(0.96, 0.04);
        journey.melt_seconds = 10.0;
        journey.elapsed = 0.0;
        assert!((journey.update(true, 0.01) - 0.96).abs() < 0.000001);
        journey.elapsed = 5.0;
        assert!(shortest_arc(journey.current(), 0.0).abs() < 0.000001);
        journey.elapsed = 9.99;
        assert!(shortest_arc(journey.current(), 0.04).abs() < 0.000001);
    }

    #[test]
    fn manual_mode_freezes_and_resumes_mid_melt_without_a_jump() {
        let mut journey = MaterialColourJourney::seeded(false, 91);
        assert_eq!(journey.update(false, 3600.0), 0.0);
        journey.begin();
        assert!((8.0..12.0).contains(&journey.melt_seconds));
        assert!((15.0..25.0).contains(&journey.hold_remaining));
        let midway = journey.update(true, 4.0);
        let elapsed = journey.elapsed;
        for dt in [3600.0, f32::NAN, f32::INFINITY, -1.0] {
            assert_eq!(journey.update(false, dt), midway);
        }
        assert_eq!(journey.elapsed, elapsed);
        assert_eq!(journey.update(true, 0.0), midway);
        let resumed = journey.update(true, 1.0 / 60.0);
        assert!(shortest_arc(f64::from(midway), f64::from(resumed)).abs() < 0.003);
    }

    #[test]
    fn holds_and_melts_use_elapsed_time_independent_of_frame_rate() {
        let run = |hz: usize| {
            let mut journey = MaterialColourJourney::seeded(true, 47);
            let first = journey.update(true, 0.0);
            assert_eq!(journey.update(true, 14.0), first);
            for _ in 0..hz * 240 {
                journey.update(true, 1.0 / hz as f32);
            }
            journey.update(true, 0.0)
        };
        let reference = run(60);
        for hz in [30, 120] {
            assert!(shortest_arc(f64::from(reference), f64::from(run(hz))).abs() < 0.0001);
        }
    }
}
