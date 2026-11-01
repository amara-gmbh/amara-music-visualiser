//! Bounded musical impacts, not a timer pretending to be drum hits.
//! Births use raw measured envelopes and physical time; only the travel of
//! already-born waves follows the user's smoothly integrated Pace.
use crate::{dsp::AudioFeatures, presets::AudioMappings};

const PRIMARY_RIPPLE_COUNT: usize = 12;
pub const RIPPLE_COUNT: usize = PRIMARY_RIPPLE_COUNT * 2;
const TAIL_SECONDS: f64 = 0.30;
// The shader's coupled secondary packet starts 0.12 travel-seconds later.
// Keep its shared event until that packet, too, finishes the age-6 fade.
pub const EVENT_MAX_AGE: f32 = 6.12;

pub struct RippleField {
    events: [[f32; 4]; RIPPLE_COUNT],
    tail_elapsed: [f64; PRIMARY_RIPPLE_COUNT],
    next: [usize; 3],
    baseline: [f32; 3],
    armed: [bool; 3],
    enabled: [bool; 3],
    cooldown: [f32; 3],
    beat_armed: bool,
    epoch: Option<u64>,
    serial: u32,
}

impl Default for RippleField {
    fn default() -> Self {
        Self {
            events: [[0.0; 4]; RIPPLE_COUNT],
            tail_elapsed: [0.0; PRIMARY_RIPPLE_COUNT],
            next: [0; 3],
            baseline: [0.0; 3],
            armed: [false; 3],
            enabled: [false; 3],
            cooldown: [0.0; 3],
            beat_armed: false,
            epoch: None,
            serial: 0,
        }
    }
}

