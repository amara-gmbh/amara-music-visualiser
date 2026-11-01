//! A separate, slow colour journey. It never changes music geometry, world
//! choice, or playback pace. RGB values are linear-light tint targets.
use crate::presets::BackgroundPalette;
use std::time::{SystemTime, UNIX_EPOCH};

const MELT_SECONDS: f64 = 10.0;
const TINT_AMOUNT: f32 = 0.46;

pub struct BackgroundJourney {
    selection: BackgroundPalette,
    target: BackgroundPalette,
    from: [f32; 4],
    to: [f32; 4],
    elapsed: f64,
    hold_remaining: f64,
    remaining: Vec<usize>,
    recent: [Option<usize>; 2],
    random: u64,
}

impl BackgroundJourney {
    pub fn new(selection: BackgroundPalette) -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self::seeded(selection, seed)
    }

    fn seeded(selection: BackgroundPalette, seed: u64) -> Self {
        let mut journey = Self {
            selection,
            target: selection,
            from: [0.0; 4],
            to: [0.0; 4],
            elapsed: MELT_SECONDS,
            hold_remaining: 0.0,
            remaining: Vec::new(),
            recent: [None; 2],
            random: seed.max(1),
        };
        if selection == BackgroundPalette::Auto {
            journey.target = journey.choose();
        }
        journey.to = tint(journey.target);
        journey.from = journey.to;
        journey.hold_remaining = journey.next_hold();
        journey
    }

    fn draw(&mut self) -> f64 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        (self.random >> 11) as f64 / (1_u64 << 53) as f64
    }

    fn next_hold(&mut self) -> f64 {
        20.0 + 15.0 * self.draw()
    }

    fn choose(&mut self) -> BackgroundPalette {
        if self.remaining.is_empty() {
            self.remaining.extend(0..BackgroundPalette::COLOURS.len());
            for index in (1..self.remaining.len()).rev() {
                let other = (self.draw() * (index + 1) as f64) as usize;
                self.remaining.swap(index, other);
            }
        }
        // A shuffle bag guarantees coverage, while these two exclusions also
        // avoid an immediate repeat or A → B → A at the bag boundary.
        let position = self
            .remaining
            .iter()
            .rposition(|index| !self.recent.contains(&Some(*index)))
            .unwrap_or(self.remaining.len() - 1);
        let next = self.remaining.remove(position);
        self.recent = [Some(next), self.recent[0]];
        BackgroundPalette::COLOURS[next]
    }

    fn begin(&mut self, target: BackgroundPalette) {
        // Sampling the current interpolated colour preserves position even if
        // a user chooses again halfway through a melt.
        self.from = self.uniforms();
        self.target = target;
        self.to = tint(target);
        self.elapsed = 0.0;
        self.hold_remaining = self.next_hold();
    }

    pub fn shuffle(&mut self) {
        if self.selection == BackgroundPalette::Auto {
            let target = self.choose();
            self.begin(target);
        }
    }

    pub fn target_label(&self) -> &'static str {
        self.target.label()
    }

    pub fn update(&mut self, selection: BackgroundPalette, dt: f32) -> [f32; 4] {
        if selection != self.selection {
            self.selection = selection;
            let target = if selection == BackgroundPalette::Auto {
                self.choose()
            } else {
                selection
            };
            self.begin(target);
        }
        let mut remaining = if dt.is_finite() {
            f64::from(dt.clamp(0.0, 3600.0))
        } else {
            0.0
        };
        // Carry time over exact boundaries so 30/60/120 Hz travel through the
        // same sequence without timer drift. Holds begin after a complete melt.
        while remaining > 0.0 {
            if self.elapsed < MELT_SECONDS {
                let step = remaining.min(MELT_SECONDS - self.elapsed);
                self.elapsed += step;
                remaining -= step;
            } else if self.selection == BackgroundPalette::Auto {
                let step = remaining.min(self.hold_remaining);
                self.hold_remaining -= step;
                remaining -= step;
                if self.hold_remaining <= 1e-9 {
                    let target = self.choose();
                    self.begin(target);
                }
            } else {
                break;
            }
        }
        self.uniforms()
    }

    fn uniforms(&self) -> [f32; 4] {
        let t = (self.elapsed / MELT_SECONDS).clamp(0.0, 1.0) as f32;
        let blend = t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
        std::array::from_fn(|index| self.from[index] + (self.to[index] - self.from[index]) * blend)
    }
}

