//! Independent per-world structural-look timelines. These are visual compositions,
//! not genre labels, detected song sections, or an AI in the render loop.
use crate::{
    dsp::AudioFeatures,
    presets::{AudioMappings, WORLD_COUNT},
};
use std::time::{SystemTime, UNIX_EPOCH};

pub const LOOK_COUNT: usize = 10;
const ALL_LOOKS: u16 = (1 << LOOK_COUNT) - 1;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Exposure {
    Hidden,
    Departing,
    Destination,
}

struct Lane {
    from: usize,
    to: usize,
    amount: f64,
    elapsed: f64,
    duration: f64,
    morph: f64,
    phase: f64,
    ages: [u8; LOOK_COUNT],
    remaining: u16,
    pair_visits: [[u16; LOOK_COUNT]; LOOK_COUNT],
    last_seen: Option<usize>,
    was_visible: bool,
    prepare_when_hidden: bool,
    random: u64,
}

impl Lane {
    fn new(seed: u64) -> Self {
        let mut lane = Self {
            from: 0,
            to: 0,
            amount: 1.0,
            elapsed: 0.0,
            duration: 15.0,
            morph: 7.0,
            phase: 0.0,
            ages: [10; LOOK_COUNT],
            remaining: ALL_LOOKS,
            pair_visits: [[0; LOOK_COUNT]; LOOK_COUNT],
            last_seen: None,
            was_visible: false,
            prepare_when_hidden: false,
            random: seed.max(1),
        };
        lane.duration = 12.0 + lane.draw() * 6.0;
        lane.to = (lane.draw() * LOOK_COUNT as f64) as usize;
        lane.from = lane.to;
        lane
    }

    fn draw(&mut self) -> f64 {
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        (self.random >> 11) as f64 / ((1_u64 << 53) as f64)
    }

    fn choose(&mut self, drive: f64) {
        if self.remaining & !(1 << self.to) == 0 {
            self.remaining = ALL_LOOKS & !(1 << self.to);
        }
        let least_used_pair = (0..LOOK_COUNT)
            .filter(|index| self.remaining & (1 << index) != 0 && *index != self.to)
            .map(|index| self.pair_visits[self.to][index])
            .min()
            .unwrap_or(0);
        let weights: [f64; LOOK_COUNT] = std::array::from_fn(|index| {
            if index == self.to || self.remaining & (1 << index) == 0 {
                0.0
            } else {
                let repeated_pair = self.pair_visits[self.to][index]
                    .saturating_sub(least_used_pair)
                    .min(3);
                (0.35 + f64::from(self.ages[index]) * 0.12)
                    * 0.22_f64.powi(i32::from(repeated_pair))
            }
        });
        let mut ticket = self.draw() * weights.iter().sum::<f64>();
        let mut next = (self.to + 1) % LOOK_COUNT;
        for (index, weight) in weights.into_iter().enumerate() {
            if weight > 0.0 {
                next = index;
                if ticket < weight {
                    break;
                }
                ticket -= weight;
            }
        }
        self.from = self.to;
        self.to = next;
        self.amount = 0.0;
        self.elapsed = 0.0;
        self.morph = 6.0 + self.draw() * 2.0;
        self.duration = (12.0 + self.draw() * 6.0 - drive * 0.5).clamp(12.0, 18.0);
    }

    fn mark_seen(&mut self) {
        if self.last_seen == Some(self.to) {
            return;
        }
        if let Some(previous) = self.last_seen {
            self.pair_visits[previous][self.to] =
                self.pair_visits[previous][self.to].saturating_add(1);
        }
        self.remaining &= !(1 << self.to);
        self.ages = self.ages.map(|age| age.saturating_add(1).min(16));
        self.ages[self.to] = 0;
        self.last_seen = Some(self.to);
    }

