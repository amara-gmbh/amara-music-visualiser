//! Conservative local voice-like lead evidence, not vocal recognition.
//!
//! Harmonic stacks and a midrange spectral envelope can also belong to strings,
//! reeds, guitars and synthesizers. Absence of evidence does not prove that a
//! recording is instrumental. No model, transcription or source separation runs.
//! Feature references: Essentia's PitchSalienceFunction and Flatness descriptions
//! (https://essentia.upf.edu/reference/std_PitchSalienceFunction.html and
//! https://essentia.upf.edu/reference/std_Flatness.html). Thresholds below are
//! deliberately simple application heuristics, not those algorithms or scores.

pub const VOICE_METHOD: &str = "heuristic; instruments can resemble voices";

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct VoiceActivity {
    /// Smoothed voice-like evidence control, NOT a probability or accuracy.
    pub presence: f32,
    /// Confirmed absent -> present entries within this source. Silence does not
    /// rewind this counter; a new source/Analyzer starts again at zero.
    pub entry: u64,
}

#[derive(Default)]
pub(crate) struct VoiceDetector {
    activity: VoiceActivity,
    evidence_seconds: f32,
    absent_seconds: f32,
    silent_seconds: f32,
    pitch: Option<f32>,
    active: bool,
}

#[derive(Default)]
struct Evidence {
    strength: f32,
    pitch: f32,
}

impl VoiceDetector {
    /// Stale/invalid capture clears evidence immediately without manufacturing
    /// another entry when a renderer observes the cleared state.
    pub(crate) fn clear(&mut self) -> VoiceActivity {
        let entry = self.activity.entry;
        *self = Self::default();
        self.activity.entry = entry;
        self.activity
    }

    /// Called only by the DSP worker, with its already completed FFT. The work
    /// is bounded by 4096 spectrum bins and 128 * 16 short harmonic probes.
    pub(crate) fn update(
        &mut self,
        magnitudes: &[f32],
        sample_rate: u32,
        rms: f32,
        dt: f32,
    ) -> VoiceActivity {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.25)
        } else {
            0.0
        };
        if dt <= 0.0 {
            return self.activity;
        }
        if !rms.is_finite() || sample_rate == 0 || !(32..=4096).contains(&magnitudes.len()) {
            return self.clear();
        }
        if rms < 0.003 {
            self.silent_seconds += dt;
            if self.silent_seconds >= 0.20 {
                return self.clear();
            }
        } else {
            self.silent_seconds = 0.0;
        }
        let measured = spectral_evidence(magnitudes, sample_rate, rms);
        // A steady harmonic lead may bend, change note, or have an octave
        // ambiguity. Rapid unrelated pitch candidates must not build evidence.
        let consistent = self.pitch.is_none_or(|previous| {
            let ratio =
                (measured.pitch / previous.max(1.0)).max(previous / measured.pitch.max(1.0));
            ratio < 1.6 || (ratio - 2.0).abs() < 0.18
        });
        let supported = measured.strength >= 0.50 && consistent;
        if supported {
            self.evidence_seconds = (self.evidence_seconds + dt).min(1.0);
            self.absent_seconds = 0.0;
            self.pitch = Some(measured.pitch);
        } else {
            self.evidence_seconds = (self.evidence_seconds - dt * 2.0).max(0.0);
            self.absent_seconds += dt;
            if self.absent_seconds > 0.18 {
                self.pitch = None;
            }
        }
        if !self.active && self.evidence_seconds >= 0.35 {
            self.active = true;
            self.activity.entry = self.activity.entry.saturating_add(1);
        } else if self.active && self.absent_seconds >= 0.45 {
            self.active = false;
            self.evidence_seconds = 0.0;
        }
        let target = if self.active {
            if supported {
                0.65 + measured.strength * 0.35
            } else {
                0.45
            }
        } else {
            0.0
        };
        let tau = if target > self.activity.presence {
            0.12
        } else {
            0.25
        };
        self.activity.presence += (target - self.activity.presence) * (1.0 - (-dt / tau).exp());
        self.activity.presence = self.activity.presence.clamp(0.0, 1.0);
        self.activity
    }
}

fn power(magnitude: f32) -> f32 {
    if magnitude.is_finite() {
        magnitude.clamp(0.0, 8.0).powi(2)
    } else {
        0.0
    }
}

