use crate::{
    dsp::AudioFeatures,
    presets::{AudioMappings, MathForm},
};

const MORPH_SECONDS: f64 = 6.0;
const FAMILIES: usize = 4;
const FAMILY_NAMES: [&str; FAMILIES] = [
    "Julia",
    "Inversion Lace",
    "Harmonic Sculpture",
    "Vortex Weave",
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MathJourneyState {
    pub from: usize,
    pub to: usize,
    pub blend: f32,
    pub phase: f32,
}

impl MathJourneyState {
    pub fn family_name(self) -> &'static str {
        FAMILY_NAMES[if self.blend < 0.5 { self.from } else { self.to }]
    }
}

#[derive(Default)]
struct MusicalEvidence {
    seconds: f64,
    energy: f64,
    brightness: f64,
    transients: f64,
    tempo: f64,
}

impl MusicalEvidence {
    fn add(&mut self, levels: [f64; 4], dt: f64) {
        self.seconds += dt;
        self.energy += levels[0] * dt;
        self.brightness += levels[1] * dt;
        self.transients += levels[2] * dt;
        self.tempo += levels[3] * dt;
    }

    fn average(&self) -> [f64; 4] {
        if self.seconds == 0.0 {
            [0.0; 4]
        } else {
            [self.energy, self.brightness, self.transients, self.tempo]
                .map(|value| (value / self.seconds).clamp(0.0, 1.0))
        }
    }
}

/// Deterministic visual variety, not AI or detected song sections. Mapped
/// signal evidence influences held phrases; it cannot change an in-flight
/// morph's endpoints. Silence continues a gentle mathematical exploration.
pub struct MathJourney {
    from: usize,
    to: usize,
    morph_elapsed: f64,
    queued: Option<usize>,
    selection: MathForm,
    held_seconds: f64,
    evidence: MusicalEvidence,
    slow_energy: f64,
    slow_brightness: f64,
    phase: f64,
    visited: u8,
    decisions: u64,
}

impl Default for MathJourney {
    fn default() -> Self {
        Self {
            from: 0,
            to: 0,
            morph_elapsed: MORPH_SECONDS,
            queued: None,
            selection: MathForm::Journey,
            held_seconds: 0.0,
            evidence: MusicalEvidence::default(),
            slow_energy: 0.0,
            slow_brightness: 0.0,
            phase: 0.0,
            visited: 1,
            decisions: 0,
        }
    }
}

impl MathJourney {
    pub fn new() -> Self {
        Self::default()
    }

    /// A new audio source must gather fresh evidence. Preserve the current
    /// family, queued choice, morph progress, held time and integrated position.
    pub fn reset_evidence(&mut self) {
        self.evidence = MusicalEvidence::default();
        self.slow_energy = 0.0;
        self.slow_brightness = 0.0;
    }

    pub fn update(
        &mut self,
        features: &AudioFeatures,
        mappings: &AudioMappings,
        selection: MathForm,
        dt: f32,
    ) -> MathJourneyState {
        let dt = if dt.is_finite() {
            f64::from(dt.clamp(0.0, 0.25))
        } else {
            0.0
        };
        // A repeated egui pass or pause cannot queue a choice, consume a hold,
        // select a new family, or advance phase.
        if dt == 0.0 {
            return self.state();
        }
        if selection != self.selection {
            if selection == MathForm::Journey {
                self.queued = None;
                self.held_seconds = 0.0;
                self.visited = 1 << self.to;
            }
            self.selection = selection;
        }
        if let Some(family) = selection.family() {
            self.request(family);
        }

        let levels = mapped_levels(features, mappings);
        self.evidence.add(levels, dt);
        // Integrate the exponential envelopes analytically. Constant inputs
        // therefore produce the same phase at 30, 60 or 120 updates per second.
        let energy_area = advance_envelope(&mut self.slow_energy, levels[0], dt, 3.0);
        let brightness_area = advance_envelope(&mut self.slow_brightness, levels[1], dt, 5.0);
        self.phase += dt * 0.16 + energy_area * 0.035 + brightness_area * 0.015;
        self.advance(dt);
        self.state()
    }

    fn state(&self) -> MathJourneyState {
        let t = (self.morph_elapsed / MORPH_SECONDS).clamp(0.0, 1.0);
        MathJourneyState {
            from: self.from,
            to: self.to,
            blend: (t * t * t * (t * (t * 6.0 - 15.0) + 10.0)) as f32,
            phase: self.phase as f32,
        }
    }