    fn update(
        &mut self,
        dt: f64,
        auto: bool,
        requested: Option<usize>,
        drive: f64,
        exposure: Exposure,
    ) -> [f32; 4] {
        if dt <= 0.0 {
            return self.uniforms();
        }
        self.phase += dt;
        let visible = exposure != Exposure::Hidden;
        if visible {
            if !self.was_visible {
                // A new visit gets a full, readable hold, not the last fraction
                // of a timer that expired while nobody could see this world.
                self.elapsed = 0.0;
            }
            self.prepare_when_hidden = false;
            self.mark_seen();
        } else if self.was_visible {
            self.prepare_when_hidden = true;
        }
        self.was_visible = visible;
        if auto && exposure == Exposure::Destination {
            // Only the incoming/current world starts new looks. The departing
            // world finishes any existing morph but does not spend new designs
            // on its last, increasingly faint seconds.
            let mut remaining = dt;
            while remaining > 0.0 {
                let until_settled = (1.0 - self.amount) * self.morph;
                let until_choice = (self.duration - self.elapsed).max(0.0);
                let boundary = until_settled.max(until_choice);
                if remaining + 1e-9 < boundary {
                    self.amount = (self.amount + remaining / self.morph).min(1.0);
                    self.elapsed += remaining;
                    break;
                }
                // Carry fractional time across the exact selection boundary.
                // Otherwise each frame rate loses a different amount of time
                // on every choice and eventually explores a different sequence.
                remaining = (remaining - boundary).max(0.0);
                self.choose(drive);
                self.mark_seen();
            }
        } else {
            self.amount = (self.amount + dt / self.morph).min(1.0);
            if !auto && let Some(next) = requested {
                let next = next.min(LOOK_COUNT - 1);
                // Keep active endpoints stable; the latest manual choice is read
                // again after this morph settles, without a visual teleport.
                if self.amount >= 1.0 && next != self.to {
                    self.from = self.to;
                    self.to = next;
                    self.amount = 0.0;
                    self.morph = 7.0;
                    if visible {
                        self.mark_seen();
                    }
                }
            }
            if auto && self.prepare_when_hidden && self.amount >= 1.0 {
                // Prepare exactly one fresh entry only after the old world is
                // entirely absent. The world-to-world melt reveals this design;
                // never jump endpoints in a world that is still on screen.
                self.choose(drive);
                self.from = self.to;
                self.amount = 1.0;
                self.prepare_when_hidden = false;
            }
        }
        self.uniforms()
    }

    fn uniforms(&self) -> [f32; 4] {
        let t = self.amount as f32;
        let blend = (t * t * t * (t * (t * 6.0 - 15.0) + 10.0)).clamp(0.0, 1.0);
        [self.from as f32, self.to as f32, blend, self.phase as f32]
    }
}

pub struct LookJourney {
    lanes: [Lane; WORLD_COUNT],
    visible_pair: Option<(usize, usize, f32)>,
    mandelbrot_camera_driven: bool,
}

