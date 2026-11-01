use crate::dsp::AudioFeatures;

/// Keep existing slow/normal profile values literal, then open the upper knob
/// range into real travel speed. This is shared by time integration and
/// history reprojection so fast motion cannot leave its feedback behind.
pub fn motion_rate(value: f32) -> f32 {
    let motion = if value.is_finite() {
        value.clamp(0.05, 1.8)
    } else {
        0.05
    };
    if motion <= 1.0 {
        return motion;
    }
    let t = (motion - 1.0) / 0.8;
    motion * (1.0 + 5.0 * t * t)
}

/// Inverse of the existing portable profile value. The UI can show literal
/// speed without changing older saved profiles or the shader uniform layout.
pub fn motion_value(rate: f32) -> f32 {
    let rate = if rate.is_finite() {
        rate.clamp(0.05, 10.8)
    } else {
        0.05
    };
    if rate <= 1.0 {
        return rate;
    }
    let (mut low, mut high) = (1.0, 1.8);
    for _ in 0..22 {
        let middle = (low + high) * 0.5;
        if motion_rate(middle) < rate {
            low = middle;
        } else {
            high = middle;
        }
    }
    (low + high) * 0.5
}

/// One user-owned pace for travel. Theme/structure changes and music responses
/// keep physical time. Integrate acceleration rather than multiplying an
/// accumulated position by a newly selected speed.
pub struct MotionPace {
    rate: f64,
}

pub struct PaceStep {
    pub travel_delta: f32,
    pub evolution_delta: f32,
    pub world_delta: f32,
    pub average_rate: f32,
}

impl MotionPace {
    pub fn new(value: f32) -> Self {
        Self {
            rate: f64::from(motion_rate(value)),
        }
    }

    pub fn advance(&mut self, value: f32, dt: f32) -> PaceStep {
        let dt = if dt.is_finite() {
            f64::from(dt.clamp(0.0, 0.1))
        } else {
            0.0
        };
        let target = f64::from(motion_rate(value));
        let before = self.rate;
        let tau = 0.16;
        let decay = (-dt / tau).exp();
        let travel = target * dt + (before - target) * tau * (1.0 - decay);
        self.rate = target + (before - target) * decay;
        // Fast flight must not turn the structure picker into rapid cycling.
        // Keep the existing split API, with both composition clocks physical.
        PaceStep {
            travel_delta: travel as f32,
            evolution_delta: dt as f32,
            world_delta: dt as f32,
            average_rate: if dt > 0.0 {
                (travel / dt) as f32
            } else {
                self.rate as f32
            },
        }
    }
}

/// Visual-only response relative to one measured band's recent level. The
/// baseline has a four-second time constant; no waveform, onset or beat is
/// generated here. This is original bounded contrast, not an imported preset
/// engine's loudness implementation.
#[derive(Default)]
struct RelativeLevel {
    average: f64,
    initialized: bool,
}

impl RelativeLevel {
    fn target(&mut self, measured: f32, energy_gate: f32, dt: f32, decay: f64) -> f32 {
        let measured = unit(measured);
        if dt <= 0.0 {
            return measured;
        }
        let gate = energy_gate * soft_gate(measured, 0.015, 0.065);
        if !self.initialized {
            if gate > 0.0 {
                // The first actual signal defines its own normal level.
                // Starting capture must not look like a giant relative onset.
                self.average = f64::from(measured);
                self.initialized = true;
            }
            return measured;
        }
        let previous = self.average;
        self.average = f64::from(measured) + (previous - f64::from(measured)) * decay;
        // Centre the slow baseline within this elapsed interval, reducing
        // repaint-rate bias while the existing fast material envelope follows.
        let reference = ((previous + self.average) * 0.5) as f32;
        let contrast = (measured - reference) / (measured + reference + 0.030);
        let emphasis = measured * (1.0 - 0.28 * (-contrast).max(0.0))
            + 0.85 * contrast.max(0.0) * (1.0 - measured);
        // Preserve measured volume and a quiet floor; weak noise never gets
        // normalized to a bright constant. At zero signal this is exactly zero.
        unit(measured + (emphasis - measured) * gate)
    }
}