impl RippleField {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn update(
        &mut self,
        audio: &AudioFeatures,
        mappings: &AudioMappings,
        fresh: bool,
        epoch: u64,
        dt: f32,
        travel_rate: f32,
    ) -> [[f32; 4]; RIPPLE_COUNT] {
        let clean = |x: f32| {
            if x.is_finite() {
                x.clamp(0.0, 1.5)
            } else {
                0.0
            }
        };
        let levels = [clean(audio.bass), clean(audio.mids), clean(audio.treble)];
        let gains = [
            mappings.bass_to_pulse,
            mappings.mids_to_geometry,
            mappings.treble_to_sparkle,
        ];
        if !fresh {
            self.reset();
            return self.events;
        }
        if self.epoch != Some(epoch) {
            self.reset();
            self.epoch = Some(epoch);
            self.baseline = levels;
            self.enabled = gains.map(|gain| gain > 0.0);
            // A reconnect to an already-held peak is not a new musical onset.
            self.beat_armed = clean(audio.beat) < 0.25;
            return self.events;
        }
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.1)
        } else {
            0.0
        };
        if dt == 0.0 {
            // A repeated UI pass may disconnect a mapping immediately, but it
            // must not consume an onset that the next timed pass should see.
            for band in 0..3 {
                if gains[band] <= 0.0 || !gains[band].is_finite() {
                    self.clear_band(band);
                    self.enabled[band] = false;
                    self.baseline[band] = levels[band];
                    self.armed[band] = false;
                }
            }
            return self.render_events();
        }
        let rate = if travel_rate.is_finite() {
            travel_rate.clamp(0.05, 10.8)
        } else {
            1.0
        };
        for (index, event) in self.events.iter_mut().enumerate() {
            if event[3] <= 0.0 {
                continue;
            }
            event[2] += dt * rate;
            let mut retired = false;
            if index >= PRIMARY_RIPPLE_COUNT {
                let tail = index - PRIMARY_RIPPLE_COUNT;
                self.tail_elapsed[tail] += f64::from(dt);
                retired = self.tail_elapsed[tail] + 1e-8 >= TAIL_SECONDS;
            }
            // Each packet's terminal 5.2–6.0 age fade reaches zero before this
            // hard storage clear, including the secondary's 0.12 age offset.
            // Replaced packets additionally have a short physical-time tail,
            // independent of the user's travel Pace.
            if event[2] >= EVENT_MAX_AGE || retired {
                *event = [0.0; 4];
            }
        }
        let beat = clean(audio.beat);
        let beat_edge = self.beat_armed && beat >= 0.62;
        if beat >= 0.62 {
            self.beat_armed = false;
        }
        if beat < 0.25 {
            self.beat_armed = true;
        }
        for band in 0..3 {
            self.cooldown[band] = (self.cooldown[band] - dt).max(0.0);
            if gains[band] <= 0.0 || !gains[band].is_finite() {
                self.clear_band(band);
                self.enabled[band] = false;
                self.baseline[band] = levels[band];
                self.armed[band] = false;
                continue;
            }
            if !self.enabled[band] {
                self.enabled[band] = true;
                self.baseline[band] = levels[band];
                self.armed[band] = false;
                continue;
            }
            let rise = levels[band] - self.baseline[band];
            if rise < 0.025 {
                self.armed[band] = true;
            }
            let flux_edge = self.armed[band] && rise > 0.075;
            if flux_edge {
                self.armed[band] = false;
            }
            let hit = flux_edge || (band == 0 && beat_edge && mappings.beat_to_flash > 0.0);
            // Consume an edge even inside the refractory window; never queue
            // it up to manufacture a later, unrelated splash.
            if dt > 0.0 && hit && self.cooldown[band] <= 0.0 && levels[band] > 0.035 {
                self.spawn(
                    band,
                    (levels[band] * 0.85 + rise.max(0.0) * 2.0).clamp(0.1, 1.5),
                );
                self.cooldown[band] = [0.18, 0.14, 0.10][band];
            }
            self.baseline[band] +=
                (levels[band] - self.baseline[band]) * (1.0 - (-dt / 0.32).exp());
        }
        self.render_events()
    }

    fn clear_band(&mut self, band: usize) {
        let start = band * 4;
        self.events[start..start + 4].fill([0.0; 4]);
        self.events[PRIMARY_RIPPLE_COUNT + start..PRIMARY_RIPPLE_COUNT + start + 4].fill([0.0; 4]);
        self.tail_elapsed[start..start + 4].fill(0.0);
    }

    fn render_events(&self) -> [[f32; 4]; RIPPLE_COUNT] {
        let mut rendered = self.events;
        for (index, event) in rendered[PRIMARY_RIPPLE_COUNT..].iter_mut().enumerate() {
            let t = (self.tail_elapsed[index] / TAIL_SECONDS).clamp(0.0, 1.0) as f32;
            // C1 endpoints: outgoing light starts at its exact prior strength
            // and reaches exact zero without a change in envelope velocity.
            // Scale a copy, never repeatedly multiply the stored strength.
            event[3] *= 1.0 - t * t * (3.0 - 2.0 * t);
        }
        rendered
    }

    fn spawn(&mut self, band: usize, strength: f32) {
        let phase = self.serial as f32 * 2.399963 + band as f32 * 1.7;
        let radius = [0.20, 0.56, 0.78][band];
        let index = band * 4 + self.next[band];
        // The retiring packet keeps its own centre and integrated age. A new
        // impact starts immediately; it does not wait for the old wave to fade.
        // Four slots × the shortest 0.10s cooldown exceeds the 0.30s tail, so
        // the same tail slot cannot be replaced before it reaches zero.
        self.events[PRIMARY_RIPPLE_COUNT + index] = self.events[index];
        self.tail_elapsed[index] = 0.0;
        self.events[index] = [
            phase.cos() * radius,
            phase.sin() * radius * 0.8,
            0.0,
            strength,
        ];
        self.next[band] = (self.next[band] + 1) % 4;
        self.serial = self.serial.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pulse() -> AudioFeatures {
        AudioFeatures {
            bass: 0.8,
            mids: 0.5,
            treble: 0.3,
            energy: 0.6,
            beat: 1.0,
            ..Default::default()
        }
    }
    #[test]
    fn held_peak_does_not_generate_periodic_splashes() {
        let mut field = RippleField::default();
        let mappings = AudioMappings::default();
        for _ in 0..600 {
            field.update(&pulse(), &mappings, true, 1, 1.0 / 60.0, 1.0);
        }
        assert_eq!(field.serial, 0);
    }
    #[test]
    fn separated_beats_spawn_at_each_render_rate_not_each_frame() {
        for hz in [30, 60, 120] {
            let mut field = RippleField::default();
            let mappings = AudioMappings::default();
            let mut audio = pulse();
            audio.beat = 0.0;
            field.update(&audio, &mappings, true, 1, 0.01, 1.0);
            for _ in 0..5 {
                for frame in 0..hz {
                    audio.beat = if frame < hz / 5 { 1.0 } else { 0.0 };
                    field.update(&audio, &mappings, true, 1, 1.0 / hz as f32, 1.0);
                }
            }
            assert_eq!(field.serial, 5, "{hz}Hz");
        }
    }
    #[test]
    fn mappings_stale_and_new_sources_clear_retained_impacts() {
        let mut field = RippleField::default();
        let mappings = AudioMappings::default();
        field.update(&AudioFeatures::default(), &mappings, true, 1, 0.016, 1.0);
        field.update(&AudioFeatures::default(), &mappings, true, 1, 0.016, 1.0);
        field.update(&pulse(), &mappings, true, 1, 0.016, 1.0);
        assert!(field.events.iter().any(|e| e[3] > 0.0));
        let off = AudioMappings {
            bass_to_pulse: 0.0,
            mids_to_geometry: 0.0,
            treble_to_sparkle: 0.0,
            beat_to_flash: 0.0,
        };
        assert_eq!(
            field.update(&pulse(), &off, true, 1, 0.0, 1.0),
            [[0.0; 4]; RIPPLE_COUNT]
        );
        assert_eq!(
            field.update(&pulse(), &mappings, false, 1, 0.016, 1.0),
            [[0.0; 4]; RIPPLE_COUNT]
        );
        assert_eq!(
            field.update(&pulse(), &mappings, true, 2, 0.016, 1.0),
            [[0.0; 4]; RIPPLE_COUNT]
        );
    }
    #[test]
    fn wave_travel_is_continuous_and_all_values_finite() {
        let mut field = RippleField::default();
        let mappings = AudioMappings::default();
        field.update(&AudioFeatures::default(), &mappings, true, 1, 0.016, 1.0);
        field.update(&pulse(), &mappings, true, 1, 0.016, 1.0);
        let event = field.events.iter().position(|e| e[3] > 0.0).unwrap();
        let before = field.events[event];
        field.update(&pulse(), &mappings, true, 1, 0.1, 2.0);
        assert!((field.events[event][2] - before[2] - 0.2).abs() < 1e-6);
        field.update(&pulse(), &mappings, true, 1, f32::NAN, f32::INFINITY);
        assert!(field.events.iter().flatten().all(|v| v.is_finite()));
    }

    #[test]
    fn repeated_ui_pass_does_not_swallow_a_real_beat_or_band_onset() {
        let mut field = RippleField::default();
        let mappings = AudioMappings::default();
        let quiet = AudioFeatures::default();
        field.update(&quiet, &mappings, true, 1, 0.016, 1.0);
        field.update(&quiet, &mappings, true, 1, 0.016, 1.0);
        let before = field.events;
        for dt in [0.0, f32::NAN, -1.0] {
            assert_eq!(field.update(&pulse(), &mappings, true, 1, dt, 1.0), before);
            assert_eq!(field.serial, 0);
        }
        field.update(&pulse(), &mappings, true, 1, 0.016, 1.0);
        assert_eq!(field.serial, 3);
    }

    #[test]
    fn cooldown_consumes_an_early_beat_without_delaying_it_into_silence() {
        let mut field = RippleField::default();
        let mappings = AudioMappings::default();
        let mut audio = pulse();
        audio.beat = 0.0;
        field.update(&audio, &mappings, true, 1, 0.01, 1.0);
        audio.beat = 1.0;
        field.update(&audio, &mappings, true, 1, 0.01, 1.0);
        assert_eq!(field.serial, 1);
        audio.beat = 0.0;
        field.update(&audio, &mappings, true, 1, 0.01, 1.0);
        audio.beat = 1.0;
        field.update(&audio, &mappings, true, 1, 0.01, 1.0);
        for _ in 0..20 {
            field.update(&audio, &mappings, true, 1, 0.1, 1.0);
        }
        assert_eq!(field.serial, 1);
        for _ in 0..70 {
            field.update(&AudioFeatures::default(), &mappings, true, 1, 0.1, 1.0);
        }
        assert_eq!(field.serial, 1);
        assert!(field.events.iter().all(|event| event[3] == 0.0));
    }

    #[test]
    fn individual_mapping_disconnects_only_its_impacts_and_does_not_retrigger() {
        for band in 0..3 {
            let mut field = RippleField::default();
            let mut mappings = AudioMappings::default();
            let quiet = AudioFeatures::default();
            field.update(&quiet, &mappings, true, 1, 0.016, 1.0);
            field.update(&quiet, &mappings, true, 1, 0.016, 1.0);
            field.update(&pulse(), &mappings, true, 1, 0.016, 1.0);
            match band {
                0 => mappings.bass_to_pulse = 0.0,
                1 => mappings.mids_to_geometry = 0.0,
                _ => mappings.treble_to_sparkle = 0.0,
            }
            field.update(&pulse(), &mappings, true, 1, 0.0, 1.0);
            for lane in 0..3 {
                assert_eq!(
                    field.events[lane * 4..lane * 4 + 4]
                        .iter()
                        .any(|event| event[3] > 0.0),
                    lane != band
                );
            }
            let serial = field.serial;
            field.update(&pulse(), &AudioMappings::default(), true, 1, 0.1, 1.0);
            assert_eq!(
                field.serial, serial,
                "reconnecting a held level is not a hit"
            );
        }
    }

    #[test]
    fn disabled_beat_mapping_does_not_birth_from_a_beat_only_edge() {
        let mut field = RippleField::default();
        let mappings = AudioMappings {
            beat_to_flash: 0.0,
            ..AudioMappings::default()
        };
        let mut audio = pulse();
        audio.beat = 0.0;
        field.update(&audio, &mappings, true, 1, 0.1, 1.0);
        audio.beat = 1.0;
        field.update(&audio, &mappings, true, 1, 0.1, 1.0);
        assert_eq!(field.serial, 0);
    }

    #[test]
    fn recycled_packet_retains_its_position_age_and_strength_until_smooth_retirement() {
        for hz in [30, 60, 120] {
            let mut field = RippleField::default();
            let quiet = AudioFeatures::default();
            let mappings = AudioMappings::default();
            let dt = 1.0 / hz as f32;
            field.update(&quiet, &mappings, true, 1, dt, 0.05);
            for _ in 0..4 {
                field.spawn(0, 0.8);
            }
            field.events[0][2] = 1.3;
            let outgoing = field.events[0];
            field.spawn(0, 1.2);
            let born = field.render_events();
            assert_eq!(born[PRIMARY_RIPPLE_COUNT], outgoing);
            assert_eq!(born[0][2], 0.0, "replacement is born without delay");
            assert_eq!(born[0][3], 1.2);
            assert_eq!(field.update(&quiet, &mappings, true, 1, 0.0, 0.05), born);
            for frame in 1..=hz * 3 / 10 {
                let shown = field.update(&quiet, &mappings, true, 1, dt, 0.05);
                let t = (f64::from(frame as f32 * dt) / TAIL_SECONDS).min(1.0) as f32;
                if frame < hz * 3 / 10 {
                    let tail = shown[PRIMARY_RIPPLE_COUNT];
                    assert_eq!(&tail[..2], &outgoing[..2]);
                    assert!((tail[2] - outgoing[2] - frame as f32 * dt * 0.05).abs() < 2e-6);
                    assert!((tail[3] - outgoing[3] * (1.0 - t * t * (3.0 - 2.0 * t))).abs() < 1e-6);
                    assert_eq!(field.events[PRIMARY_RIPPLE_COUNT][3], outgoing[3]);
                } else {
                    assert_eq!(shown[PRIMARY_RIPPLE_COUNT], [0.0; 4]);
                    assert_eq!(field.events[PRIMARY_RIPPLE_COUNT], [0.0; 4]);
                }
            }
        }
    }

    #[test]
    fn dense_onsets_at_slow_pace_never_replace_an_unfinished_tail() {
        for hz in [30, 60, 120] {
            let mut field = RippleField::default();
            let mappings = AudioMappings::default();
            let quiet = AudioFeatures::default();
            let dt = 1.0 / hz as f32;
            field.update(&quiet, &mappings, true, 1, dt, 0.05);
            field.update(&quiet, &mappings, true, 1, dt, 0.05);
            let mut recycled = [0; 3];
            for frame in 0..hz * 6 {
                let audio = if frame % (hz / 10) == 0 {
                    pulse()
                } else {
                    quiet.clone()
                };
                let before = field.events;
                let tail_times = field.tail_elapsed;
                let shown = field.update(&audio, &mappings, true, 1, dt, 0.05);
                for index in 0..PRIMARY_RIPPLE_COUNT {
                    if shown[index][3] > 0.0 && shown[index][2] == 0.0 && before[index][3] > 0.0 {
                        recycled[index / 4] += 1;
                        assert!(
                            before[PRIMARY_RIPPLE_COUNT + index][3] == 0.0
                                || tail_times[index] + f64::from(dt) + 1e-8 >= TAIL_SECONDS,
                            "{hz}Hz: an unfinished tail was overwritten"
                        );
                        let tail = shown[PRIMARY_RIPPLE_COUNT + index];
                        assert_eq!(&tail[..2], &before[index][..2]);
                        assert!((tail[2] - before[index][2] - dt * 0.05).abs() < 1e-6);
                        assert_eq!(tail[3], before[index][3]);
                    }
                }
                assert!(shown.iter().flatten().all(|value| value.is_finite()));
            }
            assert!(recycled.into_iter().all(|count| count >= 4));
        }
    }

    #[test]
    fn mapping_source_and_freshness_boundaries_also_clear_outgoing_tails() {
        for boundary in 0..3 {
            let mut field = RippleField::default();
            let mut mappings = AudioMappings::default();
            let quiet = AudioFeatures::default();
            field.update(&quiet, &mappings, true, 1, 0.016, 1.0);
            for band in 0..3 {
                for _ in 0..5 {
                    field.spawn(band, 0.7);
                }
            }
            assert!(
                field.events[PRIMARY_RIPPLE_COUNT..]
                    .iter()
                    .any(|event| event[3] > 0.0)
            );
            if boundary == 0 {
                mappings.bass_to_pulse = 0.0;
                mappings.mids_to_geometry = 0.0;
                mappings.treble_to_sparkle = 0.0;
                mappings.beat_to_flash = 0.0;
            }
            let shown = field.update(
                &quiet,
                &mappings,
                boundary != 1,
                if boundary == 2 { 2 } else { 1 },
                0.0,
                1.0,
            );
            assert_eq!(shown, [[0.0; 4]; RIPPLE_COUNT]);
        }
    }

    #[test]
    fn primary_storage_survives_until_the_delayed_secondary_finishes_its_fade() {
        for hz in [30, 60, 120] {
            for rate in [0.05, 1.0, 10.8] {
                let mut field = RippleField::default();
                let quiet = AudioFeatures::default();
                let mappings = AudioMappings::default();
                let dt = 1.0 / hz as f32;
                field.update(&quiet, &mappings, true, 1, dt, rate);
                field.spawn(1, 0.7);
                field.events[4][2] = 5.99;
                let mut crossed_primary_end = false;
                for _ in 0..hz * 4 {
                    let previous = field.events[4];
                    let expected_age = previous[2] + dt * rate;
                    let shown = field.update(&quiet, &mappings, true, 1, dt, rate);
                    if expected_age >= EVENT_MAX_AGE {
                        assert_eq!(shown[4], [0.0; 4], "{hz}Hz, rate {rate}");
                        // The shader's secondary age is event age minus 0.12:
                        // storage retires only once its own age-6 fade is zero.
                        assert!(expected_age - 0.12 >= 6.0 - 1e-6);
                        break;
                    }
                    assert_eq!(&shown[4][..2], &previous[..2]);
                    assert_eq!(shown[4][3], previous[3]);
                    assert!((shown[4][2] - expected_age).abs() < 1e-6);
                    crossed_primary_end |= shown[4][2] >= 6.0;
                }
                assert_eq!(field.events[4], [0.0; 4]);
                if dt * rate < 0.12 {
                    assert!(crossed_primary_end, "{hz}Hz, rate {rate}");
                }
                assert_eq!(field.serial, 1, "retirement must never create an onset");
            }
        }
    }

    #[test]
    fn late_secondary_retention_does_not_extend_replacement_tail_time() {
        let mut field = RippleField::default();
        let quiet = AudioFeatures::default();
        let mappings = AudioMappings::default();
        field.update(&quiet, &mappings, true, 1, 0.01, 0.05);
        for _ in 0..4 {
            field.spawn(2, 0.6);
        }
        field.events[8][2] = 6.01;
        field.spawn(2, 0.8);
        assert_eq!(field.render_events()[PRIMARY_RIPPLE_COUNT + 8][2], 6.01);
        for _ in 0..3 {
            field.update(&quiet, &mappings, true, 1, 0.1, 0.05);
        }
        assert_eq!(field.events[PRIMARY_RIPPLE_COUNT + 8], [0.0; 4]);
        assert!(field.events[8][3] > 0.0, "new impact stays independent");
    }
}
