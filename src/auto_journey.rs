//! Global automatic variety, not AI or detected song structure. Randomness is
//! seeded once; measured, user-mapped evidence gently biases phrase choices.
use crate::{
    dsp::AudioFeatures,
    presets::{AudioMappings, MathForm, WORLD_COUNT},
};
use std::time::{SystemTime, UNIX_EPOCH};

// Every world shares one fair pool. The extra passages allow randomized
// pool boundaries without leaving an early pick absent for two whole tours.
const MAX_RETURN_PASSAGES: u8 = WORLD_COUNT as u8 + 3;
const COVERAGE_WORLDS: u32 = (1_u32 << WORLD_COUNT) - 1;
const FRACTAL_WORLD: usize = 1;
const MANDELBROT_WORLD: usize = 10;
const ALL_MATH_FAMILIES: u8 = 0b1111;

#[derive(Default)]
struct Evidence {
    seconds: f64,
    sums: [f64; 3],
}

impl Evidence {
    fn add(&mut self, levels: [f64; 3], dt: f64) {
        self.seconds += dt;
        for (sum, value) in self.sums.iter_mut().zip(levels) {
            *sum += value * dt;
        }
    }

    fn average(&self) -> [f64; 3] {
        if self.seconds > 0.0 {
            self.sums.map(|sum| (sum / self.seconds).clamp(0.0, 1.0))
        } else {
            [0.0; 3]
        }
    }
}

pub struct AutoJourney {
    current: usize,
    remaining_worlds: u32,
    ages: [u8; WORLD_COUNT],
    // Coverage stops starvation; directed-pair memory also stops every visit
    // feeling like the same familiar neighbour followed by the same neighbour.
    pair_visits: [[u16; WORLD_COUNT]; WORLD_COUNT],
    previous: Option<usize>,
    elapsed: f64,
    hold_jitter: f64,
    evidence: Evidence,
    random: u64,
    math_random: u64,
    math_family: usize,
    math_remaining: u8,
    math_seen: bool,
    math_elapsed: f64,
    math_duration: f64,
    mix_seed: f32,
    morph_duration: f32,
}

impl AutoJourney {
    pub fn new(previous_world: usize, auto: bool) -> Self {
        let clock = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let seed = clock as u64 ^ ((clock >> 64) as u64).rotate_left(17);
        Self::startup_with_seed(previous_world, auto, seed)
    }

    fn startup_with_seed(previous_world: usize, auto: bool, seed: u64) -> Self {
        let previous = previous_world.min(WORLD_COUNT - 1);
        let initial = if auto {
            // last-profile already contains the last displayed destination.
            // Use it only to avoid a repeated opening, without new storage or
            // changing the user's restored manual profile.
            let mut startup_random = (seed.rotate_left(43) ^ 0x8d58ac26afe12e47).max(1);
            let ticket = (sample(&mut startup_random) * (WORLD_COUNT - 1) as f64) as usize;
            ticket + usize::from(ticket >= previous)
        } else {
            previous
        };
        Self::new_with_seed(initial, seed)
    }

    pub fn current_world(&self) -> usize {
        self.current
    }

    pub(crate) fn new_with_seed(initial_world: usize, seed: u64) -> Self {
        let current = initial_world.min(WORLD_COUNT - 1);
        let mut journey = Self {
            current,
            remaining_worlds: COVERAGE_WORLDS & !(1 << current),
            ages: [0; WORLD_COUNT],
            pair_visits: [[0; WORLD_COUNT]; WORLD_COUNT],
            previous: None,
            elapsed: 0.0,
            hold_jitter: 0.0,
            evidence: Evidence::default(),
            random: if seed == 0 { 0x9e3779b97f4a7c15 } else { seed },
            math_random: seed.rotate_left(29) ^ 0xd1b54a32d192ed03,
            math_family: 0,
            math_remaining: ALL_MATH_FAMILIES,
            math_seen: current == FRACTAL_WORLD,
            math_elapsed: 0.0,
            math_duration: 15.0,
            mix_seed: 0.0,
            morph_duration: 8.0,
        };
        if journey.math_random == 0 {
            journey.math_random = 0x94d049bb133111eb;
        }
        journey.ages[current] = 0;
        journey.hold_jitter = journey.draw() * 6.0 - 3.0;
        journey.mix_seed = (journey.draw() * 1000.0) as f32;
        journey.morph_duration = (7.0 + journey.draw() * 2.0) as f32;
        journey.math_family = (journey.draw_math() * 4.0) as usize;
        if journey.math_seen {
            journey.math_remaining &= !(1 << journey.math_family);
        }
        journey.math_duration = 12.0 + journey.draw_math() * 6.0;
        journey
    }

    /// Independently evolving subeffect choice. Feed this into MathJourney;
    /// that layer retains its own queued, continuous morph.
    pub fn math_form(&self) -> MathForm {
        match self.math_family {
            0 => MathForm::Julia,
            1 => MathForm::InversionLace,
            2 => MathForm::HarmonicSculpture,
            _ => MathForm::VortexWeave,
        }
    }

    /// Stable throughout each spatial interlace; changes only on a new world
    /// request. The root latches it when that world's morph actually begins.
    pub fn blend_seed(&self) -> f32 {
        self.mix_seed
    }

    /// Stable duration for the next interlace. Latch with blend_seed when the
    /// scene pair changes, never alter an already active morph's clock.
    pub fn morph_seconds(&self) -> f32 {
        self.morph_duration
    }

    /// Elapsed fraction and seconds until the next world request, using the
    /// same live deadline as update. This physical-time clock keeps running
    /// during the current world's melt; musical evidence may gently adjust it.
    /// Reading it never draws randomness or advances any journey state.
    pub fn next_world_progress(&self) -> (f32, f32) {
        let duration = self.request_after_seconds();
        (
            (self.elapsed / duration).clamp(0.0, 1.0) as f32,
            (duration - self.elapsed).max(0.0) as f32,
        )
    }

