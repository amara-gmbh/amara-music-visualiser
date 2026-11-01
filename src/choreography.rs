use crate::{dsp::AudioFeatures, presets::AudioMappings};

/// Slow visual controls inferred from sustained signal levels. These describe
/// an evolving visual phrase, not detected verses, choruses or song structure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Choreography {
    pub drive: f32,
    pub openness: f32,
    pub lift: f32,
    pub accent: f32,
    pub travel_speed: f32,
    pub phase: f32,
}

impl Default for Choreography {
    fn default() -> Self {
        Self {
            drive: 0.0,
            openness: 1.0,
            lift: 0.0,
            accent: 0.0,
            travel_speed: 0.65,
            phase: 0.0,
        }
    }
}

#[derive(Default)]
pub struct Choreographer {
    state: Choreography,
    energy_baseline: f32,
    phase: f64,
    tempo_candidate: Option<f32>,
    tempo_stable_seconds: f32,
}

impl Choreographer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Forget the previous source's phrase without teleporting the camera or
    /// changing its velocity abruptly. Travel eases toward the new source.
    pub fn reset(&mut self) {
        self.energy_baseline = 0.0;
        self.tempo_candidate = None;
        self.tempo_stable_seconds = 0.0;
        self.state = Choreography {
            phase: self.phase as f32,
            travel_speed: self.state.travel_speed,
            ..Choreography::default()
        };
    }

    /// Route the same user-controlled audio mappings into slow phrase motion.
    /// All-zero mappings remove audio influence; accumulated motion settles
    /// smoothly rather than snapping when a mapping is disconnected mid-flight.
    pub fn update_mapped(
        &mut self,
        features: &AudioFeatures,
        mappings: &AudioMappings,
        dt: f32,
    ) -> Choreography {
        let mut mapped = features.clone();
        mapped.energy = (unit(features.bass) * gain(mappings.bass_to_pulse)
            + unit(features.mids) * gain(mappings.mids_to_geometry)
            + unit(features.treble) * gain(mappings.treble_to_sparkle))
            / 3.0;
        mapped.transient = unit(features.transient) * gain(mappings.beat_to_flash);
        self.update_routed(&mapped, dt, gain(mappings.beat_to_flash))
    }

    #[cfg(test)]
    pub fn update(&mut self, features: &AudioFeatures, dt: f32) -> Choreography {
        self.update_routed(features, dt, 1.0)
    }

    fn update_routed(
        &mut self,
        features: &AudioFeatures,
        dt: f32,
        rhythm_gain: f32,
    ) -> Choreography {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            0.0
        };
        if dt == 0.0 {
            return self.state;
        }
        let energy = unit(features.energy);
        self.state.drive = follow(self.state.drive, energy, dt, 2.0);
        self.energy_baseline = follow(self.energy_baseline, energy, dt, 6.0);

        // A sustained rise above the slower history briefly opens a phrase.
        // Steady loud audio settles again; one kick cannot steer the camera.
        let rising = (self.state.drive - self.energy_baseline).max(0.0);
        let lift_target = (self.state.drive * 0.5 + rising * 1.25).clamp(0.0, 1.0);
        self.state.lift = follow(self.state.lift, lift_target, dt, 1.3);
        self.state.openness = 1.0 - self.state.drive * 0.72;

        // Individual beat impulses only glint the material. A sustained tempo
        // estimate can shape travel, but never kicks the camera once per beat.
        let accent_target = unit(features.transient) * 0.35;
        let accent_tau = if accent_target > self.state.accent {
            0.08
        } else {
            0.28
        };
        self.state.accent = follow(self.state.accent, accent_target, dt, accent_tau);

        let old_speed = self.state.travel_speed;
        let tempo_bias = self.tempo_bias(features.bpm, rhythm_gain, dt);
        let speed_target = (0.65 + self.state.drive * 0.85 + tempo_bias).clamp(0.55, 1.8);
        self.state.travel_speed = follow(old_speed, speed_target, dt, 2.0).clamp(0.55, 1.8);
        // Integrating velocity preserves position as the music evolves. Using
        // elapsed_time * current_speed would jump whenever speed changes.
        self.phase += f64::from(dt) * f64::from((old_speed + self.state.travel_speed) * 0.5);
        self.state.phase = self.phase as f32;
        self.state
    }

    fn tempo_bias(&mut self, bpm: Option<f32>, rhythm_gain: f32, dt: f32) -> f32 {
        // The analyser already withholds inconsistent onset estimates. Add a
        // short confirmation window here so octave flips or erratic estimates
        // cannot alternately accelerate and brake the voyage. This is still
        // an estimate, not beat-grid locking or a claim of genre recognition.
        let bpm = bpm
            .filter(|bpm| bpm.is_finite() && (50.0..=210.0).contains(bpm))
            .filter(|_| rhythm_gain > 0.0);
        let Some(bpm) = bpm else {
            self.tempo_candidate = None;
            self.tempo_stable_seconds = 0.0;
            return 0.0;
        };
        match self.tempo_candidate {
            Some(candidate) if (candidate - bpm).abs() / candidate <= 0.12 => {
                self.tempo_candidate = Some(follow(candidate, bpm, dt, 0.4));
                self.tempo_stable_seconds = (self.tempo_stable_seconds + dt).min(1.0);
            }
            _ => {
                self.tempo_candidate = Some(bpm);
                self.tempo_stable_seconds = dt;
            }
        }
        if self.tempo_stable_seconds < 0.75 {
            return 0.0;
        }
        let confirmed = self.tempo_candidate.unwrap_or(bpm);
        ((confirmed - 110.0) / 100.0).clamp(-0.45, 0.60) * rhythm_gain
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

fn follow(current: f32, target: f32, dt: f32, tau: f32) -> f32 {
    current + (target - current) * (1.0 - (-dt / tau).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loud() -> AudioFeatures {
        AudioFeatures {
            energy: 1.0,
            transient: 1.0,
            beat: 1.0,
            bpm: Some(120.0),
            ..AudioFeatures::default()
        }
    }

    fn run(
        choreographer: &mut Choreographer,
        features: &AudioFeatures,
        hz: usize,
        seconds: usize,
    ) -> Choreography {
        let mut result = choreographer.state;
        for _ in 0..hz * seconds {
            result = choreographer.update(features, 1.0 / hz as f32);
        }
        result
    }

    #[test]
    fn sustained_music_builds_a_phrase_without_a_camera_jump() {
        let mut choreography = Choreographer::new();
        let quiet = run(&mut choreography, &AudioFeatures::default(), 60, 2);
        let first = choreography.update(&loud(), 1.0 / 60.0);
        assert!(first.drive > 0.0 && first.drive < 0.01);
        assert!((first.travel_speed - quiet.travel_speed).abs() < 0.001);
        assert!((first.phase - quiet.phase) < 0.012);
        let built = run(&mut choreography, &loud(), 60, 4);
        assert!(built.drive > 0.85);
        assert!(built.lift > 0.5);
        assert!(built.openness < 0.4);
        assert!(built.travel_speed > 1.15 && built.travel_speed <= 1.8);
        assert!(built.accent > 0.3 && built.accent <= 0.35);
    }

    #[test]
    fn silence_opens_the_world_and_slows_travel_gradually() {
        let mut choreography = Choreographer::new();
        let intense = run(&mut choreography, &loud(), 60, 12);
        let first = choreography.update(&AudioFeatures::default(), 1.0 / 60.0);
        assert!(first.drive > 0.98);
        assert!((intense.travel_speed - first.travel_speed).abs() < 0.001);
        assert!(first.phase > intense.phase);
        let quiet = run(&mut choreography, &AudioFeatures::default(), 60, 18);
        assert!(quiet.drive < 0.001);
        assert!(quiet.openness > 0.999);
        assert!(quiet.lift < 0.001);
        assert!(quiet.accent < 0.001);
        assert!((quiet.travel_speed - 0.65).abs() < 0.002);
        assert!(quiet.phase > first.phase);
    }

    #[test]
    fn phrase_motion_is_close_at_30_60_and_120_hz() {
        let results: Vec<_> = [30, 60, 120]
            .into_iter()
            .map(|hz| {
                let mut choreography = Choreographer::new();
                let attack = run(&mut choreography, &loud(), hz, 8);
                let release = run(&mut choreography, &AudioFeatures::default(), hz, 8);
                [attack, release]
            })
            .collect();
        for phase in 0..2 {
            let reference = results[0][phase];
            for result in &results[1..] {
                let result = result[phase];
                assert!((result.drive - reference.drive).abs() < 0.000_1);
                assert!((result.openness - reference.openness).abs() < 0.000_1);
                assert!((result.lift - reference.lift).abs() < 0.004);
                assert!((result.travel_speed - reference.travel_speed).abs() < 0.004);
                assert!((result.phase - reference.phase).abs() < 0.015);
            }
        }
    }

    #[test]
    fn individual_beats_do_not_steer_travel_and_reset_preserves_position() {
        let mut calm = Choreographer::new();
        let mut percussive = Choreographer::new();
        let quiet = AudioFeatures {
            bpm: Some(160.0),
            ..AudioFeatures::default()
        };
        let percussion = AudioFeatures {
            beat: 1.0,
            transient: 1.0,
            bpm: Some(160.0),
            ..AudioFeatures::default()
        };
        let a = run(&mut calm, &quiet, 60, 4);
        let b = run(&mut percussive, &percussion, 60, 4);
        assert_eq!(a.phase, b.phase);
        assert_eq!(a.travel_speed, b.travel_speed);
        assert_eq!(a.drive, b.drive);
        assert!(b.accent > a.accent);
        let before = run(&mut percussive, &loud(), 60, 4);
        percussive.reset();
        let after = percussive.update(&quiet, 0.0);
        assert_eq!(before.phase, after.phase);
        assert_eq!(before.travel_speed, after.travel_speed);
        assert_eq!(after.drive, 0.0);
        assert_eq!(after.lift, 0.0);
        assert_eq!(after.accent, 0.0);
    }

    #[test]
    fn invalid_inputs_and_clocks_remain_finite() {
        let invalid = AudioFeatures {
            energy: f32::NAN,
            transient: f32::INFINITY,
            ..loud()
        };
        let mut choreography = Choreographer::new();
        for dt in [f32::NAN, f32::INFINITY, -1.0, 0.0, 0.016, 5.0] {
            let result = choreography.update(&invalid, dt);
            for value in [result.drive, result.openness, result.lift, result.accent] {
                assert!(value.is_finite() && (0.0..=1.0).contains(&value));
            }
            assert!(result.travel_speed.is_finite() && (0.55..=1.8).contains(&result.travel_speed));
            assert!(result.phase.is_finite() && result.phase >= 0.0);
        }
    }

    #[test]
    fn zero_mappings_make_quiet_and_loud_phrase_motion_identical() {
        let disconnected = AudioMappings {
            bass_to_pulse: 0.0,
            mids_to_geometry: 0.0,
            treble_to_sparkle: 0.0,
            beat_to_flash: 0.0,
        };
        let music = AudioFeatures {
            bass: 1.0,
            mids: 0.8,
            treble: 0.7,
            ..loud()
        };
        let mut quiet = Choreographer::new();
        let mut loud = Choreographer::new();
        for _ in 0..600 {
            let a = quiet.update_mapped(&AudioFeatures::default(), &disconnected, 1.0 / 60.0);
            let b = loud.update_mapped(&music, &disconnected, 1.0 / 60.0);
            assert_eq!(a, b);
            assert_eq!(b.drive, 0.0);
            assert_eq!(b.accent, 0.0);
        }
    }

    #[test]
    fn enabled_mappings_guide_motion_and_source_reset_keeps_the_flight_continuous() {
        let music = AudioFeatures {
            bass: 1.0,
            mids: 0.8,
            treble: 0.7,
            ..loud()
        };
        let mappings = AudioMappings::default();
        let mut choreography = Choreographer::new();
        let mut before = Choreography::default();
        for _ in 0..600 {
            before = choreography.update_mapped(&music, &mappings, 1.0 / 60.0);
        }
        assert!(before.drive > 0.7 && before.drive < 0.8);
        assert!(before.travel_speed > 1.2);
        assert!(before.accent > 0.2);
        choreography.reset();
        let after = choreography.update_mapped(&AudioFeatures::default(), &mappings, 0.0);
        assert_eq!(after.phase, before.phase);
        assert_eq!(after.travel_speed, before.travel_speed);
        assert_eq!(after.drive, 0.0);
        let next = choreography.update_mapped(&AudioFeatures::default(), &mappings, 1.0 / 60.0);
        assert!(next.phase > after.phase && next.phase - after.phase < 0.025);
        assert!((next.travel_speed - after.travel_speed).abs() < 0.01);
    }

    #[test]
    fn invalid_mapping_values_cannot_poison_phrase_motion() {
        let mappings = AudioMappings {
            bass_to_pulse: f32::NAN,
            mids_to_geometry: f32::INFINITY,
            treble_to_sparkle: -1.0,
            beat_to_flash: f32::NEG_INFINITY,
        };
        let music = AudioFeatures {
            bass: 1.0,
            mids: 1.0,
            treble: 1.0,
            ..loud()
        };
        let result = Choreographer::new().update_mapped(&music, &mappings, 0.1);
        assert_eq!(result.drive, 0.0);
        assert_eq!(result.accent, 0.0);
        assert!(result.phase.is_finite());
    }

    #[test]
    fn sustained_slow_and_fast_tempos_change_travel_at_equal_energy() {
        let at_tempo = |bpm| AudioFeatures {
            energy: 0.6,
            bpm,
            ..AudioFeatures::default()
        };
        let slow = run(&mut Choreographer::new(), &at_tempo(Some(80.0)), 60, 18);
        let unmeasured = run(&mut Choreographer::new(), &at_tempo(None), 60, 18);
        let fast = run(&mut Choreographer::new(), &at_tempo(Some(150.0)), 60, 18);
        assert_eq!(slow.drive, fast.drive);
        assert!(slow.travel_speed < unmeasured.travel_speed - 0.29);
        assert!(fast.travel_speed > unmeasured.travel_speed + 0.39);
        assert!((unmeasured.travel_speed - (0.65 + 0.6 * 0.85)).abs() < 0.002);
    }

    #[test]
    fn tempo_loss_and_a_new_source_decelerate_without_a_position_jump() {
        let fast = AudioFeatures {
            energy: 0.6,
            bpm: Some(150.0),
            ..AudioFeatures::default()
        };
        let unmeasured = AudioFeatures {
            bpm: None,
            ..fast.clone()
        };
        let mut controller = Choreographer::new();
        let before = run(&mut controller, &fast, 60, 18);
        let first = controller.update(&unmeasured, 1.0 / 60.0);
        assert!(first.travel_speed < before.travel_speed);
        assert!(before.travel_speed - first.travel_speed < 0.005);
        assert!(first.phase > before.phase && first.phase - before.phase < 0.03);
        let settled = run(&mut controller, &unmeasured, 60, 18);
        assert!((settled.travel_speed - 1.16).abs() < 0.002);
        controller.reset();
        let reset = controller.update(&unmeasured, 0.0);
        assert_eq!(reset.phase, settled.phase);
        assert_eq!(reset.travel_speed, settled.travel_speed);
        assert!(controller.tempo_candidate.is_none());
    }

    #[test]
    fn invalid_or_alternating_tempos_do_not_invent_rhythm() {
        let base = AudioFeatures {
            energy: 0.6,
            ..AudioFeatures::default()
        };
        for bpm in [f32::NAN, f32::INFINITY, -80.0, 0.0, 1000.0] {
            let invalid = AudioFeatures {
                bpm: Some(bpm),
                ..base.clone()
            };
            let quiet = run(&mut Choreographer::new(), &base, 60, 8);
            let result = run(&mut Choreographer::new(), &invalid, 60, 8);
            assert_eq!(quiet, result);
        }
        let mut unstable = Choreographer::new();
        let mut unmeasured = Choreographer::new();
        for frame in 0..1200 {
            let bpm = if (frame / 15) % 2 == 0 { 80.0 } else { 150.0 };
            let input = AudioFeatures {
                bpm: Some(bpm),
                ..base.clone()
            };
            assert_eq!(
                unstable.update(&input, 1.0 / 60.0),
                unmeasured.update(&base, 1.0 / 60.0)
            );
        }
    }

    #[test]
    fn rhythm_mapping_disconnects_tempo_without_disconnecting_frequency_energy() {
        let mappings = AudioMappings {
            beat_to_flash: 0.0,
            ..AudioMappings::default()
        };
        let base = AudioFeatures {
            bass: 0.6,
            mids: 0.6,
            treble: 0.6,
            ..AudioFeatures::default()
        };
        let fast = AudioFeatures {
            bpm: Some(150.0),
            ..base.clone()
        };
        let mut with_tempo = Choreographer::new();
        let mut without_tempo = Choreographer::new();
        for _ in 0..1200 {
            assert_eq!(
                with_tempo.update_mapped(&fast, &mappings, 1.0 / 60.0),
                without_tempo.update_mapped(&base, &mappings, 1.0 / 60.0)
            );
        }
        assert!(with_tempo.state.travel_speed > 1.0);
    }
}