fn tint(palette: BackgroundPalette) -> [f32; 4] {
    let rgb = match palette {
        BackgroundPalette::Auto | BackgroundPalette::WorldColours => return [0.0; 4],
        BackgroundPalette::VioletNight => [0.18, 0.055, 0.34],
        BackgroundPalette::OceanJade => [0.035, 0.24, 0.19],
        BackgroundPalette::EmberGold => [0.40, 0.12, 0.035],
        BackgroundPalette::RoseGlass => [0.32, 0.075, 0.18],
        BackgroundPalette::DeepBlue => [0.035, 0.12, 0.38],
        BackgroundPalette::PearlAmber => [0.30, 0.22, 0.12],
        BackgroundPalette::Lagoon => [0.035, 0.24, 0.32],
        BackgroundPalette::SlateLilac => [0.16, 0.14, 0.25],
    };
    [rgb[0], rgb[1], rgb[2], TINT_AMOUNT]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shuffle_bag_covers_all_colours_without_repeats_or_backtracks() {
        for seed in 1..64 {
            let mut journey = BackgroundJourney::seeded(BackgroundPalette::WorldColours, seed);
            let mut previous = [None; 2];
            for _ in 0..16 {
                let mut cycle = Vec::new();
                for _ in BackgroundPalette::COLOURS {
                    let colour = journey.choose();
                    assert!(!previous.contains(&Some(colour)));
                    assert!(!cycle.contains(&colour));
                    previous = [Some(colour), previous[0]];
                    cycle.push(colour);
                }
                assert_eq!(cycle.len(), BackgroundPalette::COLOURS.len());
            }
        }
    }

    #[test]
    fn manual_choices_and_shuffles_never_jump_at_selection_time() {
        let mut journey = BackgroundJourney::seeded(BackgroundPalette::VioletNight, 19);
        let before = journey.uniforms();
        assert_eq!(journey.update(BackgroundPalette::EmberGold, 0.0), before);
        let halfway = journey.update(BackgroundPalette::EmberGold, 5.0);
        assert_ne!(halfway, before);
        assert_ne!(halfway, tint(BackgroundPalette::EmberGold));
        assert_eq!(journey.update(BackgroundPalette::Auto, 0.0), halfway);
        journey.shuffle();
        assert_eq!(journey.uniforms(), halfway);
        let moved = journey.update(BackgroundPalette::Auto, 1.0 / 60.0);
        assert!(
            moved
                .into_iter()
                .zip(halfway)
                .all(|(a, b)| (a - b).abs() < 0.0001)
        );
    }

    #[test]
    fn physical_elapsed_time_is_independent_of_frame_rate() {
        let run = |hz: usize| {
            let mut journey = BackgroundJourney::seeded(BackgroundPalette::Auto, 47);
            for _ in 0..hz * 480 {
                journey.update(BackgroundPalette::Auto, 1.0 / hz as f32);
            }
            (journey.target, journey.uniforms())
        };
        let reference = run(60);
        for hz in [30, 120] {
            let actual = run(hz);
            assert_eq!(actual.0, reference.0);
            for (a, b) in actual.1.into_iter().zip(reference.1) {
                assert!((a - b).abs() < 0.0001);
            }
        }
    }

    #[test]
    fn world_colours_disable_tint_and_invalid_time_remains_finite() {
        let mut journey = BackgroundJourney::seeded(BackgroundPalette::OceanJade, 12);
        assert_eq!(
            journey.update(BackgroundPalette::WorldColours, 10.0),
            [0.0; 4]
        );
        for dt in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -3.0, 0.0] {
            assert_eq!(
                journey.update(BackgroundPalette::WorldColours, dt),
                [0.0; 4]
            );
        }
    }

    #[test]
    fn manual_background_stays_held_and_shuffle_is_auto_only() {
        let mut journey = BackgroundJourney::seeded(BackgroundPalette::RoseGlass, 93);
        journey.shuffle();
        assert_eq!(
            journey.update(BackgroundPalette::RoseGlass, 1000.0),
            tint(BackgroundPalette::RoseGlass)
        );
    }

    #[test]
    fn auto_holds_then_melts_and_uses_bounded_linear_colours() {
        let mut journey = BackgroundJourney::seeded(BackgroundPalette::Auto, 51);
        assert!((20.0..35.0).contains(&journey.hold_remaining));
        let before = journey.uniforms();
        assert_eq!(journey.update(BackgroundPalette::Auto, 19.0), before);
        for _ in 0..2400 {
            let value = journey.update(BackgroundPalette::Auto, 0.5);
            assert!(
                value
                    .into_iter()
                    .all(|v| v.is_finite() && (0.0..=TINT_AMOUNT).contains(&v))
            );
        }
    }
}