    fn request_after_seconds(&self) -> f64 {
        let [energy, _, transients] = self.evidence.average();
        // Keep Mandelbrot's existing zoom/turn/redive pace, but leave time for
        // unfolding, a detail dive and renewed formation before departure. This
        // changes exposure, never selection frequency or the inner clocks.
        let centre = if self.current == MANDELBROT_WORLD {
            67.0
        } else {
            21.0
        };
        (centre + self.hold_jitter - energy * 0.5 - transients * 0.25)
            .clamp(centre - 3.0, centre + 3.0)
    }

    /// Call on manual selection/resuming Auto, not on every render frame.
    /// Begin a fresh hold from the actual selected destination, without
    /// reseeding or returning to a fixed world sequence.
    pub fn set_current(&mut self, world: usize) {
        self.current = world.min(WORLD_COUNT - 1);
        // Resuming Auto is not amnesia: the world the user just watched counts
        // as visited, while other worlds retain their coverage/recency history.
        self.remaining_worlds &= !(1 << self.current);
        self.ages[self.current] = 0;
        self.elapsed = 0.0;
        self.evidence = Evidence::default();
        if self.current == FRACTAL_WORLD {
            // Resuming the current/manual Fractal is a hold, not a new entry:
            // do not request a different mathematical endpoint on that toggle.
            self.math_remaining &= !(1 << self.math_family);
            self.math_seen = true;
            self.math_elapsed = 0.0;
        }
    }

    /// Source changes discard musical evidence, not the journey's clock,
    /// selected world, recency weights or RNG. The root owns in-flight morphs.
    pub fn reset_evidence(&mut self) {
        self.evidence = Evidence::default();
    }

    /// Request a new main world every 18–24 physical seconds, or 64–70 for
    /// Mandelbrot. Preserve the latched 7–9 second melt and inner look clocks.
    /// Feed physical elapsed time here, independently of the flight Pace.
    /// A pause/repeated UI pass cannot advance time or consume randomness.
    pub fn update(
        &mut self,
        features: &AudioFeatures,
        mappings: &AudioMappings,
        dt: f32,
    ) -> Option<usize> {
        let dt = if dt.is_finite() {
            f64::from(dt.clamp(0.0, 0.25))
        } else {
            0.0
        };
        if dt == 0.0 {
            return None;
        }
        let levels = mapped_levels(features, mappings);
        self.evidence.add(levels, dt);
        self.elapsed += dt;
        let [energy, brightness, transients] = self.evidence.average();
        if self.current == FRACTAL_WORLD {
            self.advance_math([energy, brightness, transients], dt);
        }
        let duration = self.request_after_seconds();
        if self.elapsed + 1e-9 < duration {
            return None;
        }

        // One appearance per pool for EVERY world, including Flight. Music and
        // pair novelty affect only the order, never a world's long-run share.
        // Deadline feasibility keeps pool boundaries varied but starvation-free.
        let preferences: [f64; WORLD_COUNT] = [
            0.8 + energy * 0.9 + transients * 0.35,
            0.9 + (1.0 - energy) * 0.55 + brightness * 0.35,
            0.9 + (1.0 - energy) * 0.75 + brightness * 0.25,
            0.8 + transients * 0.7 + energy * 0.35 + brightness * 0.25,
            0.9 + brightness * 0.8 + transients * 0.45,
            1.0 + (1.0 - energy) * 0.45 + transients * 0.35,
            0.9 + energy * 0.6 + (1.0 - brightness) * 0.5,
            0.95 + energy * 0.4 + transients * 0.35 + brightness * 0.2,
            0.95 + (1.0 - energy) * 0.45 + transients * 0.4 + brightness * 0.15,
            0.95 + (1.0 - energy) * 0.55 + brightness * 0.45,
            0.95 + energy * 0.40 + brightness * 0.35,
            0.95 + brightness * 0.50 + (1.0 - energy) * 0.30,
            0.95 + energy * 0.45 + transients * 0.35,
            0.95 + transients * 0.45 + brightness * 0.35,
            0.95 + energy * 0.40 + brightness * 0.30,
            0.95 + (1.0 - energy) * 0.35 + brightness * 0.30,
            0.95 + energy * 0.35 + brightness * 0.35,
        ];
        let next = {
            if self.remaining_worlds == 0 {
                self.remaining_worlds = COVERAGE_WORLDS;
            }
            let safe: [bool; WORLD_COUNT] = std::array::from_fn(|world| {
                world != self.current && self.choice_keeps_deadlines(world)
            });
            let least_used_pair = (0..WORLD_COUNT)
                .filter(|world| safe[*world])
                .map(|world| self.pair_visits[self.current][world])
                .min()
                .unwrap_or(0);
            let weights: [f64; WORLD_COUNT] = std::array::from_fn(|world| {
                if !safe[world] {
                    0.0
                } else {
                    let repeated_pair = self.pair_visits[self.current][world]
                        .saturating_sub(least_used_pair)
                        .min(3);
                    let pair_novelty = 0.22_f64.powi(i32::from(repeated_pair));
                    let backtrack = if self.previous == Some(world) {
                        0.18
                    } else {
                        1.0
                    };
                    preferences[world]
                        * (1.0 + f64::from(self.ages[world]) * 0.08)
                        * pair_novelty
                        * backtrack
                }
            });
            let total = weights.iter().sum::<f64>();
            debug_assert!(total > 0.0, "a coverage deadline must remain feasible");
            let mut choice = self.draw() * total;
            let mut selected = self.current;
            for (world, weight) in weights.into_iter().enumerate() {
                if weight > 0.0 {
                    selected = world;
                    if choice < weight {
                        break;
                    }
                    choice -= weight;
                }
            }
            selected
        };
        self.pair_visits[self.current][next] =
            self.pair_visits[self.current][next].saturating_add(1);
        self.previous = Some(self.current);
        self.current = next;
        if next == FRACTAL_WORLD {
            // Fresh families are requested when Fractal actually returns, not
            // repeatedly while it is hidden. MathJourney owns its six-second
            // endpoint-stable morph, including during this outer-world melt.
            if self.math_seen {
                self.choose_math([energy, brightness, transients]);
            } else {
                self.math_remaining &= !(1 << self.math_family);
                self.math_seen = true;
            }
            self.math_elapsed = 0.0;
        }
        self.remaining_worlds &= !(1 << next);
        self.ages = self
            .ages
            .map(|age| age.saturating_add(1).min(MAX_RETURN_PASSAGES));
        self.ages[next] = 0;
        // Carry fractional overshoot so small render intervals do not make
        // the journey slower or faster at different display refresh rates.
        self.elapsed = (self.elapsed - duration).max(0.0);
        self.evidence = Evidence::default();
        self.evidence.add(levels, self.elapsed);
        self.hold_jitter = self.draw() * 6.0 - 3.0;
        self.mix_seed = (self.draw() * 1000.0) as f32;
        self.morph_duration = (7.0 + self.draw() * 2.0) as f32;
        Some(next)
    }

