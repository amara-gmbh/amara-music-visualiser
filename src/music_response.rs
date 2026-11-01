//! Smooth, measured musical emphasis shared by every visual world.
//! This is articulation/onset activity, not a beat-grid oscillator, genre
//! classifier, loudness multiplier or camera controller. No events are random.
use crate::{dsp::AudioFeatures, presets::AudioMappings};

#[derive(Default)]
pub struct MusicResponse {
    epoch: Option<u64>,
    output: [f64; 4],
    onset_mass: f64,
    transient_floor: f64,
    fast_activity: f64,
    activity_baseline: f64,
    since_onset: f64,
    primed: bool,
    armed: bool,
}

impl MusicResponse {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn clear(&mut self) {
        let epoch = self.epoch;
        self.reset();
        self.epoch = epoch;
    }

    /// [rhythm drive, pickup envelope, low pulse, air detail], all 0..1.
    ///
    /// Drive integrates actual onset density, with ~0.65 s attack / 2 s release.
    /// Pickup follows activity rising above its slower baseline, only while
    /// measured onsets are recent; steady high volume cannot produce it.
    /// Low/air follow their real mapped bands with soft material accents.
    /// Beat mapping gates drive/pickup; bass/air mappings independently gate
    /// channels 2/3; mids contribute only to the mapped activity evidence.
    ///
    /// Supply raw worker features and physical dt, not motion-scaled time.
    /// Epoch changes, stale/silent sources and disconnected gates cannot replay
    /// an old peak. A new stream primes its edge detector before accepting one.
    pub fn update(
        &mut self,
        audio: &AudioFeatures,
        mappings: &AudioMappings,
        fresh: bool,
        epoch: u64,
        dt: f32,
    ) -> [f32; 4] {
        if self.epoch != Some(epoch) {
            self.reset();
            self.epoch = Some(epoch);
        }
        let gains = [
            gain(mappings.bass_to_pulse),
            gain(mappings.mids_to_geometry),
            gain(mappings.treble_to_sparkle),
        ];
        let rhythm_gain = gain(mappings.beat_to_flash);
        let bands = [unit(audio.bass), unit(audio.mids), unit(audio.treble)];
        let bass = (bands[0] * gains[0]).min(1.0);
        let mids = (bands[1] * gains[1]).min(1.0);
        let air = (bands[2] * gains[2]).min(1.0);
        let material = ((bass + mids * 0.90 + air * 0.75) / 2.65).clamp(0.0, 1.0);
        let energy = unit(audio.energy);
        if !fresh || energy <= 0.008 || material <= 0.002 || !dt.is_finite() || dt > 0.5 {
            self.clear();
            return [0.0; 4];
        }
        // Gates act even in a repeated/paused UI pass. Re-enabling rhythm must
        // prime again, never reinterpret a previously held peak as a new edge.
        if rhythm_gain == 0.0 {
            self.output[0] = 0.0;
            self.output[1] = 0.0;
            self.onset_mass = 0.0;
            self.primed = false;
            self.armed = false;
        }
        if gains[0] == 0.0 {
            self.output[2] = 0.0;
        }
        if gains[2] == 0.0 {
            self.output[3] = 0.0;
        }
        if dt <= 0.0 {
            return self.uniforms();
        }
        let dt = f64::from(dt);
        let beat = unit(audio.beat);
        let transient = unit(audio.transient);
        if rhythm_gain > 0.0 {
            if !self.primed {
                self.transient_floor = transient;
                self.fast_activity = material * 0.22;
                self.activity_baseline = self.fast_activity;
                self.since_onset = 10.0;
                self.armed = beat < 0.25;
                self.primed = true;
            } else {
                self.since_onset += dt;
                let novelty = ((transient - self.transient_floor - 0.025)
                    / (0.18 + self.transient_floor * 0.45))
                    .clamp(0.0, 1.0);
                let onset = beat.max(novelty);
                if onset < 0.25 {
                    self.armed = true;
                }
                let rising = self.armed && onset >= 0.62;
                if rising {
                    // Consume cooldown-suppressed edges too: no delayed replay.
                    self.armed = false;
                }
                self.onset_mass *= (-dt / 1.4).exp();
                if rising && self.since_onset >= 0.18 {
                    let audible = smooth((energy - 0.008) / 0.052);
                    let strength = onset * audible * (0.45 + material * 0.55);
                    self.onset_mass = (self.onset_mass + strength * 0.40).min(2.5);
                    self.since_onset = 0.0;
                }
                self.transient_floor = follow(self.transient_floor, transient, dt, 2.4);
            }
            // Leave headroom above an ordinary two-onset/second rhythm so an
            // actual busier passage can still create a distinct pickup.
            let density = (self.onset_mass * 0.85).min(1.0);
            let drive = (density * rhythm_gain).min(1.0);
            self.output[0] = envelope(self.output[0], drive, dt, 0.65, 2.0);
            self.fast_activity = follow(
                self.fast_activity,
                density * 0.78 + material * 0.22,
                dt,
                0.45,
            );
            self.activity_baseline = follow(self.activity_baseline, self.fast_activity, dt, 7.0);
            let growth = (self.fast_activity - self.activity_baseline - 0.025).max(0.0);
            let pickup = if self.since_onset <= 1.4 {
                (growth * 3.8 * rhythm_gain).min(1.0)
            } else {
                0.0
            };
            self.output[1] = envelope(self.output[1], pickup, dt, 0.50, 2.0);
        }
        let low = bass * (0.85 + self.output[0] * 0.15);
        let detail = air * (0.75 + self.output[0] * 0.25);
        self.output[2] = envelope(self.output[2], low, dt, 0.20, 0.65);
        self.output[3] = envelope(self.output[3], detail, dt, 0.30, 0.85);
        self.uniforms()
    }