    fn request(&mut self, family: usize) {
        if family == self.to {
            self.queued = None;
        } else if self.morph_elapsed < MORPH_SECONDS {
            self.queued = Some(family);
        } else {
            self.begin(family);
        }
    }

    fn begin(&mut self, family: usize) {
        self.from = self.to;
        self.to = family;
        self.morph_elapsed = 0.0;
        self.held_seconds = 0.0;
        self.visited |= 1 << family;
        self.evidence = MusicalEvidence::default();
    }

    fn hold_duration(&self) -> f64 {
        let [energy, _, transients, tempo] = self.evidence.average();
        (22.0 - energy * 6.0 - transients * 2.0 - tempo * 2.0).clamp(12.0, 22.0)
    }

    fn advance(&mut self, mut remaining: f64) {
        while remaining > 1e-10 {
            if self.morph_elapsed < MORPH_SECONDS {
                let consumed = remaining.min(MORPH_SECONDS - self.morph_elapsed);
                self.morph_elapsed += consumed;
                remaining -= consumed;
                if self.morph_elapsed + 1e-9 >= MORPH_SECONDS {
                    self.morph_elapsed = MORPH_SECONDS;
                    if let Some(family) = self.queued.take() {
                        self.begin(family);
                    }
                }
            } else if self.selection == MathForm::Journey {
                let duration = self.hold_duration();
                let consumed = remaining.min((duration - self.held_seconds).max(0.0));
                self.held_seconds += consumed;
                remaining -= consumed;
                if self.held_seconds + 1e-9 >= duration {
                    let family = self.choose();
                    self.begin(family);
                }
            } else {
                break;
            }
        }
    }

    fn choose(&mut self) -> usize {
        let all = (1 << FAMILIES) - 1;
        if self.visited == all {
            self.visited = 1 << self.to;
        }
        let [energy, brightness, transients, tempo] = self.evidence.average();
        let character = [
            0.55 * energy + 0.30 * (1.0 - brightness),
            0.65 * (1.0 - energy) + 0.45 * brightness,
            0.55 * (1.0 - transients) + 0.25 * (1.0 - brightness),
            0.40 * energy + 0.40 * transients + 0.20 * tempo,
        ];
        let mut next = (self.to + 1) % FAMILIES;
        let mut best = f64::NEG_INFINITY;
        for (family, preference) in character.into_iter().enumerate() {
            if self.visited & (1 << family) != 0 || family == self.to {
                continue;
            }
            // A slow, deterministic tie-break adds variety at phrase boundaries.
            // There is no frame-rate RNG, genre label dependency or beat cut.
            let drift = (self.phase * 0.13 + self.decisions as f64 * 1.618 + family as f64 * 2.4)
                .sin()
                * 0.12;
            let score = preference + drift;
            if score > best {
                next = family;
                best = score;
            }
        }
        self.decisions = self.decisions.wrapping_add(1);
        next
    }
}

fn mapped_levels(features: &AudioFeatures, mappings: &AudioMappings) -> [f64; 4] {
    let bass = unit(features.bass) * gain(mappings.bass_to_pulse);
    let mids = unit(features.mids) * gain(mappings.mids_to_geometry);
    let air = unit(features.treble) * gain(mappings.treble_to_sparkle);
    let total = bass + mids + air;
    let energy = (total / 3.0).clamp(0.0, 1.0);
    let brightness = if total > 0.001 { air / total } else { 0.0 };
    let transient = (unit(features.transient) * gain(mappings.beat_to_flash)).min(1.0);
    let tempo = if energy > 0.005 {
        features
            .bpm
            .filter(|value| value.is_finite() && *value > 0.0)
            .map_or(0.0, |value| {
                ((f64::from(value) - 60.0) / 120.0).clamp(0.0, 1.0)
            })
            * gain(mappings.beat_to_flash).min(1.0)
    } else {
        0.0
    };
    [energy, brightness, transient, tempo]
}

fn unit(value: f32) -> f64 {
    if value.is_finite() {
        f64::from(value.clamp(0.0, 1.0))
    } else {
        0.0
    }
}