    fn choice_keeps_deadlines(&self, world: usize) -> bool {
        if self.remaining_worlds & (1 << world) == 0 || self.ages[world] >= MAX_RETURN_PASSAGES {
            return false;
        }
        // Oldest-first is only a feasibility check, never the selected order.
        // An all-world pool has a safe ordering within WORLD_COUNT requests;
        // three extra requests leave room for varied pool boundaries.
        let remaining = self.remaining_worlds & !(1 << world);
        let mut deadlines: Vec<u8> = (0..WORLD_COUNT)
            .filter(|index| remaining & (1 << index) != 0)
            .map(|index| self.ages[index])
            .collect();
        deadlines.sort_unstable_by(|a, b| b.cmp(a));
        for (ahead, age) in deadlines.into_iter().enumerate() {
            if usize::from(age) + 2 + ahead > usize::from(MAX_RETURN_PASSAGES) {
                return false;
            }
        }
        true
    }

    fn advance_math(&mut self, levels: [f64; 3], dt: f64) {
        self.math_elapsed += dt;
        if self.math_elapsed + 1e-9 < self.math_duration {
            return;
        }
        self.math_elapsed = (self.math_elapsed - self.math_duration).max(0.0);
        self.choose_math(levels);
    }

    fn choose_math(&mut self, levels: [f64; 3]) {
        let [energy, brightness, transients] = levels;
        let mut weights = [
            0.9 + energy * 0.3 + (1.0 - brightness) * 0.3,
            0.8 + brightness * 0.6 + (1.0 - energy) * 0.3,
            0.8 + (1.0 - transients) * 0.5 + (1.0 - brightness) * 0.25,
            0.8 + transients * 0.7 + energy * 0.4,
        ];
        if self.math_remaining & !(1 << self.math_family) == 0 {
            self.math_remaining = ALL_MATH_FAMILIES & !(1 << self.math_family);
        }
        for (family, weight) in weights.iter_mut().enumerate() {
            if family == self.math_family || self.math_remaining & (1 << family) == 0 {
                *weight = 0.0;
            }
        }
        let mut choice = self.draw_math() * weights.iter().sum::<f64>();
        for (family, weight) in weights.into_iter().enumerate() {
            if weight > 0.0 {
                self.math_family = family;
                if choice < weight {
                    break;
                }
                choice -= weight;
            }
        }
        self.math_remaining &= !(1 << self.math_family);
        self.math_duration =
            (12.0 + self.draw_math() * 6.0 - energy * 0.5 - transients * 0.25).clamp(12.0, 18.0);
    }

    fn draw(&mut self) -> f64 {
        sample(&mut self.random)
    }

    fn draw_math(&mut self) -> f64 {
        sample(&mut self.math_random)
    }
}

fn sample(state: &mut u64) -> f64 {
    // Separate xorshift64* lanes for worlds, families and startup:
    // visual exploration, not security randomness.
    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    ((state.wrapping_mul(0x2545f4914f6cdd1d) >> 11) as f64) / 9_007_199_254_740_992.0
}