    fn uniforms(&self) -> [f32; 4] {
        self.output.map(|value| value.clamp(0.0, 1.0) as f32)
    }
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

fn follow(current: f64, target: f64, dt: f64, tau: f64) -> f64 {
    target + (current - target) * (-dt / tau).exp()
}

fn envelope(current: f64, target: f64, dt: f64, attack: f64, release: f64) -> f64 {
    follow(
        current,
        target,
        dt,
        if target > current { attack } else { release },
    )
}

fn smooth(value: f64) -> f64 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn material() -> AudioFeatures {
        AudioFeatures {
            energy: 0.6,
            bass: 0.65,
            mids: 0.55,
            treble: 0.40,
            ..AudioFeatures::default()
        }
    }

    fn pulse(time: f64, rate: f64) -> AudioFeatures {
        let mut audio = material();
        let since = (time * rate).rem_euclid(1.0) / rate;
        audio.beat = (-since * 8.0).exp() as f32;
        audio.transient = audio.beat * 0.8;
        audio
    }

    fn tick(response: &mut MusicResponse, time: f64, hz: usize, rate: f64) -> [f32; 4] {
        response.update(
            &pulse(time, rate),
            &AudioMappings::default(),
            true,
            1,
            1.0 / hz as f32,
        )
    }

    #[test]
    fn steady_loudness_and_held_peaks_do_not_invent_rhythm_or_pickup() {
        for held in [false, true] {
            let mut response = MusicResponse::new();
            let mut audio = material();
            if held {
                audio.beat = 1.0;
                audio.transient = 1.0;
            }
            for _ in 0..600 {
                let out = response.update(&audio, &AudioMappings::default(), true, 1, 1.0 / 60.0);
                assert_eq!(out[0], 0.0);
                assert_eq!(out[1], 0.0);
                assert!(out.iter().all(|value| value.is_finite()));
            }
            assert!(response.uniforms()[2] > 0.4);
            assert!(response.uniforms()[3] > 0.2);
        }
    }

    #[test]
    fn measured_onsets_build_visible_but_fluid_drive_and_pickup_then_settle() {
        let mut response = MusicResponse::new();
        let mut previous = [0.0; 4];
        let mut pickup = 0.0_f32;
        for frame in 0..60 * 24 {
            let out = tick(&mut response, frame as f64 / 60.0, 60, 2.0);
            pickup = pickup.max(out[1]);
            for channel in 0..4 {
                assert!((out[channel] - previous[channel]).abs() < 0.085);
            }
            previous = out;
        }
        assert!(previous[0] > 0.5, "{previous:?}");
        assert!(pickup > 0.35, "pickup {pickup}");
        assert!(
            previous[1] < 0.18,
            "steady rhythm should cease being a pickup: {previous:?}"
        );
        let mut stronger_pickup = 0.0_f32;
        for frame in 0..60 * 4 {
            stronger_pickup =
                stronger_pickup.max(tick(&mut response, frame as f64 / 60.0, 60, 4.0)[1]);
        }
        assert!(
            stronger_pickup > 0.20,
            "new density should lift: {stronger_pickup}"
        );
    }

