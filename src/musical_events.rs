//! Sparse, measured-onset ink gestures. This controller emits geometry seeds,
//! never a global brightness/camera multiplier, genre claim or synthetic beat.
use crate::{dsp::AudioFeatures, presets::AudioMappings};

const INACTIVE: [f32; 4] = [1.0, 0.0, 0.0, 0.0];
const SLOTS: usize = 3;

#[derive(Debug, Clone, Copy)]
struct InkEvent {
    seconds: f64,
    duration: f64,
    strength: f32,
    seed: f32,
    kind: usize,
}

impl Default for InkEvent {
    fn default() -> Self {
        Self {
            seconds: 0.0,
            duration: 1.0,
            strength: 0.0,
            seed: 0.0,
            kind: 0,
        }
    }
}

pub struct MusicalEvents {
    events: [InkEvent; SLOTS],
    seconds: f64,
    next_allowed: f64,
    transient_floor: f32,
    armed: bool,
    random: u32,
}

impl Default for MusicalEvents {
    fn default() -> Self {
        Self {
            events: [InkEvent::default(); SLOTS],
            seconds: 0.0,
            next_allowed: 0.0,
            transient_floor: 0.0,
            // A newly selected/started source must release once first, avoiding
            // a false gesture from the FFT's initial all-zero history.
            armed: false,
            random: 0xA341_316C,
        }
    }
}

impl MusicalEvents {
    pub fn new() -> Self {
        Self::default()
    }

    /// Call on source epoch/rate changes; stale input also clears immediately.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Each slot is [progress, strength, seed, kind]. Progress is 0..1 over
    /// 1.6–3 real seconds; progress >=1 or strength 0 means inactive. Seed is
    /// deterministic 0..1; kinds 0/1/2 represent bass/mid/air gestures.
    ///
    /// Supply measured worker features and physical dt, before visual Motion
    /// scaling. No allocations, raw PCM, wall clock or background task is used.
    /// Beat mapping gates onset emission; each kind also obeys its own band
    /// mapping. Disabling a gate clears its existing gestures immediately.
    pub fn update(
        &mut self,
        audio: &AudioFeatures,
        mappings: &AudioMappings,
        fresh: bool,
        dt: f32,
    ) -> [[f32; 4]; SLOTS] {
        let gains = [
            gain(mappings.bass_to_pulse),
            gain(mappings.mids_to_geometry),
            gain(mappings.treble_to_sparkle),
        ];
        let onset_gain = gain(mappings.beat_to_flash);
        if !fresh || onset_gain == 0.0 || gains.iter().all(|value| *value == 0.0) {
            self.reset();
            return [INACTIVE; SLOTS];
        }
        for event in &mut self.events {
            if gains[event.kind] == 0.0 {
                *event = InkEvent::default();
            }
        }
        if !dt.is_finite() || dt <= 0.0 {
            return self.uniforms(gains, onset_gain);
        }
        // A large gap is missing onset evidence, not permission to replay an
        // old peak. Normal updates retain their exact elapsed physical time.
        if dt > 0.5 {
            self.reset();
            return [INACTIVE; SLOTS];
        }
        let dt = f64::from(dt);
        self.seconds += dt;
        for event in &mut self.events {
            event.seconds += dt;
            if event.seconds >= event.duration {
                *event = InkEvent::default();
            }
        }

        let transient = unit(audio.transient);
        let novelty = ((transient - self.transient_floor - 0.025)
            / (0.18 + self.transient_floor * 0.45))
            .clamp(0.0, 1.0);
        let onset = novelty.max(unit(audio.beat));
        let follow = (1.0 - (-dt / 2.0).exp()) as f32;
        self.transient_floor += (transient - self.transient_floor) * follow;
        if onset < 0.24 {
            self.armed = true;
        }
        let rising = self.armed && onset >= 0.62;
        if rising {
            // Consume suppressed edges too. A held onset must never create a
            // delayed gesture merely because the cooldown has just elapsed.
            self.armed = false;
        }
        let bands = [unit(audio.bass), unit(audio.mids), unit(audio.treble)];
        let weights: [f32; SLOTS] = std::array::from_fn(|band| {
            if bands[band] >= 0.035 {
                bands[band] * gains[band]
            } else {
                0.0
            }
        });
        let total = weights.iter().sum::<f32>();
        let energy = unit(audio.energy);
        if rising && self.seconds + 1e-6 >= self.next_allowed && energy >= 0.025 && total >= 0.04 {
            let choice = self.next_random() * total;
            let mut accumulated = 0.0;
            let mut kind = 0;
            for (band, weight) in weights.iter().enumerate() {
                if *weight > 0.0 {
                    kind = band;
                    accumulated += weight;
                    if choice < accumulated {
                        break;
                    }
                }
            }
            let seed = self.next_random();
            let duration = match kind {
                0 => 2.8 + f64::from(seed) * 0.15,
                1 => 2.2 + f64::from(seed) * 0.30,
                _ => 1.6 + f64::from(seed) * 0.30,
            };
            let slot = self
                .events
                .iter()
                .position(|event| event.strength == 0.0)
                .unwrap_or_else(|| {
                    self.events
                        .iter()
                        .enumerate()
                        .max_by(|(_, a), (_, b)| {
                            (a.seconds / a.duration).total_cmp(&(b.seconds / b.duration))
                        })
                        .map_or(0, |(index, _)| index)
                });
            self.events[slot] = InkEvent {
                seconds: 0.0,
                duration,
                strength: (onset * 0.65 + energy * 0.35).clamp(0.0, 1.0),
                seed,
                kind,
            };
            let activity = (total / 3.0).clamp(0.0, 1.0);
            self.next_allowed = self.seconds + f64::from(3.0 - activity * 1.6);
        }
        self.uniforms(gains, onset_gain)
    }

