//! A fluid visual response to the analyser's explicitly heuristic voice events.
//! This module does not classify sound or manufacture events from midrange level.
use crate::{dsp::AudioFeatures, presets::AudioMappings};

const DURATION: f64 = 3.6;
const ATTACK: f64 = 0.46;
const INACTIVE: [f32; 4] = [0.0, 1.0, 0.0, 0.0];

pub struct VocalResponse {
    epoch: Option<u64>,
    last_entry: u64,
    elapsed: f64,
    strength: f32,
    presence: f64,
    phase: f64,
}

impl Default for VocalResponse {
    fn default() -> Self {
        Self {
            epoch: None,
            last_entry: 0,
            elapsed: DURATION,
            strength: 0.0,
            presence: 0.0,
            phase: 0.0,
        }
    }
}

impl VocalResponse {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// [smoothed presence, entry progress, entry envelope, continuous phase].
    /// Physical seconds only: Pace cannot speed up vocal evidence or its reveal.
    /// Stale/disconnected sources consume counters without replaying old entries.
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
            self.last_entry = audio.voice.entry;
        }
        let gain = if mappings.mids_to_geometry.is_finite() {
            mappings.mids_to_geometry.clamp(0.0, 1.5)
        } else {
            0.0
        };
        let energy = unit(audio.energy);
        if !fresh || gain == 0.0 || energy <= 0.008 || !dt.is_finite() || dt > 0.5 {
            self.elapsed = DURATION;
            self.strength = 0.0;
            self.presence = 0.0;
            self.last_entry = audio.voice.entry;
            return INACTIVE;
        }
        if dt > 0.0 {
            let dt = f64::from(dt);
            self.elapsed = (self.elapsed + dt).min(DURATION);
            self.phase = (self.phase + dt * 0.35).rem_euclid(std::f64::consts::TAU);
            // The level gate only prevents quiet/no-signal enhancement. Voice
            // evidence and entries come exclusively from the worker detector.
            let level_gate = smooth((energy - 0.008) / 0.032);
            // A gentle perceptual lift gives an already-confirmed lead enough
            // contour to read. This is an artistic control, not a probability.
            let target = unit(audio.voice.presence).sqrt() * level_gate;
            let tau = if f64::from(target) > self.presence {
                0.40
            } else {
                0.65
            };
            self.presence =
                f64::from(target) + (self.presence - f64::from(target)) * (-dt / tau).exp();
            if self.presence < 0.00001 && target == 0.0 {
                self.presence = 0.0;
            }
            if audio.voice.entry != self.last_entry {
                let new_entry = audio.voice.entry > self.last_entry;
                // Consume every observed counter exactly once. Do not restart
                // an unfinished reveal: that would jump a travelling ribbon.
                self.last_entry = audio.voice.entry;
                if new_entry && self.elapsed >= DURATION {
                    self.elapsed = 0.0;
                    // The event already represents sustained worker evidence.
                    // Its first published presence may still be at the start
                    // of an attack; do not lose or weaken that confirmed entry.
                    self.strength = (0.90 + unit(audio.voice.presence) * 0.10) * level_gate;
                }
            }
        }
        let progress = (self.elapsed / DURATION) as f32;
        let envelope =
            smooth((self.elapsed / ATTACK) as f32) * (1.0 - smooth((progress - 0.45) / 0.55));
        [
            self.presence as f32 * gain,
            progress,
            self.strength * envelope * gain,
            self.phase as f32,
        ]
    }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn smooth(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audio(entry: u64, presence: f32) -> AudioFeatures {
        let mut audio = AudioFeatures {
            energy: 0.5,
            ..Default::default()
        };
        audio.voice.entry = entry;
        audio.voice.presence = presence;
        audio
    }

    #[test]
    fn entry_is_consumed_once_and_a_second_entry_cannot_jump_an_active_reveal() {
        let mappings = AudioMappings::default();
        let mut response = VocalResponse::default();
        response.update(&audio(0, 0.0), &mappings, true, 1, 0.1);
        assert_eq!(
            response.update(&audio(1, 0.8), &mappings, true, 1, 0.1)[2],
            0.0
        );
        let first = response.update(&audio(1, 0.8), &mappings, true, 1, 0.1);
        assert!(first[2] > 0.0);
        assert_eq!(
            response.update(&audio(1, 0.8), &mappings, true, 1, 0.0),
            first
        );
        let second = response.update(&audio(2, 0.8), &mappings, true, 1, 0.1);
        assert!(second[1] > first[1]);
        for _ in 0..40 {
            response.update(&audio(2, 0.8), &mappings, true, 1, 0.1);
        }
        assert_eq!(
            response.update(&audio(2, 0.8), &mappings, true, 1, 0.1)[2],
            0.0
        );
    }

    #[test]
    fn pause_source_changes_silence_and_disconnection_clear_without_replay() {
        for condition in 0..4 {
            let mut response = VocalResponse::default();
            let mappings = AudioMappings::default();
            response.update(&audio(0, 0.0), &mappings, true, 1, 0.1);
            response.update(&audio(1, 0.8), &mappings, true, 1, 0.1);
            assert!(response.update(&audio(1, 0.8), &mappings, true, 1, 0.2)[2] > 0.0);
            let mut muted = mappings.clone();
            let mut input = audio(1, 0.8);
            let epoch = if condition == 1 { 2 } else { 1 };
            if condition == 2 {
                input.energy = 0.0;
            }
            if condition == 3 {
                muted.mids_to_geometry = 0.0;
            }
            let cleared = response.update(&input, &muted, condition != 0, epoch, 0.0);
            assert_eq!(cleared[0], 0.0);
            assert_eq!(cleared[2], 0.0);
            let resumed = response.update(&audio(1, 0.8), &mappings, true, epoch, 0.1);
            assert_eq!(resumed[2], 0.0, "must not replay a consumed entry");
        }
    }

    #[test]
    fn level_without_voice_events_never_makes_an_entry() {
        let mut response = VocalResponse::default();
        let mappings = AudioMappings::default();
        let mut input = audio(0, 0.0);
        input.mids = 1.0;
        input.beat = 1.0;
        input.transient = 1.0;
        for _ in 0..120 {
            let output = response.update(&input, &mappings, true, 1, 1.0 / 60.0);
            assert_eq!(output[0], 0.0);
            assert_eq!(output[2], 0.0);
        }
    }

    #[test]
    fn confirmed_entry_is_visible_even_when_its_first_presence_is_zero() {
        let mappings = AudioMappings {
            mids_to_geometry: 1.0,
            ..Default::default()
        };
        for presence in [0.0, 0.001, 0.1, 0.9] {
            let mut response = VocalResponse::default();
            response.update(&audio(0, 0.0), &mappings, true, 1, 0.0);
            let onset = response.update(&audio(1, presence), &mappings, true, 1, 0.01);
            assert_eq!(onset[2], 0.0, "the reveal starts without a flash");
            let early = response.update(&audio(1, presence), &mappings, true, 1, 0.23);
            assert!((0.44..=0.50).contains(&early[2]));
            let peak = response.update(&audio(1, presence), &mappings, true, 1, 0.23);
            assert!(
                peak[2] >= 0.89,
                "confirmed entry must not depend on first presence"
            );
            assert!(peak[2] <= 1.0);
            for _ in 0..27 {
                response.update(&audio(1, presence), &mappings, true, 1, 0.1);
            }
            let late = response.update(&audio(1, presence), &mappings, true, 1, 0.1);
            assert!(
                late[2] > 0.0,
                "reveal remains softly visible beyond three seconds"
            );
            for _ in 0..4 {
                response.update(&audio(1, presence), &mappings, true, 1, 0.1);
            }
            assert_eq!(
                response.update(&audio(1, presence), &mappings, true, 1, 0.0)[2],
                0.0
            );
        }
    }

    #[test]
    fn sustained_contours_open_gradually_and_close_without_a_step() {
        let mappings = AudioMappings {
            mids_to_geometry: 1.0,
            ..Default::default()
        };
        let mut response = VocalResponse::default();
        response.update(&audio(0, 0.0), &mappings, true, 1, 0.0);
        let first = response.update(&audio(1, 0.64), &mappings, true, 1, 1.0 / 60.0);
        assert!((0.0..0.04).contains(&first[0]));
        let mut held = first;
        for _ in 0..119 {
            let next = response.update(&audio(1, 0.64), &mappings, true, 1, 1.0 / 60.0);
            assert!(next[0] >= held[0]);
            assert!(next[0] - held[0] < 0.04);
            held = next;
        }
        assert!((0.79..=0.80).contains(&held[0]));
        let release = response.update(&audio(1, 0.0), &mappings, true, 1, 1.0 / 60.0);
        assert!(release[0] < held[0] && held[0] - release[0] < 0.025);
    }

    #[test]
    fn counter_reset_does_not_invent_a_new_entry() {
        let mappings = AudioMappings::default();
        let mut response = VocalResponse::default();
        response.update(&audio(4, 0.7), &mappings, true, 1, 0.1);
        let reset = response.update(&audio(0, 0.7), &mappings, true, 1, 0.1);
        assert_eq!(reset[1], 1.0);
        assert_eq!(reset[2], 0.0);
    }

    #[test]
    fn physical_response_matches_at_thirty_sixty_and_one_twenty_hz() {
        let mappings = AudioMappings::default();
        let mut outputs = Vec::new();
        for hz in [30, 60, 120] {
            let mut response = VocalResponse::default();
            response.update(&audio(0, 0.0), &mappings, true, 1, 0.0);
            response.update(&audio(1, 0.8), &mappings, true, 1, 0.001);
            let mut output = [0.0; 4];
            for _ in 0..hz {
                output = response.update(&audio(1, 0.8), &mappings, true, 1, 1.0 / hz as f32);
            }
            outputs.push(output);
        }
        for output in &outputs[1..] {
            for (a, b) in outputs[0].iter().zip(output) {
                assert!((a - b).abs() < 0.00001);
            }
        }
    }
}