fn mapped_levels(features: &AudioFeatures, mappings: &AudioMappings) -> [f64; 3] {
    fn bounded(value: f32, maximum: f32) -> f64 {
        if value.is_finite() {
            f64::from(value.clamp(0.0, maximum))
        } else {
            0.0
        }
    }
    let bass = bounded(features.bass, 1.0) * bounded(mappings.bass_to_pulse, 1.5);
    let mids = bounded(features.mids, 1.0) * bounded(mappings.mids_to_geometry, 1.5);
    let air = bounded(features.treble, 1.0) * bounded(mappings.treble_to_sparkle, 1.5);
    let total = bass + mids + air;
    let energy = (total / 3.0).clamp(0.0, 1.0);
    let brightness = if total > 0.001 { air / total } else { 0.0 };
    let transients =
        (bounded(features.transient, 1.0) * bounded(mappings.beat_to_flash, 1.5)).min(1.0);
    [energy, brightness, transients]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loud() -> AudioFeatures {
        AudioFeatures {
            bass: 1.0,
            mids: 1.0,
            treble: 1.0,
            energy: 1.0,
            transient: 1.0,
            ..Default::default()
        }
    }

    fn requests(seed: u64, hz: usize, audio: &AudioFeatures) -> Vec<(usize, f64)> {
        let mut journey = AutoJourney::new_with_seed(0, seed);
        let mut result = Vec::new();
        for frame in 1..=hz * 960 {
            if let Some(world) = journey.update(audio, &AudioMappings::default(), 1.0 / hz as f32) {
                result.push((world, frame as f64 / hz as f64));
            }
        }
        result
    }

    #[test]
    fn countdown_uses_the_actual_request_deadline_and_resets_after_each_request() {
        for hz in [30, 60, 120] {
            let mut journey = AutoJourney::new_with_seed(0, 73);
            let quiet = AudioFeatures::default();
            let mappings = AudioMappings::default();
            let dt = 1.0 / hz as f32;
            let mut requests = 0;
            assert_eq!(journey.next_world_progress().0, 0.0);
            for _ in 0..hz * 120 {
                let (progress, remaining) = journey.next_world_progress();
                assert!((0.0..=1.0).contains(&progress));
                assert!(
                    (remaining as f64 - (journey.request_after_seconds() - journey.elapsed)).abs()
                        < 2e-6
                );
                let requested = journey.update(&quiet, &mappings, dt).is_some();
                let (next_progress, next_remaining) = journey.next_world_progress();
                if requested {
                    requests += 1;
                    assert!(
                        remaining <= dt + 2e-6,
                        "{hz}Hz countdown must reach the real request"
                    );
                    assert!(next_progress < dt / 18.0 + 1e-6);
                    assert!(next_remaining > 17.0);
                } else {
                    assert!((remaining - next_remaining - dt).abs() < 6e-6);
                    assert!(next_progress >= progress);
                }
            }
            assert!(requests >= 3);
        }
    }

    #[test]
    fn countdown_observes_music_evidence_without_mutating_the_journey() {
        let mut journey = AutoJourney::new_with_seed(3, 12);
        let before = journey.next_world_progress();
        journey.update(&loud(), &AudioMappings::default(), 0.25);
        let snapshot = (
            journey.random,
            journey.math_random,
            journey.elapsed,
            journey.evidence.seconds,
            journey.mix_seed,
        );
        let timing = journey.next_world_progress();
        assert!(timing.0 > 0.0 && timing.1 < before.1 - 0.25);
        for _ in 0..8 {
            assert_eq!(journey.next_world_progress(), timing);
        }
        assert_eq!(
            snapshot,
            (
                journey.random,
                journey.math_random,
                journey.elapsed,
                journey.evidence.seconds,
                journey.mix_seed,
            )
        );
        journey.reset_evidence();
        assert!(journey.next_world_progress().1 > timing.1);
        journey.set_current(8);
        assert_eq!(journey.next_world_progress().0, 0.0);
    }

    #[test]
    fn mandelbrot_extended_visit_uses_the_real_countdown() {
        use crate::scene::SceneTransition;

        for hz in [30, 60, 120] {
            for audio in [AudioFeatures::default(), loud()] {
                let mut journey = AutoJourney::new_with_seed(MANDELBROT_WORLD, 81);
                let mut scene = SceneTransition::new(0);
                scene.request(MANDELBROT_WORLD);
                let incoming_morph = journey.morph_seconds();
                let mappings = AudioMappings::default();
                let dt = 1.0 / hz as f32;
                let mut departed = false;
                for frame in 1..=hz * 71 {
                    let remaining = journey.next_world_progress().1;
                    if let Some(next) = journey.update(&audio, &mappings, dt) {
                        assert!(
                            (64.0..=70.0 + f64::from(dt)).contains(&(frame as f64 / hz as f64))
                        );
                        assert!(
                            remaining <= dt + 8e-6,
                            "visible countdown must predict this frame"
                        );
                        assert_eq!(scene.amount, 1.0, "complete the incoming world melt");
                        assert_ne!(next, MANDELBROT_WORLD);
                        assert!((7.0..=9.0).contains(&journey.morph_seconds()));
                        assert!(journey.next_world_progress().0 <= dt / 18.0 + 1e-6);
                        scene.request(next);
                        assert_eq!(
                            (scene.from, scene.to, scene.amount),
                            (MANDELBROT_WORLD, next, 0.0)
                        );
                        departed = true;
                        break;
                    }
                    scene.advance_over(dt, incoming_morph);
                }
                assert!(departed);
            }
        }
    }

    #[test]
    fn silent_journey_visits_every_world_without_repeats_or_fixed_cycle() {
        let first = requests(83, 4, &AudioFeatures::default());
        let second = requests(192, 4, &AudioFeatures::default());
        assert!(first.len() >= 10);
        assert_eq!(
            first
                .iter()
                .fold(1_u32, |mask, (world, _)| mask | 1 << world),
            (1_u32 << WORLD_COUNT) - 1
        );
        assert!(first.windows(2).all(|pair| pair[0].0 != pair[1].0));
        assert_ne!(
            first.iter().map(|item| item.0).collect::<Vec<_>>(),
            second.iter().map(|item| item.0).collect::<Vec<_>>()
        );
        let mut previous = 0.0;
        let mut held = 0;
        for (next, time) in first {
            let interval = if held == MANDELBROT_WORLD {
                64.0..=70.25
            } else {
                18.0..=24.25
            };
            assert!(interval.contains(&(time - previous)));
            previous = time;
            held = next;
        }
        let order: Vec<_> = requests(83, 4, &AudioFeatures::default())
            .into_iter()
            .map(|item| item.0)
            .collect();
        assert!(
            order
                .chunks_exact(WORLD_COUNT)
                .zip(order.chunks_exact(WORLD_COUNT).skip(1))
                .any(|(a, b)| a != b),
            "coverage must not repeat a fixed world tour"
        );
    }

    #[test]
    fn startup_draws_keep_all_worlds_in_the_weighted_pool() {
        let mut first_counts = [0; WORLD_COUNT];
        const SAMPLES: usize = 4096;
        for seed in 1..=SAMPLES as u64 {
            let mut journey = AutoJourney::new_with_seed(0, seed);
            journey.elapsed = journey.request_after_seconds() + 1.0;
            let first = journey
                .update(&AudioFeatures::default(), &AudioMappings::default(), 0.01)
                .unwrap();
            first_counts[first] += 1;
        }
        assert_eq!(first_counts[0], 0, "never repeat the initial Flight scene");
        // Every remaining world shares the initial weighted draw. Scale the
        // sampling bounds with that pool, not a historical number of worlds.
        let expected = SAMPLES as f64 / (WORLD_COUNT - 1) as f64;
        for (world, count) in first_counts.into_iter().enumerate().skip(1) {
            assert!(
                (expected * 0.55..=expected * 1.95).contains(&(count as f64)),
                "world {world} must have a fair startup share, got {count}/{SAMPLES}"
            );
        }
    }

    #[test]
    fn auto_startup_varies_and_avoids_last_world_while_manual_startup_restores_it() {
        for previous in 0..WORLD_COUNT {
            let mut starts = [0; WORLD_COUNT];
            for seed in 1..=512 {
                let auto = AutoJourney::startup_with_seed(previous, true, seed);
                let repeat = AutoJourney::startup_with_seed(previous, true, seed);
                let manual = AutoJourney::startup_with_seed(previous, false, seed);
                assert_ne!(auto.current_world(), previous);
                assert_eq!(auto.current_world(), repeat.current_world());
                assert_eq!(
                    auto.remaining_worlds,
                    COVERAGE_WORLDS & !(1 << auto.current_world())
                );
                assert_eq!(manual.current_world(), previous);
                assert_eq!(auto.elapsed, 0.0);
                starts[auto.current_world()] += 1;
            }
            assert_eq!(starts[previous], 0);
            let expected = 512.0 / (WORLD_COUNT - 1) as f64;
            for (world, count) in starts.into_iter().enumerate() {
                if world != previous {
                    assert!(
                        (expected * 0.65..=expected * 1.85).contains(&(count as f64)),
                        "last {previous}, start {world}: {count}/512"
                    );
                }
            }
        }
    }

    #[test]
    fn timed_morphs_keep_ordinary_worlds_fair_and_give_mandelbrot_longer_exposure() {
        use crate::scene::SceneTransition;

        for seed in [1, 11, 77, 512] {
            for features in [AudioFeatures::default(), loud()] {
                let mut journey = AutoJourney::startup_with_seed(0, true, seed);
                let mut scene = SceneTransition::new(journey.current_world());
                let mut morph = journey.morph_seconds();
                let mut nominal_exposure = [0.0_f64; WORLD_COUNT];
                let mut dominant_seconds = [0.0_f64; WORLD_COUNT];
                let mut settled_seconds = [0.0_f64; WORLD_COUNT];
                let mut requests = 0;
                let dt = 1.0 / 30.0;
                for _ in 0..30 * WORLD_COUNT * 24 * 25 {
                    if let Some(next) = journey.update(&features, &AudioMappings::default(), dt) {
                        assert_eq!(scene.amount, 1.0, "Auto cannot preempt a visible morph");
                        morph = journey.morph_seconds();
                        scene.request(next);
                        requests += 1;
                    }
                    scene.advance_over(dt, morph);
                    let blend = f64::from(scene.blend());
                    nominal_exposure[scene.from] += f64::from(dt) * (1.0 - blend);
                    nominal_exposure[scene.to] += f64::from(dt) * blend;
                    dominant_seconds[if blend >= 0.5 { scene.to } else { scene.from }] +=
                        f64::from(dt);
                    if scene.amount == 1.0 {
                        settled_seconds[scene.to] += f64::from(dt);
                    }
                    if requests == WORLD_COUNT * 24 {
                        break;
                    }
                }
                assert_eq!(requests, WORLD_COUNT * 24);
                for (label, times) in [
                    ("nominal blend exposure", nominal_exposure),
                    ("dominant world", dominant_seconds),
                    ("fully settled", settled_seconds),
                ] {
                    let mean = times
                        .iter()
                        .enumerate()
                        .filter(|(world, _)| *world != MANDELBROT_WORLD)
                        .map(|(_, seconds)| seconds)
                        .sum::<f64>()
                        / (WORLD_COUNT - 1) as f64;
                    for (world, seconds) in times.into_iter().enumerate() {
                        if world == MANDELBROT_WORLD {
                            let ratio = if label == "fully settled" {
                                3.8..=5.4
                            } else {
                                2.8..=3.6
                            };
                            assert!(
                                ratio.contains(&(seconds / mean)),
                                "{label}: Mandelbrot must have room for its longer journey"
                            );
                            continue;
                        }
                        assert!(
                            (mean * 0.85..=mean * 1.15).contains(&seconds),
                            "seed {seed}, {label}, world {world}: {seconds:.2}s vs {mean:.2}s mean"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn water_participates_in_auto_coverage_and_can_be_the_resumed_world() {
        let mut journey = AutoJourney::new_with_seed(8, 2016);
        assert_eq!(journey.current, 8);
        assert_eq!(journey.remaining_worlds & (1 << 8), 0);
        let mut water_visits = 0;
        for _ in 0..usize::from(MAX_RETURN_PASSAGES) * 4 {
            journey.elapsed = journey.request_after_seconds() + 1.0;
            if journey.update(&loud(), &AudioMappings::default(), 0.01) == Some(8) {
                water_visits += 1;
            }
        }
        assert!(water_visits >= 4, "water cannot starve behind other media");
        journey.set_current(8);
        assert_eq!(journey.current, 8);
        assert_eq!(journey.elapsed, 0.0);
        assert_eq!(journey.ages[8], 0);
    }

    #[test]
    fn every_world_including_deep_flight_returns_within_a_bounded_random_journey() {
        for seed in 1..=512 {
            for features in [AudioFeatures::default(), loud()] {
                let mut journey =
                    AutoJourney::new_with_seed((seed % WORLD_COUNT as u64) as usize, seed);
                let mut last_seen = [0_u16; WORLD_COUNT];
                let mut previous = journey.current;
                let mut coverage = 1_u32 << journey.current;
                for passage in 1..=200_u16 {
                    // Exercise selection with deterministic passage boundaries;
                    // timing/refresh equivalence is covered independently.
                    journey.elapsed = journey.request_after_seconds() + 1.0;
                    let world = journey
                        .update(&features, &AudioMappings::default(), 0.01)
                        .unwrap();
                    assert_ne!(world, previous, "no immediate world repeats");
                    previous = world;
                    coverage |= 1 << world;
                    assert!(
                        passage - last_seen[world] <= u16::from(MAX_RETURN_PASSAGES),
                        "seed {seed}: world {world} was absent too long"
                    );
                    last_seen[world] = passage;
                    for (world, previous) in last_seen.iter().enumerate() {
                        assert!(
                            passage - previous < u16::from(MAX_RETURN_PASSAGES),
                            "seed {seed}: world {world} starved at passage {passage}"
                        );
                    }
                    if journey.remaining_worlds == 0 {
                        assert_eq!(coverage, (1 << WORLD_COUNT) - 1);
                        coverage = 0;
                    }
                }
            }
        }
    }

    #[test]
    fn flight_is_fair_not_a_privileged_every_fourth_visit() {
        for seed in 1..=64 {
            let mut journey = AutoJourney::new_with_seed(0, seed);
            let mut counts = [0; WORLD_COUNT];
            counts[0] = 1;
            let mut last_flight = 0_u16;
            let mut observed_gaps = 0_u32;
            for passage in 1..=(WORLD_COUNT as u16 * 30 - 1) {
                journey.elapsed = journey.request_after_seconds() + 1.0;
                let world = journey
                    .update(&AudioFeatures::default(), &AudioMappings::default(), 0.01)
                    .unwrap();
                counts[world] += 1;
                if world == 0 {
                    let gap = passage - last_flight;
                    assert!((2..=u16::from(MAX_RETURN_PASSAGES)).contains(&gap));
                    observed_gaps |= 1 << gap;
                    last_flight = passage;
                }
                assert!(
                    counts.iter().max().unwrap() - counts.iter().min().unwrap() <= 1,
                    "every prefix stays balanced, seed {seed}: {counts:?}"
                );
            }
            assert_eq!(counts, [30; WORLD_COUNT]);
            assert!(
                observed_gaps.count_ones() >= 3,
                "seed {seed}: tunnel returns cannot form a fixed cycle"
            );
        }
    }

    #[test]
    fn world_pair_memory_strongly_prefers_an_unseen_transition_when_safe() {
        let mut novel_choices = 0;
        for seed in 1..=128 {
            let mut journey = AutoJourney::new_with_seed(0, seed);
            // Every destination is deadline-safe, but only 0 -> 2 is new.
            journey.pair_visits[0] = [12; WORLD_COUNT];
            journey.pair_visits[0][2] = 0;
            journey.elapsed = journey.request_after_seconds() + 1.0;
            if journey.update(&AudioFeatures::default(), &AudioMappings::default(), 0.01) == Some(2)
            {
                novel_choices += 1;
            }
        }
        assert!(
            novel_choices > 105,
            "novel safe pairs must dominate repeats"
        );
    }

    #[test]
    fn resuming_auto_preserves_exploration_memory_and_counts_the_manual_world() {
        let mut journey = AutoJourney::new_with_seed(0, 28);
        for _ in 0..5 {
            journey.elapsed = journey.request_after_seconds() + 1.0;
            journey.update(&AudioFeatures::default(), &AudioMappings::default(), 0.01);
        }
        let pairs = journey.pair_visits;
        let remaining = journey.remaining_worlds;
        let ages = journey.ages;
        journey.set_current(7);
        assert_eq!(journey.pair_visits, pairs);
        assert_eq!(journey.remaining_worlds, remaining & !(1 << 7));
        assert_eq!(journey.ages[..7], ages[..7]);
        assert_eq!(journey.ages[7], 0);
        assert_eq!(journey.elapsed, 0.0);
    }

    #[test]
    fn each_outer_morph_finishes_before_the_next_world_is_requested() {
        for seed in 1..=32 {
            let mut journey = AutoJourney::new_with_seed(0, seed);
            let mut since_last = 0.0_f64;
            let mut active_morph = journey.morph_seconds();
            let mut held = journey.current_world();
            for _ in 0..60 * 300 {
                since_last += 1.0 / 60.0;
                if journey
                    .update(&loud(), &AudioMappings::default(), 1.0 / 60.0)
                    .is_some()
                {
                    // Fractional overshoot can shorten the measured interval
                    // by one frame, but never overlap an unfinished passage.
                    let (minimum, maximum) = if held == MANDELBROT_WORLD {
                        (64.0, 70.0)
                    } else {
                        (18.0, 24.0)
                    };
                    assert!((minimum - 1.0 / 60.0..=maximum + 1.0 / 60.0).contains(&since_last));
                    assert!(since_last + 1.0 / 60.0 >= f64::from(active_morph) + 9.0);
                    assert!((7.0..=9.0).contains(&journey.morph_seconds()));
                    active_morph = journey.morph_seconds();
                    held = journey.current_world();
                    since_last = 0.0;
                }
            }
        }
    }

    #[test]
    fn zero_mappings_disconnect_all_audio_from_choices_and_timing() {
        let mappings = AudioMappings {
            bass_to_pulse: 0.0,
            mids_to_geometry: 0.0,
            treble_to_sparkle: 0.0,
            beat_to_flash: 0.0,
        };
        let mut quiet = AutoJourney::new_with_seed(2, 92);
        let mut music = AutoJourney::new_with_seed(2, 92);
        for _ in 0..1200 {
            assert_eq!(
                quiet.update(&AudioFeatures::default(), &mappings, 0.25),
                music.update(&loud(), &mappings, 0.25)
            );
            assert_eq!(quiet.random, music.random);
            assert_eq!(quiet.elapsed, music.elapsed);
            assert_eq!(quiet.math_form(), music.math_form());
            assert_eq!(quiet.blend_seed(), music.blend_seed());
            assert_eq!(quiet.morph_seconds(), music.morph_seconds());
        }
    }

    #[test]
    fn constant_music_converges_at_30_60_and_120_hz() {
        let reference = requests(48, 120, &loud());
        for hz in [30, 60] {
            let measured = requests(48, hz, &loud());
            assert_eq!(reference.len(), measured.len());
            for (left, right) in reference.iter().zip(measured) {
                assert_eq!(left.0, right.0);
                assert!((left.1 - right.1).abs() <= 1.0 / hz as f64 + 0.001);
            }
        }
    }

    #[test]
    fn slow_math_family_intervals_match_across_refresh_rates() {
        let run = |seed, hz: usize, audio: &AudioFeatures| {
            let mut journey = AutoJourney::new_with_seed(FRACTAL_WORLD, seed);
            let mut previous = journey.math_form();
            let mut previous_time = 0.0;
            let mut changes = Vec::new();
            for frame in 1..=hz * 240 {
                // Isolate the visible family clock from outer-world departures.
                journey.elapsed = 0.0;
                journey.update(audio, &AudioMappings::default(), 1.0 / hz as f32);
                if journey.math_form() != previous {
                    let time = frame as f64 / hz as f64;
                    let tolerance = 1.0 / hz as f64 + 1e-6;
                    assert!(
                        (12.0 - tolerance..=18.0 + tolerance).contains(&(time - previous_time))
                    );
                    changes.push((journey.math_form(), time));
                    previous = journey.math_form();
                    previous_time = time;
                }
            }
            assert!(changes.len() >= 13);
            changes
        };
        for seed in 1..=8 {
            for audio in [AudioFeatures::default(), loud()] {
                let reference = run(seed, 120, &audio);
                for hz in [30, 60] {
                    let measured = run(seed, hz, &audio);
                    assert_eq!(reference.len(), measured.len());
                    for (a, b) in reference.iter().zip(measured) {
                        assert_eq!(a.0, b.0);
                        assert!((a.1 - b.1).abs() <= 1.0 / hz as f64 + 0.001);
                    }
                }
            }
        }
    }

    #[test]
    fn music_changes_pacing_and_math_lane_runs_independently() {
        let quiet = requests(62, 60, &AudioFeatures::default());
        // Compare the same held world: music may choose Mandelbrot earlier in
        // a tour, so two different world sequences need not depart earlier at
        // every cumulative step now that Mandelbrot has a longer dwell.
        for world in 0..WORLD_COUNT {
            let first_request = |audio: &AudioFeatures| {
                let mut journey = AutoJourney::new_with_seed(world, 62);
                (1..=60 * 71)
                    .find(|_| {
                        journey
                            .update(audio, &AudioMappings::default(), 1.0 / 60.0)
                            .is_some()
                    })
                    .expect("every held world eventually departs")
            };
            let quiet = first_request(&AudioFeatures::default());
            let music = first_request(&loud());
            assert!(music <= quiet);
            assert!(
                quiet - music <= 46,
                "music only gently adjusts the same-world deadline"
            );
        }
        let raw = AudioFeatures {
            energy: 1.0,
            ..Default::default()
        };
        assert_eq!(requests(62, 60, &raw), quiet);
        let mut first = AutoJourney::new_with_seed(FRACTAL_WORLD, 62);
        let mut offset = AutoJourney::new_with_seed(FRACTAL_WORLD, 62);
        offset.math_elapsed = 2.5;
        let mut previous_math = first.math_form();
        let mut independent_changes = 0;
        for _ in 1..=960 {
            let world = first.update(&loud(), &AudioMappings::default(), 0.25);
            assert_eq!(
                world,
                offset.update(&loud(), &AudioMappings::default(), 0.25)
            );
            assert_eq!(first.blend_seed(), offset.blend_seed());
            assert_eq!(first.morph_seconds(), offset.morph_seconds());
            assert!(first.math_form().family().is_some());
            if first.math_form() != previous_math {
                assert_eq!(first.current, FRACTAL_WORLD);
                if world.is_none() {
                    independent_changes += 1;
                }
            }
            previous_math = first.math_form();
        }
        assert!(independent_changes > 0);
    }

    #[test]
    fn initial_world_counts_as_seen_before_the_first_coverage_pool() {
        for initial in 1..WORLD_COUNT {
            for seed in 1..=32 {
                let mut journey = AutoJourney::new_with_seed(initial, seed);
                assert_eq!(journey.remaining_worlds & (1 << initial), 0);
                let mut seen = 1_u32 << initial;
                for _ in 0..24 {
                    journey.elapsed = journey.request_after_seconds() + 1.0;
                    let next = journey
                        .update(&AudioFeatures::default(), &AudioMappings::default(), 0.01)
                        .unwrap();
                    if next == initial {
                        assert_eq!(seen, (1 << WORLD_COUNT) - 1);
                        break;
                    }
                    seen |= 1 << next;
                }
            }
        }
    }

    #[test]
    fn hidden_math_does_not_advance_or_consume_its_visible_bag() {
        let mut journey = AutoJourney::new_with_seed(4, 72);
        let before = (
            journey.math_family,
            journey.math_random,
            journey.math_remaining,
            journey.math_elapsed,
            journey.math_duration,
        );
        for _ in 0..4 * 180 {
            journey.elapsed = 0.0;
            journey.update(&loud(), &AudioMappings::default(), 0.25);
        }
        assert_eq!(
            before,
            (
                journey.math_family,
                journey.math_random,
                journey.math_remaining,
                journey.math_elapsed,
                journey.math_duration,
            )
        );
    }

    #[test]
    fn returning_fractal_exposes_all_four_families_before_repeating() {
        for seed in 1..=64 {
            let mut journey = AutoJourney::new_with_seed(4, seed);
            let mut seen = 0_u8;
            for _ in 0..4 {
                journey.set_current(2);
                // Isolate entry choices; ordinary coverage is tested separately.
                journey.remaining_worlds = 1 << FRACTAL_WORLD;
                journey.ages = [0; WORLD_COUNT];
                journey.elapsed = journey.request_after_seconds() + 1.0;
                assert_eq!(
                    journey.update(&loud(), &AudioMappings::default(), 0.01),
                    Some(FRACTAL_WORLD),
                );
                assert_eq!(seen & (1 << journey.math_family), 0);
                seen |= 1 << journey.math_family;
                let family = journey.math_family;
                journey.set_current(FRACTAL_WORLD);
                assert_eq!(
                    journey.math_family, family,
                    "Auto resume preserves current family"
                );
                assert_eq!(journey.math_elapsed, 0.0);
                for _ in 0..4 * 11 {
                    journey.elapsed = 0.0;
                    journey.update(&loud(), &AudioMappings::default(), 0.25);
                    assert_eq!(
                        journey.math_family, family,
                        "new entry has an unhurried hold"
                    );
                }
            }
            assert_eq!(seen, ALL_MATH_FAMILIES);
        }
    }

    #[test]
    fn reset_preserves_journey_but_manual_resume_starts_a_fresh_hold() {
        let mut journey = AutoJourney::new_with_seed(1, 19);
        for _ in 0..20 {
            journey.update(&loud(), &AudioMappings::default(), 0.25);
        }
        let before = (
            journey.current,
            journey.remaining_worlds,
            journey.ages,
            journey.elapsed,
            journey.random,
            journey.hold_jitter,
            journey.math_elapsed,
            journey.math_duration,
            journey.math_random,
            journey.math_form(),
            journey.blend_seed(),
            journey.morph_seconds(),
        );
        journey.reset_evidence();
        assert_eq!(
            before,
            (
                journey.current,
                journey.remaining_worlds,
                journey.ages,
                journey.elapsed,
                journey.random,
                journey.hold_jitter,
                journey.math_elapsed,
                journey.math_duration,
                journey.math_random,
                journey.math_form(),
                journey.blend_seed(),
                journey.morph_seconds(),
            )
        );
        assert_eq!(journey.evidence.seconds, 0.0);
        journey.set_current(99);
        assert_eq!(journey.current, WORLD_COUNT - 1);
        assert_eq!(
            journey.remaining_worlds,
            before.1 & !(1 << (WORLD_COUNT - 1))
        );
        assert_eq!(journey.ages[WORLD_COUNT - 1], 0);
        assert!(journey.ages.iter().all(|age| *age == 0));
        assert_eq!(journey.elapsed, 0.0);
        assert_eq!(
            journey.update(&loud(), &AudioMappings::default(), 0.25),
            None
        );
    }

    #[test]
    fn invalid_values_are_bounded_and_zero_dt_is_fully_inert() {
        let mut journey = AutoJourney::new_with_seed(usize::MAX, 0);
        let initial_random = journey.random;
        let initial_math = (
            journey.math_random,
            journey.math_form(),
            journey.blend_seed(),
            journey.morph_seconds(),
        );
        for dt in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(journey.update(&loud(), &AudioMappings::default(), dt), None);
            assert_eq!(journey.elapsed, 0.0);
            assert_eq!(journey.random, initial_random);
            assert_eq!(
                (
                    journey.math_random,
                    journey.math_form(),
                    journey.blend_seed(),
                    journey.morph_seconds()
                ),
                initial_math
            );
            assert_eq!(journey.math_elapsed, 0.0);
        }
        let broken = AudioFeatures {
            bass: f32::NAN,
            mids: f32::INFINITY,
            treble: f32::NEG_INFINITY,
            transient: f32::NAN,
            ..Default::default()
        };
        for _ in 0..200 {
            let before = (journey.blend_seed(), journey.morph_seconds());
            if let Some(world) = journey.update(&broken, &AudioMappings::default(), f32::MAX) {
                assert!(world < WORLD_COUNT);
            } else {
                assert_eq!((journey.blend_seed(), journey.morph_seconds()), before);
            }
            assert!((7.0..=9.0).contains(&journey.morph_seconds()));
            assert!((12.0..=18.0).contains(&journey.math_duration));
            assert!(journey.ages.iter().all(|age| *age <= MAX_RETURN_PASSAGES));
            assert!(journey.elapsed.is_finite());
            assert!(journey.evidence.sums.iter().all(|value| value.is_finite()));
        }
    }
}