fn soft_gate(value: f32, low: f32, high: f32) -> f32 {
    let t = ((unit(value) - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Interpolates completed analysis snapshots and emphasizes measured relative
/// band accents for drawing only. Recognition, meters, energy and beat timing
/// retain the original worker features; every clock here is real elapsed time.
#[derive(Default)]
pub struct RenderMotion {
    features: AudioFeatures,
    bands: [RelativeLevel; 3],
    spectrum: Vec<RelativeLevel>,
}

impl RenderMotion {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a newly selected source from a quiet visual state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update(&mut self, latest: &AudioFeatures, dt: f32) -> AudioFeatures {
        // A missed frame should catch up smoothly, while bogus clocks must not
        // inject NaNs or jump the camera. A normal 30/60/120 Hz update is exact
        // exponential interpolation, independent of the repaint frequency.
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            0.0
        };
        let baseline_decay = (-f64::from(dt) / 4.0).exp();
        let energy_gate = soft_gate(latest.energy, 0.012, 0.070);
        let measured = [latest.bass, latest.mids, latest.treble];
        let targets: [f32; 3] = std::array::from_fn(|index| {
            self.bands[index].target(measured[index], energy_gate, dt, baseline_decay)
        });
        // Short material attacks retain the articulation of kicks and hats.
        // Camera acceleration has its own much slower choreography envelope.
        self.features.bass = follow(self.features.bass, targets[0], dt, 0.045, 0.18);
        self.features.mids = follow(self.features.mids, targets[1], dt, 0.065, 0.20);
        self.features.treble = follow(self.features.treble, targets[2], dt, 0.035, 0.14);
        self.features.energy = follow(self.features.energy, latest.energy, dt, 0.08, 0.20);
        self.features.beat = follow(self.features.beat, latest.beat, dt, 0.03, 0.14);
        self.features.transient =
            follow(self.features.transient, latest.transient, dt, 0.025, 0.12);
        self.features.spectrum.resize(latest.spectrum.len(), 0.0);
        self.spectrum
            .resize_with(latest.spectrum.len(), RelativeLevel::default);
        for ((current, measured), relative) in self
            .features
            .spectrum
            .iter_mut()
            .zip(&latest.spectrum)
            .zip(&mut self.spectrum)
        {
            let target = relative.target(*measured, energy_gate, dt, baseline_decay);
            *current = follow(*current, target, dt, 0.045, 0.16);
        }
        self.features.bpm = latest.bpm.filter(|bpm| bpm.is_finite() && *bpm > 0.0);
        self.features.dna = latest.dna.clone();
        self.features.dna.brightness = unit(self.features.dna.brightness);
        self.features.dna.danceability = unit(self.features.dna.danceability);
        self.features.clone()
    }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn follow(current: f32, target: f32, dt: f32, attack: f32, release: f32) -> f32 {
    let current = unit(current);
    let target = unit(target);
    let tau = if target > current { attack } else { release };
    let amount = 1.0 - (-dt / tau).exp();
    let next = current + (target - current) * amount;
    if dt > 0.0 && target == 0.0 && next < 0.000_01 {
        0.0
    } else {
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_speed_round_trips_existing_profile_values() {
        for step in 0..=175 {
            let value = 0.05 + step as f32 * 0.01;
            assert!((motion_value(motion_rate(value)) - value).abs() < 0.000_002);
        }
    }

    #[test]
    fn pace_changes_travel_but_not_theme_or_structure_clocks() {
        let mut pace = MotionPace::new(0.05);
        let slow = pace.advance(0.05, 0.1);
        assert!((slow.travel_delta - 0.005).abs() < 0.000_001);
        assert!((slow.evolution_delta - 0.1).abs() < 0.000_001);
        assert!((slow.world_delta - 0.1).abs() < 0.000_001);
        let fast = pace.advance(1.8, 1.0 / 60.0);
        assert!(fast.travel_delta > slow.travel_delta);
        assert!(fast.average_rate < 1.2); // acceleration, not an instant jump to 10.8
        for _ in 0..120 {
            pace.advance(1.8, 1.0 / 60.0);
        }
        let settled = pace.advance(1.8, 0.1);
        assert!((settled.travel_delta - 1.08).abs() < 0.0001);
        assert!((settled.evolution_delta - 0.1).abs() < 0.0001);
        assert!((settled.world_delta - 0.1).abs() < 0.0001);
        let paused = pace.advance(0.05, 0.0);
        assert_eq!(paused.travel_delta, 0.0);
        assert_eq!(paused.evolution_delta, 0.0);
        assert_eq!(paused.world_delta, 0.0);
    }

    #[test]
    fn pace_ramps_and_physical_composition_time_are_refresh_rate_independent() {
        let mut results = Vec::new();
        for hz in [30, 60, 120] {
            let mut pace = MotionPace::new(0.05);
            let (mut travel, mut evolution, mut world) = (0.0_f64, 0.0_f64, 0.0_f64);
            for value in [1.8, 0.05, 1.4, 0.3] {
                for _ in 0..hz {
                    let step = pace.advance(value, 1.0 / hz as f32);
                    travel += f64::from(step.travel_delta);
                    evolution += f64::from(step.evolution_delta);
                    world += f64::from(step.world_delta);
                }
            }
            results.push((travel, evolution, world));
            assert!((evolution - 4.0).abs() < 0.00001);
            assert!((world - 4.0).abs() < 0.00001);
        }
        for result in &results[1..] {
            assert!((result.0 - results[0].0).abs() < 0.000_01);
            assert!((result.1 - results[0].1).abs() < 0.000_01);
            assert!((result.2 - results[0].2).abs() < 0.000_01);
        }
    }

    #[test]
    fn motion_rate_preserves_slow_profiles_and_accelerates_the_upper_range() {
        for value in [0.05, 0.25, 0.75, 1.0] {
            assert_eq!(motion_rate(value), value);
        }
        assert!(motion_rate(1.4) > 3.0);
        assert!((motion_rate(1.8) - 10.8).abs() < 0.000_01);
    }

    #[test]
    fn motion_rate_is_continuous_and_monotonic() {
        let mut previous = motion_rate(0.05);
        for step in 1..=1750 {
            let next = motion_rate(0.05 + step as f32 / 1000.0);
            assert!(next >= previous);
            assert!(next - previous < 0.04);
            previous = next;
        }
        assert!((motion_rate(1.000_01) - motion_rate(1.0)).abs() < 0.000_02);
    }

    #[test]
    fn motion_rate_bounds_bad_values_without_nan_or_accidental_maximum() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -10.0, 0.0] {
            assert_eq!(motion_rate(value), 0.05);
        }
        assert_eq!(motion_rate(100.0), motion_rate(1.8));
    }

    fn full_signal() -> AudioFeatures {
        AudioFeatures {
            bass: 1.0,
            mids: 1.0,
            treble: 1.0,
            energy: 1.0,
            beat: 1.0,
            transient: 1.0,
            bpm: Some(120.0),
            spectrum: vec![1.0; 48],
            ..AudioFeatures::default()
        }
    }

    fn relative_signal(level: f32) -> AudioFeatures {
        AudioFeatures {
            bass: level,
            mids: level * 0.8,
            treble: level * 0.6,
            energy: level * 2.0,
            spectrum: (0..48)
                .map(|index| level * (0.6 + index as f32 / 120.0))
                .collect(),
            ..AudioFeatures::default()
        }
    }

    #[test]
    fn relative_accents_remain_visible_at_different_recording_levels_and_then_settle() {
        let mut contrasts = Vec::new();
        for level in [0.05, 0.20] {
            let mut motion = RenderMotion::new();
            let quiet_passage = relative_signal(level);
            let accent = relative_signal(level * 2.0);
            let mut held = AudioFeatures::default();
            for _ in 0..480 {
                held = motion.update(&quiet_passage, 1.0 / 60.0);
            }
            assert!((held.bass - level).abs() < 0.0001);
            let mut peak = held.clone();
            for _ in 0..12 {
                peak = motion.update(&accent, 1.0 / 60.0);
            }
            assert!(peak.bass > accent.bass + 0.10);
            assert!(peak.spectrum[24] > accent.spectrum[24] + 0.10);
            assert_eq!(peak.beat, 0.0);
            assert_eq!(peak.transient, 0.0);
            contrasts.push(peak.bass - held.bass);
            for _ in 0..1200 {
                held = motion.update(&accent, 1.0 / 60.0);
            }
            assert!(peak.bass > held.bass + 0.10);
            assert!((held.bass - accent.bass).abs() < 0.005);
        }
        // A fourfold recording-level difference no longer causes a fourfold
        // accent difference, while the louder passage still looks stronger.
        assert!(contrasts[1] > contrasts[0]);
        assert!(contrasts[1] / contrasts[0] < 2.0);
    }

    #[test]
    fn relative_response_does_not_amplify_quiet_noise_and_releases_to_exact_silence() {
        let mut motion = RenderMotion::new();
        for frame in 0..600 {
            let mut noise = relative_signal(if frame % 30 < 15 { 0.04 } else { 0.10 });
            noise.energy = 0.009;
            let out = motion.update(&noise, 1.0 / 60.0);
            assert!(out.bass <= 0.10);
            assert!(out.spectrum.iter().all(|value| *value <= 0.10));
        }
        motion.reset();
        for frame in 0..600 {
            let mut noise_bin = relative_signal(if frame % 30 < 15 { 0.003 } else { 0.010 });
            noise_bin.energy = 0.8; // a weak bin within otherwise loud music
            let out = motion.update(&noise_bin, 1.0 / 60.0);
            assert!(out.bass <= 0.010);
            assert!(out.spectrum.iter().all(|value| *value <= 0.010));
        }
        motion.update(&full_signal(), 0.1);
        let mut out = AudioFeatures::default();
        for _ in 0..240 {
            out = motion.update(&AudioFeatures::default(), 1.0 / 60.0);
        }
        assert_eq!(
            [
                out.bass,
                out.mids,
                out.treble,
                out.energy,
                out.beat,
                out.transient
            ],
            [0.0; 6]
        );
        assert!(out.spectrum.iter().all(|value| *value == 0.0));
    }

    #[test]
    fn source_reset_clears_relative_history_without_changing_bpm_or_dna() {
        let mut used = RenderMotion::new();
        for _ in 0..600 {
            used.update(&relative_signal(0.04), 1.0 / 60.0);
        }
        used.reset();
        let mut fresh = RenderMotion::new();
        let mut signal = relative_signal(0.35);
        signal.bpm = Some(127.5);
        signal.dna.genre_hint = "Measured example".into();
        signal.dna.mood = "Steady example".into();
        signal.dna.instrument_hints = "Uncertain example".into();
        signal.dna.brightness = 0.43;
        signal.dna.danceability = 0.76;
        let mut out = AudioFeatures::default();
        for _ in 0..60 {
            out = used.update(&signal, 1.0 / 60.0);
            let clean = fresh.update(&signal, 1.0 / 60.0);
            assert_eq!(out.bass, clean.bass);
            assert_eq!(out.spectrum, clean.spectrum);
        }
        assert!((out.bass - signal.bass).abs() < 0.0001);
        assert_eq!(out.bpm, signal.bpm);
        assert_eq!(out.dna.genre_hint, signal.dna.genre_hint);
        assert_eq!(out.dna.mood, signal.dna.mood);
        assert_eq!(out.dna.instrument_hints, signal.dna.instrument_hints);
        assert_eq!(out.dna.brightness, signal.dna.brightness);
        assert_eq!(out.dna.danceability, signal.dna.danceability);
        assert_eq!(out.dna.inference_method, signal.dna.inference_method);
        assert_eq!(out.beat, 0.0);
        assert_eq!(out.transient, 0.0);
    }

    #[test]
    fn adaptive_band_and_bin_accents_are_stable_across_refresh_rates() {
        let mut runs = Vec::new();
        for hz in [30, 60, 120] {
            let mut motion = RenderMotion::new();
            let mut checkpoints = Vec::new();
            for _ in 0..hz * 6 {
                motion.update(&relative_signal(0.12), 1.0 / hz as f32);
            }
            for _ in 0..6 {
                for (level, frames) in [(0.30, hz / 5), (0.09, hz * 4 / 5)] {
                    let mut out = AudioFeatures::default();
                    for _ in 0..frames {
                        out = motion.update(&relative_signal(level), 1.0 / hz as f32);
                    }
                    checkpoints.push([out.bass, out.mids, out.treble, out.spectrum[24]]);
                }
            }
            runs.push(checkpoints);
        }
        for run in &runs[1..] {
            for (actual, reference) in run.iter().zip(&runs[0]) {
                for (actual, reference) in actual.iter().zip(reference) {
                    assert!((actual - reference).abs() < 0.003);
                }
            }
        }
    }

    #[test]
    fn a_new_snapshot_becomes_continuous_visual_motion() {
        let mut motion = RenderMotion::new();
        let target = full_signal();
        let first = motion.update(&target, 1.0 / 60.0);
        assert!(first.bass > 0.30 && first.bass < 0.32);
        assert!(first.spectrum[0] > 0.30 && first.spectrum[0] < 0.32);
        assert!(first.beat > first.bass && first.beat < 0.5);
        let second = motion.update(&target, 1.0 / 60.0);
        assert!(second.bass > first.bass && second.bass < 0.53);
        assert!(second.bass - first.bass < first.bass);
        assert_eq!(second.bpm, Some(120.0));
    }

    #[test]
    fn envelopes_converge_equally_at_30_60_and_120_hz() {
        let target = full_signal();
        let mut results = Vec::new();
        for hz in [30, 60, 120] {
            let mut motion = RenderMotion::new();
            let mut result = AudioFeatures::default();
            for _ in 0..hz / 2 {
                result = motion.update(&target, 1.0 / hz as f32);
            }
            let attack = result.clone();
            for _ in 0..hz / 2 {
                result = motion.update(&AudioFeatures::default(), 1.0 / hz as f32);
            }
            results.push((attack, result));
        }
        for phase in [0, 1] {
            let reference = if phase == 0 {
                &results[0].0
            } else {
                &results[0].1
            };
            for result in &results[1..] {
                let result = if phase == 0 { &result.0 } else { &result.1 };
                assert!((result.bass - reference.bass).abs() < 0.000_01);
                assert!((result.energy - reference.energy).abs() < 0.000_01);
                assert!((result.spectrum[24] - reference.spectrum[24]).abs() < 0.000_01);
                assert!((result.beat - reference.beat).abs() < 0.000_01);
            }
        }
    }

    #[test]
    fn silence_decays_without_a_cut_and_source_reset_is_immediate() {
        let mut motion = RenderMotion::new();
        let target = full_signal();
        for _ in 0..60 {
            motion.update(&target, 1.0 / 60.0);
        }
        let quiet = AudioFeatures::default();
        let first = motion.update(&quiet, 1.0 / 60.0);
        assert!(first.energy > 0.9 && first.energy < 1.0);
        assert!(first.beat > 0.85 && first.beat < 1.0);
        for _ in 0..120 {
            motion.update(&quiet, 1.0 / 60.0);
        }
        let settled = motion.update(&quiet, 1.0 / 60.0);
        assert!(settled.energy < 0.001);
        assert!(settled.beat < 0.001);
        assert!(settled.spectrum.iter().all(|value| *value < 0.001));
        motion.update(&target, 0.1);
        motion.reset();
        let reset = motion.update(&quiet, 0.0);
        assert_eq!(reset.energy, 0.0);
        assert_eq!(reset.beat, 0.0);
    }

    #[test]
    fn invalid_clocks_and_signals_cannot_produce_nan() {
        let mut motion = RenderMotion::new();
        let mut invalid = full_signal();
        invalid.bass = f32::NAN;
        invalid.energy = f32::INFINITY;
        invalid.transient = f32::NEG_INFINITY;
        invalid.spectrum = vec![f32::NAN, f32::INFINITY, -1.0, 2.0];
        invalid.bpm = Some(f32::NAN);
        invalid.dna.brightness = f32::NAN;
        invalid.dna.danceability = f32::INFINITY;
        for dt in [f32::NAN, f32::INFINITY, -0.1, 0.0, 0.016] {
            let result = motion.update(&invalid, dt);
            assert!(result.bass.is_finite() && result.energy.is_finite());
            assert!(result.transient.is_finite());
            assert!(
                result
                    .spectrum
                    .iter()
                    .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
            );
            assert!(result.bpm.is_none());
            assert!(result.dna.brightness.is_finite() && result.dna.danceability.is_finite());
        }
    }
}