fn spectral_evidence(magnitudes: &[f32], sample_rate: u32, rms: f32) -> Evidence {
    if rms < 0.006 {
        return Evidence::default();
    }
    let bin_hz = sample_rate as f32 / (magnitudes.len() * 2) as f32;
    let mut total = 0.0;
    let mut body = 0.0;
    let mut upper = 0.0;
    let mut log_body = 0.0;
    let mut bins = 0_u32;
    let mut peak = 0.0_f32;
    for (index, magnitude) in magnitudes.iter().enumerate() {
        let frequency = index as f32 * bin_hz;
        let value = power(*magnitude);
        if (80.0..=8000.0).contains(&frequency) {
            total += value;
        }
        if (300.0..=3600.0).contains(&frequency) {
            body += value;
            log_body += value.max(1.0e-12).ln();
            bins += 1;
            peak = peak.max(value);
        }
        if (900.0..=3400.0).contains(&frequency) {
            upper += value;
        }
    }
    if bins == 0 || body < 1.0e-9 || total < 1.0e-9 {
        return Evidence::default();
    }
    let body_fraction = body / total;
    let upper_fraction = upper / total;
    let mean = body / bins as f32;
    let flatness = (log_body / bins as f32).exp() / mean;
    // Reject bass-only energy, a lone low tone, and broadly noise-like spectra.
    // Rich sustained instruments may still pass: this is explicitly voice-like.
    if body_fraction < 0.35 || upper_fraction < 0.10 || flatness > 0.32 {
        return Evidence::default();
    }
    let low = 85.0_f32.max(bin_hz * 3.0);
    let high = 555.0_f32.min(bin_hz * (magnitudes.len() - 2) as f32);
    if high <= low {
        return Evidence::default();
    }
    let candidates = (((high - low) / (bin_hz * 0.25)).ceil() as usize).clamp(2, 128);
    let mut best = Evidence::default();
    for candidate in 0..candidates {
        let fundamental = low + (high - low) * candidate as f32 / (candidates - 1) as f32;
        let mut harmonic_power = 0.0;
        let mut harmonics = 0;
        let mut upper_harmonics = 0;
        for harmonic in 1..=16 {
            let frequency = fundamental * harmonic as f32;
            if !(300.0..=3600.0).contains(&frequency) {
                continue;
            }
            let centre = (frequency / bin_hz).round() as usize;
            if centre == 0 || centre + 1 >= magnitudes.len() {
                continue;
            }
            let strongest = magnitudes[centre - 1..=centre + 1]
                .iter()
                .copied()
                .map(power)
                .fold(0.0_f32, f32::max);
            if strongest >= peak * 0.045 && strongest >= mean * 4.5 {
                harmonic_power += strongest;
                harmonics += 1;
                upper_harmonics += usize::from(frequency >= 900.0);
            }
        }
        let ratio = harmonic_power / body;
        if harmonics < 4 || upper_harmonics < 2 || ratio < 0.42 {
            continue;
        }
        let strength = ((ratio - 0.30) / 0.35).clamp(0.0, 1.0) * 0.50
            + ((body_fraction - 0.25) / 0.40).clamp(0.0, 1.0) * 0.30
            + ((upper_fraction - 0.07) / 0.23).clamp(0.0, 1.0) * 0.20;
        if strength > best.strength {
            best = Evidence {
                strength,
                pitch: fundamental,
            };
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustfft::{FftPlanner, num_complex::Complex32};
    use std::f32::consts::TAU;

    const SIZE: usize = 2048;
    const RATE: u32 = 48000;
    const DT: f32 = 0.016;

    fn spectrum(kind: usize, tick: usize) -> Vec<f32> {
        let mut samples: Vec<_> = (0..SIZE)
            .map(|index| {
                let time = (tick * 768 + index) as f32 / RATE as f32;
                let signal = match kind {
                    0 => {
                        // Synthetic voiced excitation with two vowel-like spectral
                        // lobes and slow pitch modulation, not a recorded singer.
                        let fundamental = 180.0 * (1.0 + (time * TAU * 5.0).sin() * 0.004);
                        let phase = time * 180.0 * TAU - 0.72 / 5.0 * (time * TAU * 5.0).cos();
                        (1..=18)
                            .map(|harmonic| {
                                let hz = fundamental * harmonic as f32;
                                let envelope = (-((hz - 700.0) / 180.0).powi(2)).exp()
                                    + (-((hz - 1450.0) / 250.0).powi(2)).exp() * 0.8
                                    + (-((hz - 2500.0) / 320.0).powi(2)).exp() * 0.35;
                                (phase * harmonic as f32).sin() * envelope * 0.035
                            })
                            .sum::<f32>()
                    }
                    1 => (time * 220.0 * TAU).sin() * 0.12,
                    2 => {
                        let n = ((tick * 768 + index) as u64)
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        let n = n ^ (n >> 17) ^ n.rotate_left(23);
                        ((n >> 40) as f32 / (1_u32 << 24) as f32 * 2.0 - 1.0) * 0.12
                    }
                    3 => {
                        let onset = time % 0.5;
                        (onset * 55.0 * TAU).sin() * (-onset * 28.0).exp() * 0.4
                    }
                    _ => 0.0,
                };
                Complex32::new(
                    signal * (0.5 - 0.5 * (TAU * index as f32 / (SIZE - 1) as f32).cos()),
                    0.0,
                )
            })
            .collect();
        FftPlanner::new()
            .plan_fft_forward(SIZE)
            .process(&mut samples);
        samples[..SIZE / 2]
            .iter()
            .map(|value| value.norm() * 2.0 / SIZE as f32)
            .collect()
    }

    #[test]
    fn sustained_voice_like_harmonics_confirm_one_entry_not_each_frame() {
        let mut detector = VoiceDetector::default();
        for tick in 0..18 {
            assert_eq!(detector.update(&spectrum(0, tick), RATE, 0.08, DT).entry, 0);
        }
        let mut result = VoiceActivity::default();
        for tick in 18..100 {
            let bins = spectrum(0, tick);
            result = detector.update(&bins, RATE, 0.08, DT);
        }
        assert_eq!(result.entry, 1);
        assert!(result.presence > 0.65, "{result:?}");
    }

    #[test]
    fn pure_tone_noise_kicks_and_silence_do_not_confirm_entries() {
        for kind in 1..=4 {
            let mut detector = VoiceDetector::default();
            for tick in 0..100 {
                let result = detector.update(
                    &spectrum(kind, tick),
                    RATE,
                    if kind == 4 { 0.0 } else { 0.08 },
                    DT,
                );
                assert_eq!(result, VoiceActivity::default(), "kind {kind}, tick {tick}");
            }
        }
    }

    #[test]
    fn silence_and_stale_clear_evidence_but_entry_counter_stays_monotonic() {
        let mut detector = VoiceDetector::default();
        for tick in 0..90 {
            detector.update(&spectrum(0, tick), RATE, 0.08, DT);
        }
        assert_eq!(detector.activity.entry, 1);
        for tick in 0..35 {
            detector.update(&spectrum(4, tick), RATE, 0.0, DT);
        }
        assert_eq!(
            detector.activity,
            VoiceActivity {
                presence: 0.0,
                entry: 1
            }
        );
        for tick in 0..90 {
            detector.update(&spectrum(0, tick), RATE, 0.08, DT);
        }
        assert_eq!(detector.activity.entry, 2);
        assert_eq!(
            detector.clear(),
            VoiceActivity {
                presence: 0.0,
                entry: 2
            }
        );
        let new_source = VoiceDetector::default();
        assert_eq!(new_source.activity, VoiceActivity::default());
    }

    #[test]
    fn short_bursts_never_accumulate_into_a_sustained_entry() {
        let mut detector = VoiceDetector::default();
        for tick in 0..150 {
            let voiced = tick % 35 < 10;
            detector.update(
                &spectrum(if voiced { 0 } else { 4 }, tick),
                RATE,
                if voiced { 0.08 } else { 0.0 },
                DT,
            );
        }
        assert_eq!(detector.activity, VoiceActivity::default());
    }

    #[test]
    fn paused_or_invalid_input_is_finite_and_cannot_invent_an_entry() {
        let mut detector = VoiceDetector::default();
        let voiced = spectrum(0, 0);
        for dt in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert_eq!(
                detector.update(&voiced, RATE, 0.08, dt),
                VoiceActivity::default()
            );
        }
        for (bins, rate, rms) in [
            (&[][..], RATE, 0.1),
            (&voiced[..], 0, 0.1),
            (&voiced[..], RATE, f32::NAN),
        ] {
            assert_eq!(
                detector.update(bins, rate, rms, DT),
                VoiceActivity::default()
            );
        }
        let broken = vec![f32::NAN; SIZE / 2];
        for _ in 0..40 {
            detector.update(&broken, RATE, 0.08, DT);
        }
        assert_eq!(detector.activity, VoiceActivity::default());
    }
}