    fn uniforms(&self, gains: [f32; 3], onset_gain: f32) -> [[f32; 4]; SLOTS] {
        std::array::from_fn(|slot| {
            let event = self.events[slot];
            let strength = (event.strength * gains[event.kind] * onset_gain).clamp(0.0, 1.5);
            if strength == 0.0 || event.seconds >= event.duration {
                INACTIVE
            } else {
                [
                    (event.seconds / event.duration) as f32,
                    strength,
                    event.seed,
                    event.kind as f32,
                ]
            }
        })
    }

    fn next_random(&mut self) -> f32 {
        self.random = self
            .random
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.random >> 8) as f32 / 16_777_216.0
    }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn gain(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.5)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active(slots: &[[f32; 4]; SLOTS]) -> usize {
        slots
            .iter()
            .filter(|slot| slot[0] < 1.0 && slot[1] > 0.0)
            .count()
    }

    fn music() -> AudioFeatures {
        AudioFeatures {
            bass: 0.9,
            mids: 0.8,
            treble: 0.7,
            energy: 0.8,
            beat: 1.0,
            transient: 0.9,
            ..AudioFeatures::default()
        }
    }

    fn release(controller: &mut MusicalEvents, mappings: &AudioMappings, dt: f32) {
        controller.update(&AudioFeatures::default(), mappings, true, dt);
    }

    #[test]
    fn silence_or_unmeasured_band_energy_never_creates_a_gesture() {
        let mut controller = MusicalEvents::new();
        let mappings = AudioMappings::default();
        for _ in 0..600 {
            assert_eq!(
                controller.update(&AudioFeatures::default(), &mappings, true, 1.0 / 60.0),
                [INACTIVE; SLOTS]
            );
        }
        let mut invalid_signal = music();
        invalid_signal.energy = 0.0;
        assert_eq!(
            active(&controller.update(&invalid_signal, &mappings, true, 0.02)),
            0
        );
        release(&mut controller, &mappings, 0.02);
        invalid_signal.energy = 1.0;
        invalid_signal.bass = 0.0;
        invalid_signal.mids = 0.0;
        invalid_signal.treble = 0.0;
        assert_eq!(
            active(&controller.update(&invalid_signal, &mappings, true, 0.02)),
            0
        );
    }

    #[test]
    fn one_rising_onset_emits_once_and_held_energy_does_not_pump() {
        let mut controller = MusicalEvents::new();
        let mappings = AudioMappings::default();
        release(&mut controller, &mappings, 0.02);
        let first = controller.update(&music(), &mappings, true, 0.02);
        assert_eq!(active(&first), 1);
        assert_eq!(first[0][0], 0.0);
        let seed = first[0][2];
        for _ in 0..300 {
            let slots = controller.update(&music(), &mappings, true, 0.02);
            for event in slots.into_iter().filter(|event| event[1] > 0.0) {
                assert_eq!(event[2], seed);
            }
        }
        assert_eq!(
            active(&controller.update(&music(), &mappings, true, 0.02)),
            0
        );
    }

    #[test]
    fn frequent_measured_onsets_obey_adaptive_cooldown_and_fixed_capacity() {
        let mut controller = MusicalEvents::new();
        let mappings = AudioMappings::default();
        let mut births = Vec::new();
        for frame in 0..900 {
            let features = if frame % 30 < 3 {
                music()
            } else {
                AudioFeatures::default()
            };
            let slots = controller.update(&features, &mappings, true, 1.0 / 60.0);
            assert!(active(&slots) <= SLOTS);
            if slots.iter().any(|slot| slot[0] == 0.0 && slot[1] > 0.0) {
                births.push(f64::from(frame) / 60.0);
                assert!(
                    (1.4 - 1e-6..=3.0).contains(&(controller.next_allowed - controller.seconds))
                );
                assert!(
                    controller
                        .events
                        .iter()
                        .filter(|event| event.strength > 0.0)
                        .all(|event| (1.6..=3.0).contains(&event.duration))
                );
            }
        }
        assert!(
            births.len() >= 4 && births.len() <= 10,
            "births: {births:?}"
        );
        assert!(births.windows(2).all(|pair| pair[1] - pair[0] >= 1.4));
    }

    #[test]
    fn each_band_is_independently_selected_and_zero_mapping_clears_active_ink() {
        for kind in 0..3 {
            let mut controller = MusicalEvents::new();
            let mut mappings = AudioMappings {
                bass_to_pulse: if kind == 0 { 1.0 } else { 0.0 },
                mids_to_geometry: if kind == 1 { 1.0 } else { 0.0 },
                treble_to_sparkle: if kind == 2 { 1.0 } else { 0.0 },
                beat_to_flash: 1.0,
            };
            release(&mut controller, &mappings, 0.02);
            let slots = controller.update(&music(), &mappings, true, 0.02);
            assert_eq!(active(&slots), 1);
            assert_eq!(slots[0][3], kind as f32);
            mappings.beat_to_flash = 0.0;
            assert_eq!(
                controller.update(&music(), &mappings, true, 0.0),
                [INACTIVE; SLOTS]
            );
            mappings.beat_to_flash = 1.0;
            release(&mut controller, &mappings, 0.02);
            assert_eq!(
                active(&controller.update(&music(), &mappings, true, 0.02)),
                1
            );
            // Clearing one band's mapping must remove its old gesture even
            // while a different band stays enabled; no new onset is inferred.
            mappings.bass_to_pulse = if kind == 1 { 1.0 } else { 0.0 };
            mappings.mids_to_geometry = if kind == 2 { 1.0 } else { 0.0 };
            mappings.treble_to_sparkle = if kind == 0 { 1.0 } else { 0.0 };
            assert_eq!(
                controller.update(&music(), &mappings, true, 0.0),
                [INACTIVE; SLOTS]
            );
            mappings.bass_to_pulse = 0.0;
            mappings.mids_to_geometry = 0.0;
            mappings.treble_to_sparkle = 0.0;
            assert_eq!(
                controller.update(&music(), &mappings, true, 0.0),
                [INACTIVE; SLOTS]
            );
        }
    }

    #[test]
    fn stale_source_and_explicit_reset_clear_without_replaying_the_old_onset() {
        let mut controller = MusicalEvents::new();
        let mappings = AudioMappings::default();
        release(&mut controller, &mappings, 0.02);
        assert_eq!(
            active(&controller.update(&music(), &mappings, true, 0.02)),
            1
        );
        assert_eq!(
            controller.update(&music(), &mappings, false, 0.0),
            [INACTIVE; SLOTS]
        );
        assert_eq!(
            active(&controller.update(&music(), &mappings, true, 0.02)),
            0
        );
        release(&mut controller, &mappings, 0.02);
        assert_eq!(
            active(&controller.update(&music(), &mappings, true, 0.02)),
            1
        );
        controller.reset();
        assert_eq!(
            controller.update(&music(), &mappings, true, 0.0),
            [INACTIVE; SLOTS]
        );
    }

    #[test]
    fn lifetimes_and_deterministic_geometry_match_at_multiple_refresh_rates() {
        let mappings = AudioMappings::default();
        let mut checkpoints = Vec::new();
        for hz in [30, 60, 120] {
            let mut controller = MusicalEvents::new();
            let dt = 1.0 / hz as f32;
            release(&mut controller, &mappings, dt);
            let first = controller.update(&music(), &mappings, true, dt);
            assert_eq!(active(&first), 1);
            let mut after_second = first;
            for _ in 0..hz {
                after_second = controller.update(&AudioFeatures::default(), &mappings, true, dt);
            }
            checkpoints.push(after_second);
            for _ in 0..hz * 3 {
                controller.update(&AudioFeatures::default(), &mappings, true, dt);
            }
            assert_eq!(
                controller.update(&AudioFeatures::default(), &mappings, true, dt),
                [INACTIVE; SLOTS]
            );
        }
        for pair in checkpoints.windows(2) {
            assert!((pair[0][0][0] - pair[1][0][0]).abs() < 0.00001);
            assert_eq!(pair[0][0][1..], pair[1][0][1..]);
        }
    }

    #[test]
    fn repeated_layout_passes_and_invalid_clocks_cannot_create_or_age_ink() {
        let mut controller = MusicalEvents::new();
        let mappings = AudioMappings::default();
        release(&mut controller, &mappings, 0.02);
        let first = controller.update(&music(), &mappings, true, 0.02);
        for dt in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(controller.update(&music(), &mappings, true, dt), first);
        }
        assert_eq!(
            controller.update(&music(), &mappings, true, f32::MAX),
            [INACTIVE; SLOTS]
        );
    }

    #[test]
    fn nonfinite_features_and_mapping_values_never_reach_the_uniforms() {
        let mut controller = MusicalEvents::new();
        let features = AudioFeatures {
            bass: f32::NAN,
            mids: f32::INFINITY,
            treble: f32::NEG_INFINITY,
            energy: f32::NAN,
            transient: f32::INFINITY,
            beat: f32::NAN,
            ..AudioFeatures::default()
        };
        let mappings = AudioMappings {
            bass_to_pulse: f32::INFINITY,
            mids_to_geometry: -1.0,
            treble_to_sparkle: f32::NAN,
            beat_to_flash: f32::INFINITY,
        };
        let slots = controller.update(&features, &mappings, true, 0.02);
        assert_eq!(slots, [INACTIVE; SLOTS]);
        assert!(slots.iter().flatten().all(|value| value.is_finite()));
    }
}