impl LookJourney {
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |time| time.as_nanos() as u64);
        Self::seeded(seed)
    }

    pub(crate) fn seeded(seed: u64) -> Self {
        Self {
            lanes: std::array::from_fn(|index| {
                Lane::new(
                    seed.rotate_left(index as u32 * 13)
                        ^ (0x9e3779b97f4a7c15_u64.wrapping_mul(index as u64 + 1)),
                )
            }),
            visible_pair: None,
            mandelbrot_camera_driven: false,
        }
    }

    pub fn current(&self, scene: usize) -> usize {
        self.lanes[scene.min(WORLD_COUNT - 1)].to
    }

    /// The app's physical Mandelbrot camera owns visible region changes in
    /// both Auto and manual modes. Hidden preparation still uses this lane's
    /// shuffle bag; standalone callers without the camera keep ordinary timing.
    pub fn guide_mandelbrot_with_camera(&mut self) {
        self.mandelbrot_camera_driven = true;
    }

    /// A held Mandelbrot journey may request a new region only while its
    /// camera is wide. Reuse the visible shuffle bag and the ordinary smooth
    /// lane; an unfinished explicit user choice is never replaced.
    pub fn next_manual_mandelbrot_region(&mut self, selected: usize) -> Option<usize> {
        let lane = &mut self.lanes[10];
        if lane.amount < 1.0 || lane.to != selected {
            return None;
        }
        lane.choose(0.0);
        lane.morph = 7.0;
        lane.mark_seen();
        Some(lane.to)
    }

    /// Pass the actual outer blend before update, not merely its destination.
    /// Both endpoints count as exposed during the melt.
    pub fn set_visible_pair(&mut self, from: usize, to: usize, blend: f32) {
        self.visible_pair = Some((
            from.min(WORLD_COUNT - 1),
            to.min(WORLD_COUNT - 1),
            if blend.is_finite() {
                blend.clamp(0.0, 1.0)
            } else {
                0.0
            },
        ));
    }

    /// Keep the user's current composition and any active morph intact when
    /// enabling Auto, but give it a fresh hold before the next variation.
    pub fn resume_auto(&mut self, scene: usize) {
        let lane = &mut self.lanes[scene.min(WORLD_COUNT - 1)];
        lane.elapsed = 0.0;
        lane.prepare_when_hidden = false;
    }

    /// Feed physical elapsed seconds, not the Pace-scaled travel clock. Each
    /// design gets a 12–18 second passage and a continuous 6–8 second Auto morph.
    pub fn update(
        &mut self,
        audio: &AudioFeatures,
        mappings: &AudioMappings,
        auto: bool,
        scene: usize,
        selected: usize,
        dt: f32,
    ) -> [[f32; 4]; WORLD_COUNT] {
        let dt = if dt.is_finite() {
            f64::from(dt.clamp(0.0, 0.25))
        } else {
            0.0
        };
        let drive = f64::from(
            (unit(audio.bass) * unit(mappings.bass_to_pulse)
                + unit(audio.mids) * unit(mappings.mids_to_geometry)
                + unit(audio.treble) * unit(mappings.treble_to_sparkle))
                / 3.0,
        );
        let scene = scene.min(WORLD_COUNT - 1);
        let (from, to, blend) = self.visible_pair.unwrap_or((scene, scene, 1.0));
        std::array::from_fn(|index| {
            let exposure = if index == to && (blend > 0.0 || from == to) {
                Exposure::Destination
            } else if index == from && blend < 1.0 {
                Exposure::Departing
            } else {
                Exposure::Hidden
            };
            let camera_driven =
                self.mandelbrot_camera_driven && index == 10 && exposure != Exposure::Hidden;
            self.lanes[index].update(
                dt,
                auto && !camera_driven,
                (index == scene && !(auto && camera_driven)).then_some(selected),
                drive,
                exposure,
            )
        })
    }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_world_lanes_explore_without_a_fixed_tour() {
        let mut journey = LookJourney::seeded(42);
        let mut seen = [[false; LOOK_COUNT]; WORLD_COUNT];
        let mut last = [usize::MAX; WORLD_COUNT];
        let mut changes = [0; WORLD_COUNT];
        for frame in 0..4 * 240 * WORLD_COUNT {
            let world = frame / (4 * 240);
            let values = journey.update(
                &AudioFeatures::default(),
                &AudioMappings::default(),
                true,
                world,
                0,
                0.25,
            );
            for (index, value) in values
                .iter()
                .enumerate()
                .filter(|(index, _)| *index == world)
            {
                let id = value[1] as usize;
                seen[index][id] = true;
                if id != last[index] {
                    changes[index] += 1;
                    last[index] = id;
                }
                assert!((0.0..=1.0).contains(&value[2]));
            }
        }
        assert!(seen.iter().all(|lane| lane.iter().all(|seen| *seen)));
        assert!(changes.iter().all(|changes| *changes >= 13));
    }
    #[test]
    fn paused_updates_preserve_phase_and_choices() {
        let mut journey = LookJourney::seeded(93);
        let a = journey.update(
            &AudioFeatures::default(),
            &AudioMappings::default(),
            true,
            0,
            0,
            0.0,
        );
        let b = journey.update(
            &AudioFeatures::default(),
            &AudioMappings::default(),
            true,
            0,
            0,
            f32::NAN,
        );
        assert_eq!(a, b);
        let c = journey.update(
            &AudioFeatures::default(),
            &AudioMappings::default(),
            false,
            0,
            9,
            0.0,
        );
        assert_eq!(a, c);
    }
    #[test]
    fn manual_choices_morph_and_hold() {
        let mut journey = LookJourney::seeded(14);
        let mut last = [[0.0; 4]; WORLD_COUNT];
        for _ in 0..60 * 12 {
            last = journey.update(
                &AudioFeatures::default(),
                &AudioMappings::default(),
                false,
                2,
                7,
                1.0 / 60.0,
            );
        }
        assert_eq!(last[2][1], 7.0);
        assert_eq!(last[2][2], 1.0);
        assert_eq!(journey.current(2), 7);
    }

    #[test]
    fn new_worlds_have_independent_manual_lanes_and_invalid_scene_clamps_safely() {
        let mut journey = LookJourney::seeded(24);
        for scene in 4..WORLD_COUNT {
            for _ in 0..60 * 12 {
                journey.update(
                    &AudioFeatures::default(),
                    &AudioMappings::default(),
                    false,
                    scene,
                    scene + 2,
                    1.0 / 60.0,
                );
            }
            assert_eq!(journey.current(scene), (scene + 2).min(LOOK_COUNT - 1));
        }
        for _ in 0..60 * 12 {
            journey.update(
                &AudioFeatures::default(),
                &AudioMappings::default(),
                false,
                usize::MAX,
                usize::MAX,
                1.0 / 60.0,
            );
        }
        assert_eq!(journey.current(usize::MAX), LOOK_COUNT - 1);
        assert_eq!(journey.current(4), 6);
        assert_eq!(journey.current(5), 7);
        assert_eq!(journey.current(6), 8);
        assert_eq!(journey.current(7), 9);
    }

    #[test]
    fn active_point_world_randomly_explores_all_ten_submodes_during_passages() {
        let mut journey = LookJourney::seeded(42);
        let mut previous = journey.current(4);
        let mut seen = [false; LOOK_COUNT];
        seen[previous] = true;
        let mut changes = Vec::new();
        for frame in 1..=60 * 600 {
            let values = journey.update(
                &AudioFeatures::default(),
                &AudioMappings::default(),
                true,
                4,
                0,
                1.0 / 60.0,
            );
            let look = values[4][1] as usize;
            seen[look] = true;
            if look != previous {
                changes.push((look, frame as f64 / 60.0));
                previous = look;
            }
        }
        assert!(seen.into_iter().all(|seen| seen));
        assert!(changes.len() > 30);
        assert!(
            (12.0..=18.02).contains(&changes[0].1),
            "a dot design remains recognizable before its first gentle change"
        );
        for pair in changes.windows(2) {
            let interval = pair[1].1 - pair[0].1;
            assert!((11.98..=18.02).contains(&interval));
            assert_ne!(pair[0].0, pair[1].0);
        }
        assert!(
            changes
                .windows(3)
                .any(|window| window[0].0 > window[1].0 && window[1].0 < window[2].0),
            "not a fixed ascending tour"
        );
    }

    #[test]
    fn automatic_choices_and_morphs_converge_at_30_60_and_120_hz() {
        let run = |hz: usize| {
            let mut journey = LookJourney::seeded(193);
            let mut choices: [Vec<(usize, f64)>; WORLD_COUNT] = std::array::from_fn(|_| Vec::new());
            let mut previous =
                std::array::from_fn::<_, WORLD_COUNT, _>(|world| journey.current(world));
            let mut last = [[0.0; 4]; WORLD_COUNT];
            let audio = AudioFeatures {
                bass: 0.8,
                mids: 0.5,
                treble: 0.3,
                ..Default::default()
            };
            for frame in 1..=hz * 120 {
                last = journey.update(
                    &audio,
                    &AudioMappings::default(),
                    true,
                    4,
                    0,
                    1.0 / hz as f32,
                );
                for world in 0..WORLD_COUNT {
                    let to = last[world][1] as usize;
                    if to != previous[world] {
                        choices[world].push((to, frame as f64 / hz as f64));
                        previous[world] = to;
                    }
                }
            }
            (choices, last)
        };
        let (reference, reference_state) = run(120);
        for hz in [30, 60] {
            let (measured, measured_state) = run(hz);
            for world in 0..WORLD_COUNT {
                assert_eq!(reference[world].len(), measured[world].len());
                for (a, b) in reference[world].iter().zip(&measured[world]) {
                    assert_eq!(a.0, b.0);
                    assert!((a.1 - b.1).abs() <= 1.0 / hz as f64 + 0.001);
                }
                assert_eq!(reference_state[world][..2], measured_state[world][..2]);
                assert!((reference_state[world][2] - measured_state[world][2]).abs() < 0.0001);
                assert!((reference_state[world][3] - measured_state[world][3]).abs() < 0.001);
            }
        }
    }

    #[test]
    fn every_design_gets_a_slow_morph_and_settled_time_even_on_loud_music() {
        for seed in 1..=8 {
            for level in [0.0, 1.0] {
                let audio = AudioFeatures {
                    bass: level,
                    mids: level,
                    treble: level,
                    ..Default::default()
                };
                let mut journey = LookJourney::seeded(seed);
                let mut previous =
                    std::array::from_fn::<_, WORLD_COUNT, _>(|world| journey.current(world));
                let mut previous_time = [0.0; WORLD_COUNT];
                let mut previous_morph = [7.0; WORLD_COUNT];
                let mut changes = [0; WORLD_COUNT];
                for frame in 1..=60 * 240 {
                    let values =
                        journey.update(&audio, &AudioMappings::default(), true, 4, 0, 1.0 / 60.0);
                    {
                        let world = 4;
                        let next = values[world][1] as usize;
                        if next != previous[world] {
                            let time = frame as f64 / 60.0;
                            let interval = time - previous_time[world];
                            assert!((11.98..=18.02).contains(&interval));
                            assert!(interval + 1.0 / 60.0 >= previous_morph[world] + 4.0);
                            let lane = &journey.lanes[world];
                            assert!((6.0..=8.0).contains(&lane.morph));
                            assert_eq!(lane.from, previous[world]);
                            // The next design starts at the same endpoint with
                            // only the fractional remainder of one physical frame.
                            assert!(lane.amount <= f64::from(1.0_f32 / 60.0) / 6.0 + 1e-8);
                            previous[world] = next;
                            previous_time[world] = time;
                            previous_morph[world] = lane.morph;
                            changes[world] += 1;
                        }
                    }
                }
                assert!(changes[4] >= 13);
                assert!(
                    changes
                        .iter()
                        .enumerate()
                        .all(|(world, count)| world == 4 || *count == 0)
                );
            }
        }
    }

    #[test]
    fn outer_morph_keeps_both_visible_endpoints_continuous() {
        let mut journey = LookJourney::seeded(71);
        for frame in 0..60 * 30 {
            // Scene four dissolves into scene five over eight seconds.
            let blend = ((frame as f32 / 60.0 - 16.0) / 8.0).clamp(0.0, 1.0);
            journey.set_visible_pair(4, 5, blend);
            let before: [_; WORLD_COUNT] = std::array::from_fn(|world| {
                (
                    journey.lanes[world].from,
                    journey.lanes[world].to,
                    journey.lanes[world].amount,
                    journey.lanes[world].morph,
                    journey.lanes[world].phase,
                )
            });
            journey.update(
                &AudioFeatures::default(),
                &AudioMappings::default(),
                true,
                5,
                0,
                1.0 / 60.0,
            );
            for (world, old) in before.iter().enumerate() {
                let lane = &journey.lanes[world];
                assert!((lane.phase - old.4 - f64::from(1.0_f32 / 60.0)).abs() < 1e-10);
                let visible = (world == 4 && blend < 1.0) || (world == 5 && blend > 0.0);
                if visible && old.2 + f64::from(1.0_f32 / 60.0) / old.3 < 1.0 - 1e-8 {
                    assert_eq!((lane.from, lane.to), (old.0, old.1));
                }
            }
        }
    }

    #[test]
    fn hidden_lanes_do_not_burn_choices_and_manual_pending_choices_stay_continuous() {
        let mut first = LookJourney::seeded(9);
        let mut second = LookJourney::seeded(9);
        let hidden_initial = first.lanes[5].uniforms();
        for _ in 0..60 * 20 {
            let a = first.update(
                &AudioFeatures::default(),
                &AudioMappings::default(),
                true,
                4,
                0,
                1.0 / 60.0,
            );
            let b = second.update(
                &AudioFeatures::default(),
                &AudioMappings::default(),
                true,
                5,
                0,
                1.0 / 60.0,
            );
            assert_eq!(a[5][..2], hidden_initial[..2]);
            assert_eq!(a[4][3], b[4][3]);
        }
        assert_eq!(first.lanes[5].last_seen, None);
        assert_eq!(first.lanes[5].remaining, ALL_LOOKS);
        assert_eq!(first.lanes[5].elapsed, 0.0);
        assert!(first.lanes[4].last_seen.is_some());
        assert_eq!(second.lanes[4].last_seen, None);
        let lane = &mut first.lanes[4];
        lane.from = 1;
        lane.to = 2;
        lane.amount = 0.25;
        lane.morph = 4.0;
        lane.update(0.25, false, Some(8), 0.0, Exposure::Destination);
        assert_eq!((lane.from, lane.to), (1, 2));
        for _ in 0..12 {
            lane.update(0.25, false, Some(9), 0.0, Exposure::Destination);
        }
        assert_eq!((lane.from, lane.to), (2, 9));
        for _ in 0..60 {
            lane.update(0.25, false, Some(9), 0.0, Exposure::Destination);
        }
        assert_eq!((lane.to, lane.amount), (9, 1.0));
    }

    #[test]
    fn visible_shuffle_bag_exposes_every_look_before_repeating() {
        for seed in 1..=64 {
            let mut journey = LookJourney::seeded(seed);
            let mut seen = 0_u16;
            let mut previous = usize::MAX;
            let mut visits = 0;
            for _ in 0..4 * 180 {
                let state = journey.update(
                    &AudioFeatures::default(),
                    &AudioMappings::default(),
                    true,
                    4,
                    0,
                    0.25,
                );
                let look = state[4][1] as usize;
                if look != previous {
                    assert_eq!(
                        seen & (1 << look),
                        0,
                        "seed {seed}: visible look repeated early"
                    );
                    seen |= 1 << look;
                    previous = look;
                    visits += 1;
                    if visits == LOOK_COUNT {
                        break;
                    }
                }
            }
            assert_eq!(seen, ALL_LOOKS);
        }
    }

    #[test]
    fn returning_world_prepares_one_new_entry_not_an_invisible_carousel() {
        let mut journey = LookJourney::seeded(55);
        let audio = AudioFeatures::default();
        let mappings = AudioMappings::default();
        for _ in 0..40 {
            journey.update(&audio, &mappings, true, 4, 0, 0.25);
        }
        let old = journey.current(4);
        // The departing world has fully faded: it may now prepare one unseen look.
        journey.set_visible_pair(5, 5, 1.0);
        for _ in 0..4 * 180 {
            journey.update(&audio, &mappings, true, 5, 0, 0.25);
        }
        let ready = journey.current(4);
        assert_ne!(ready, old);
        assert_eq!(journey.lanes[4].last_seen, Some(old));
        assert_ne!(journey.lanes[4].remaining & (1 << ready), 0);
        let before = journey.lanes[4].uniforms();
        for _ in 0..4 * 30 {
            journey.update(&audio, &mappings, true, 5, 0, 0.25);
        }
        assert_eq!(journey.lanes[4].uniforms()[..3], before[..3]);
        journey.set_visible_pair(5, 4, 0.01);
        journey.update(&audio, &mappings, true, 4, 0, 0.25);
        assert_eq!(journey.current(4), ready);
        assert_eq!(journey.lanes[4].last_seen, Some(ready));
        assert!(journey.lanes[4].elapsed < 0.26);
    }

    #[test]
    fn enabling_auto_keeps_manual_dots_and_gives_them_a_fresh_hold() {
        let mut journey = LookJourney::seeded(82);
        let audio = AudioFeatures::default();
        let mappings = AudioMappings::default();
        for _ in 0..4 * 20 {
            journey.update(&audio, &mappings, false, 7, 1, 0.25);
        }
        let before = journey.lanes[7].uniforms();
        journey.resume_auto(7);
        assert_eq!(before, journey.lanes[7].uniforms());
        for _ in 0..4 * 11 {
            journey.update(&audio, &mappings, true, 7, 1, 0.25);
            assert_eq!(journey.current(7), 1);
        }
        assert_eq!(journey.lanes[7].amount, 1.0);
    }

    #[test]
    fn manual_mandelbrot_requests_keep_other_lanes_and_user_choices_intact() {
        let mut journey = LookJourney::seeded(17);
        let before: [_; WORLD_COUNT] = std::array::from_fn(|world| journey.lanes[world].uniforms());
        let selected = (journey.current(10) + 1) % LOOK_COUNT;
        journey.update(
            &AudioFeatures::default(),
            &AudioMappings::default(),
            false,
            10,
            selected,
            0.1,
        );
        assert_eq!(journey.next_manual_mandelbrot_region(selected), None);
        for _ in 0..80 {
            journey.update(
                &AudioFeatures::default(),
                &AudioMappings::default(),
                false,
                10,
                selected,
                0.1,
            );
        }
        assert_eq!(
            journey.next_manual_mandelbrot_region((selected + 1) % LOOK_COUNT),
            None
        );
        let from = journey.lanes[10].uniforms();
        let next = journey.next_manual_mandelbrot_region(selected).unwrap();
        let after = journey.lanes[10].uniforms();
        assert_ne!(next, selected);
        assert_eq!(after[0], from[1]);
        assert_eq!(after[2], 0.0);
        for (world, original) in before.iter().enumerate().take(10) {
            assert_eq!(journey.lanes[world].uniforms()[..3], original[..3]);
        }
    }

    #[test]
    fn camera_driven_mandelbrot_waits_for_its_region_request_and_still_prepares_hidden_entries() {
        let mut journey = LookJourney::seeded(17);
        journey.guide_mandelbrot_with_camera();
        let selected = journey.current(10);
        let audio = AudioFeatures::default();
        let mappings = AudioMappings::default();
        for _ in 0..600 {
            let state = journey.update(
                &audio,
                &mappings,
                true,
                10,
                (selected + 1) % LOOK_COUNT,
                0.1,
            );
            assert_eq!(state[10][1] as usize, selected);
        }
        let next = journey.next_manual_mandelbrot_region(selected).unwrap();
        for _ in 0..80 {
            let state = journey.update(&audio, &mappings, true, 10, selected, 0.1);
            assert_eq!(state[10][1] as usize, next);
        }
        assert_eq!(journey.lanes[10].amount, 1.0);
        journey.set_visible_pair(10, 7, 0.5);
        journey.update(&audio, &mappings, true, 7, 0, 0.1);
        assert_eq!(journey.current(10), next, "departing region stays intact");
        journey.set_visible_pair(10, 7, 1.0);
        journey.update(&audio, &mappings, true, 7, 0, 0.1);
        let prepared = journey.current(10);
        assert_ne!(
            prepared, next,
            "fully hidden next entry gets a fresh region"
        );
        for _ in 0..100 {
            journey.update(&audio, &mappings, true, 7, 0, 0.1);
            assert_eq!(
                journey.current(10),
                prepared,
                "only one hidden entry is prepared"
            );
        }
    }
}