    #[test]
    fn silence_stale_and_source_changes_clear_without_replaying_a_held_peak() {
        for reason in 0..3 {
            let mut response = MusicResponse::new();
            for frame in 0..180 {
                tick(&mut response, frame as f64 / 60.0, 60, 2.0);
            }
            assert!(response.uniforms()[0] > 0.1);
            let quiet = AudioFeatures::default();
            let held = pulse(0.0, 2.0);
            let epoch = if reason == 2 { 2 } else { 1 };
            let input = if reason == 0 { &quiet } else { &held };
            assert_eq!(
                response.update(input, &AudioMappings::default(), reason != 1, epoch, 0.0),
                [0.0; 4]
            );
            for _ in 0..90 {
                let out =
                    response.update(&held, &AudioMappings::default(), true, epoch, 1.0 / 60.0);
                assert_eq!(out[0], 0.0);
                assert_eq!(out[1], 0.0);
            }
        }
    }

    #[test]
    fn mapping_gates_clear_each_owned_channel_including_paused_passes() {
        let mut response = MusicResponse::new();
        for frame in 0..180 {
            tick(&mut response, frame as f64 / 60.0, 60, 2.0);
        }
        let muted = AudioMappings {
            bass_to_pulse: 0.0,
            mids_to_geometry: 0.0,
            treble_to_sparkle: 0.0,
            beat_to_flash: 0.0,
        };
        assert_eq!(response.update(&material(), &muted, true, 1, 0.0), [0.0; 4]);
        for gate in 0..3 {
            let mut response = MusicResponse::new();
            for frame in 0..180 {
                tick(&mut response, frame as f64 / 60.0, 60, 2.0);
            }
            let mut mappings = AudioMappings::default();
            match gate {
                0 => mappings.beat_to_flash = 0.0,
                1 => mappings.bass_to_pulse = 0.0,
                _ => mappings.treble_to_sparkle = 0.0,
            }
            let out = response.update(&material(), &mappings, true, 1, 0.0);
            match gate {
                0 => assert_eq!(&out[..2], &[0.0; 2]),
                1 => assert_eq!(out[2], 0.0),
                _ => assert_eq!(out[3], 0.0),
            }
        }
    }

    #[test]
    fn physical_envelopes_match_at_thirty_sixty_and_one_twenty_hz() {
        let outputs: Vec<_> = [30, 60, 120]
            .into_iter()
            .map(|hz| {
                let mut response = MusicResponse::new();
                let mut out = [0.0; 4];
                for frame in 0..=hz * 9 {
                    out = tick(&mut response, frame as f64 / hz as f64, hz, 2.0);
                }
                out
            })
            .collect();
        for out in &outputs[1..] {
            for channel in 0..4 {
                assert!(
                    (out[channel] - outputs[0][channel]).abs() < 0.035,
                    "{outputs:?}"
                );
            }
        }
    }

    #[test]
    fn duplicate_passes_invalid_input_and_large_gaps_do_not_create_events() {
        let mut response = MusicResponse::new();
        for frame in 0..180 {
            tick(&mut response, frame as f64 / 60.0, 60, 2.0);
        }
        let before = response.uniforms();
        assert_eq!(
            response.update(&material(), &AudioMappings::default(), true, 1, 0.0),
            before
        );
        assert_eq!(
            response.update(&material(), &AudioMappings::default(), true, 1, -1.0),
            before
        );
        assert_eq!(
            response.update(&material(), &AudioMappings::default(), true, 1, 5.0),
            [0.0; 4]
        );
        let invalid = AudioFeatures {
            bass: f32::NAN,
            mids: f32::INFINITY,
            treble: f32::NEG_INFINITY,
            energy: f32::NAN,
            beat: 1.0,
            transient: 1.0,
            ..AudioFeatures::default()
        };
        for dt in [0.1, f32::NAN, f32::INFINITY] {
            assert_eq!(
                response.update(&invalid, &AudioMappings::default(), true, 1, dt),
                [0.0; 4]
            );
        }
    }
}