fn gain(value: f32) -> f64 {
    if value.is_finite() {
        f64::from(value.clamp(0.0, 1.5))
    } else {
        0.0
    }
}

fn advance_envelope(current: &mut f64, target: f64, dt: f64, tau: f64) -> f64 {
    let decay = (-dt / tau).exp();
    let area = target * dt + (*current - target) * tau * (1.0 - decay);
    *current = target + (*current - target) * decay;
    area.max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn music() -> AudioFeatures {
        AudioFeatures {
            bass: 0.9,
            mids: 0.75,
            treble: 0.6,
            energy: 0.9,
            transient: 0.7,
            beat: 1.0,
            bpm: Some(128.0),
            ..AudioFeatures::default()
        }
    }

    fn run(
        journey: &mut MathJourney,
        selection: MathForm,
        seconds: usize,
        hz: usize,
    ) -> MathJourneyState {
        let mut state = journey.state();
        for _ in 0..seconds * hz {
            state = journey.update(
                &music(),
                &AudioMappings::default(),
                selection,
                1.0 / hz as f32,
            );
        }
        state
    }

    #[test]
    fn manual_morph_has_six_second_quintic_endpoints() {
        let mut journey = MathJourney::new();
        let midpoint = run(&mut journey, MathForm::InversionLace, 3, 60);
        assert_eq!((midpoint.from, midpoint.to), (0, 1));
        assert!((midpoint.blend - 0.5).abs() < 0.000_01);
        let end = run(&mut journey, MathForm::InversionLace, 3, 60);
        assert_eq!(end.blend, 1.0);
        assert_eq!(end.family_name(), "Inversion Lace");
        let next = journey.update(
            &music(),
            &AudioMappings::default(),
            MathForm::VortexWeave,
            0.001,
        );
        assert_eq!((next.from, next.to), (1, 3));
        assert!(next.blend < 0.000_001);
    }

    #[test]
    fn rapid_manual_choices_keep_endpoints_and_queue_only_the_latest() {
        let mut journey = MathJourney::new();
        run(&mut journey, MathForm::InversionLace, 2, 10);
        let before = journey.state();
        journey.update(
            &music(),
            &AudioMappings::default(),
            MathForm::HarmonicSculpture,
            0.1,
        );
        let queued = journey.update(
            &music(),
            &AudioMappings::default(),
            MathForm::VortexWeave,
            0.1,
        );
        assert_eq!((queued.from, queued.to), (0, 1));
        assert!(queued.blend > before.blend);
        for _ in 0..38 {
            journey.update(
                &music(),
                &AudioMappings::default(),
                MathForm::VortexWeave,
                0.1,
            );
        }
        let next = journey.state();
        assert_eq!((next.from, next.to), (1, 3));
        assert!(next.blend < 0.000_001);
    }

    #[test]
    fn returning_to_target_cancels_a_pending_manual_choice() {
        let mut journey = MathJourney::new();
        run(&mut journey, MathForm::InversionLace, 2, 60);
        journey.update(
            &music(),
            &AudioMappings::default(),
            MathForm::VortexWeave,
            0.1,
        );
        run(&mut journey, MathForm::InversionLace, 5, 60);
        assert_eq!((journey.state().to, journey.state().blend), (1, 1.0));
    }

    #[test]
    fn disconnected_mappings_make_quiet_and_music_exactly_identical() {
        let off = AudioMappings {
            bass_to_pulse: 0.0,
            mids_to_geometry: 0.0,
            treble_to_sparkle: 0.0,
            beat_to_flash: 0.0,
        };
        let mut quiet = MathJourney::new();
        let mut loud = MathJourney::new();
        for _ in 0..180 * 60 {
            let a = quiet.update(
                &AudioFeatures::default(),
                &off,
                MathForm::Journey,
                1.0 / 60.0,
            );
            let b = loud.update(&music(), &off, MathForm::Journey, 1.0 / 60.0);
            assert_eq!(a, b);
        }
        assert!(
            quiet.decisions >= 4,
            "Silence should still explore mathematics"
        );
    }

    #[test]
    fn constant_music_is_consistent_at_thirty_sixty_and_one_twenty_hz() {
        let timelines: Vec<Vec<_>> = [30, 60, 120]
            .into_iter()
            .map(|hz| {
                let mut journey = MathJourney::new();
                (0..6)
                    .map(|_| run(&mut journey, MathForm::Journey, 20, hz))
                    .collect()
            })
            .collect();
        for timeline in &timelines[1..] {
            for (reference, state) in timelines[0].iter().zip(timeline) {
                assert_eq!((state.from, state.to), (reference.from, reference.to));
                assert!((state.blend - reference.blend).abs() < 0.005);
                assert!((state.phase - reference.phase).abs() < 0.000_1);
            }
        }
    }

    #[test]
    fn source_reset_forgets_evidence_but_preserves_the_visible_journey() {
        let mut journey = MathJourney::new();
        let before = run(&mut journey, MathForm::HarmonicSculpture, 3, 60);
        assert!(journey.evidence.seconds > 0.0);
        journey.reset_evidence();
        assert_eq!(journey.evidence.seconds, 0.0);
        assert_eq!(journey.state(), before);
        let paused = journey.update(
            &music(),
            &AudioMappings::default(),
            MathForm::VortexWeave,
            0.0,
        );
        assert_eq!(paused, before);
        assert!(journey.queued.is_none());
        let resumed = journey.update(
            &music(),
            &AudioMappings::default(),
            MathForm::HarmonicSculpture,
            1.0 / 60.0,
        );
        assert_eq!((resumed.from, resumed.to), (before.from, before.to));
        assert!(resumed.phase > before.phase && resumed.phase - before.phase < 0.01);
    }

    #[test]
    fn automatic_choices_visit_every_family_without_immediate_repeats() {
        let mut journey = MathJourney::new();
        let mut chosen = vec![0];
        for _ in 0..240 * 30 {
            let state = journey.update(
                &music(),
                &AudioMappings::default(),
                MathForm::Journey,
                1.0 / 30.0,
            );
            if chosen.last() != Some(&state.to) {
                chosen.push(state.to);
            }
        }
        assert!(chosen.len() >= 8);
        assert!(
            chosen[..4]
                .iter()
                .fold(0_u8, |mask, family| mask | (1 << family))
                == 15
        );
        assert!(chosen.windows(2).all(|pair| pair[0] != pair[1]));
    }

    #[test]
    fn sustained_music_changes_phrase_holds_without_using_genre_labels() {
        let mut quiet = MathJourney::new();
        let mut loud = MathJourney::new();
        for _ in 0..10 * 60 {
            quiet.update(
                &AudioFeatures::default(),
                &AudioMappings::default(),
                MathForm::Journey,
                1.0 / 60.0,
            );
            loud.update(
                &music(),
                &AudioMappings::default(),
                MathForm::Journey,
                1.0 / 60.0,
            );
        }
        assert_eq!(quiet.hold_duration(), 22.0);
        assert!(loud.hold_duration() >= 12.0 && loud.hold_duration() < 18.0);
        assert!(loud.state().phase > quiet.state().phase);
    }

    #[test]
    fn fixed_family_stays_fixed_while_its_internal_phase_evolves() {
        let mut journey = MathJourney::new();
        let state = run(&mut journey, MathForm::Julia, 90, 30);
        assert_eq!((state.from, state.to, state.blend), (0, 0, 1.0));
        assert!(state.phase > 10.0);
        assert_eq!(journey.decisions, 0);
    }

    #[test]
    fn invalid_inputs_and_pause_cannot_create_nan_or_change_choices() {
        let mut journey = MathJourney::new();
        let before = journey.state();
        let invalid = AudioFeatures {
            bass: f32::NAN,
            mids: f32::INFINITY,
            treble: f32::NEG_INFINITY,
            transient: f32::NAN,
            bpm: Some(f32::INFINITY),
            ..music()
        };
        let invalid_maps = AudioMappings {
            bass_to_pulse: f32::NAN,
            mids_to_geometry: -1.0,
            treble_to_sparkle: f32::INFINITY,
            beat_to_flash: f32::NEG_INFINITY,
        };
        for dt in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(
                journey.update(&invalid, &invalid_maps, MathForm::VortexWeave, dt),
                before
            );
        }
        let state = journey.update(&invalid, &invalid_maps, MathForm::Journey, 0.016);
        assert!(state.phase.is_finite() && state.blend.is_finite());
        assert!((0.0..=1.0).contains(&state.blend));
        assert!(state.from < FAMILIES && state.to < FAMILIES);
    }
}
