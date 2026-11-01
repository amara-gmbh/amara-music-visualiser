use crate::dsp::AudioFeatures;

/// Display ballistics only. The analyzer, beat detector and director keep their
/// original input; these are measured bars with easing, never a scripted EQ.
pub struct MeterDisplay {
    pub spectrum: [f32; 48],
    pub peaks: [f32; 48],
    peak_holds: [f32; 48],
    pub bass: f32,
    pub mids: f32,
    pub treble: f32,
    pub energy: f32,
    pub beat: f32,
}

impl MeterDisplay {
    pub fn new() -> Self {
        Self {
            spectrum: [0.0; 48],
            peaks: [0.0; 48],
            peak_holds: [0.0; 48],
            bass: 0.0,
            mids: 0.0,
            treble: 0.0,
            energy: 0.0,
            beat: 0.0,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn update(&mut self, input: &AudioFeatures, dt: f32) {
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.1)
        } else {
            0.0
        };
        if dt == 0.0 {
            return;
        }
        for (index, value) in self.spectrum.iter_mut().enumerate() {
            let sample = |at: usize| unit(input.spectrum.get(at).copied().unwrap_or(0.0));
            let target = sample(index) * 0.8
                + sample(index.saturating_sub(1)) * 0.1
                + sample((index + 1).min(47)) * 0.1;
            *value = follow(*value, target, dt, 0.13, 0.42);
            // Hold measured band maxima for 700 ms, then fall in elapsed time,
            // not pixels per frame. No effect on FFT, beats or rendering input.
            let peak = &mut self.peaks[index];
            let hold = &mut self.peak_holds[index];
            if target >= *peak {
                *peak = target;
                *hold = 0.7;
            } else {
                let falling_dt = (dt - *hold).max(0.0);
                *hold = (*hold - dt).max(0.0);
                *peak = (*peak - falling_dt * 0.28).max(*value);
            }
        }
        self.bass = follow(self.bass, input.bass, dt, 0.12, 0.38);
        self.mids = follow(self.mids, input.mids, dt, 0.12, 0.38);
        self.treble = follow(self.treble, input.treble, dt, 0.12, 0.38);
        self.energy = follow(self.energy, input.energy, dt, 0.10, 0.42);
        self.beat = follow(self.beat, input.beat, dt, 0.035, 0.20);
    }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn follow(value: f32, target: f32, dt: f32, attack: f32, release: f32) -> f32 {
    let target = unit(target);
    let tau = if target > value { attack } else { release };
    value + (target - value) * (1.0 - (-dt / tau).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bars_ease_into_signal_and_decay_without_touching_analysis() {
        let input = AudioFeatures {
            bass: 1.0,
            spectrum: vec![1.0; 48],
            ..Default::default()
        };
        let mut display = MeterDisplay::new();
        display.update(&input, 1.0 / 60.0);
        assert!(display.spectrum[20] > 0.0 && display.spectrum[20] < 0.15);
        assert_eq!(input.spectrum[20], 1.0);
        let before = display.spectrum[20];
        display.update(&AudioFeatures::default(), 1.0 / 60.0);
        assert!(display.spectrum[20] < before && display.spectrum[20] > before * 0.95);
        display.reset();
        assert_eq!(display.spectrum, [0.0; 48]);
    }

    #[test]
    fn display_converges_at_different_refresh_rates() {
        let input = AudioFeatures {
            spectrum: vec![0.8; 48],
            ..Default::default()
        };
        let mut results = Vec::new();
        for hz in [30, 60, 120] {
            let mut display = MeterDisplay::new();
            for _ in 0..hz {
                display.update(&input, 1.0 / hz as f32);
            }
            results.push(display.spectrum[24]);
        }
        assert!((results[0] - results[1]).abs() < 0.00001);
        assert!((results[1] - results[2]).abs() < 0.00001);
    }

    #[test]
    fn invalid_input_and_duplicate_passes_cannot_move_or_poison_bars() {
        let mut display = MeterDisplay::new();
        let input = AudioFeatures {
            spectrum: vec![1.0; 48],
            ..Default::default()
        };
        display.update(&input, 0.016);
        let before = display.spectrum;
        display.update(&input, 0.0);
        assert_eq!(before, display.spectrum);
        let bad = AudioFeatures {
            bass: f32::NAN,
            spectrum: vec![f32::INFINITY; 48],
            ..Default::default()
        };
        for dt in [f32::NAN, -1.0, 0.016] {
            display.update(&bad, dt);
        }
        assert!(
            display
                .spectrum
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
        );
    }

    #[test]
    fn peaks_hold_then_fall_gently_and_reset_with_the_source() {
        let mut display = MeterDisplay::new();
        display.update(
            &AudioFeatures {
                spectrum: vec![0.8; 48],
                ..Default::default()
            },
            0.05,
        );
        assert!((display.peaks[24] - 0.8).abs() < 0.00001);
        let quiet = AudioFeatures::default();
        for _ in 0..6 {
            display.update(&quiet, 0.1);
        }
        assert!((display.peaks[24] - 0.8).abs() < 0.00001);
        for _ in 0..5 {
            display.update(&quiet, 0.1);
        }
        assert!((display.peaks[24] - (0.8 - 0.4 * 0.28)).abs() < 0.00001);
        assert!(
            display
                .peaks
                .iter()
                .zip(display.spectrum)
                .all(|(peak, bar)| *peak >= bar)
        );
        let before = (display.peaks, display.peak_holds);
        display.update(&quiet, 0.0);
        assert_eq!(before, (display.peaks, display.peak_holds));
        display.reset();
        assert_eq!(display.peaks, [0.0; 48]);
        assert_eq!(display.peak_holds, [0.0; 48]);
    }

    #[test]
    fn peak_fall_is_consistent_across_refresh_rates() {
        let mut results = Vec::new();
        for hz in [30, 60, 120] {
            let mut display = MeterDisplay::new();
            display.update(
                &AudioFeatures {
                    spectrum: vec![0.9; 48],
                    ..Default::default()
                },
                0.01,
            );
            for _ in 0..hz * 2 {
                display.update(&AudioFeatures::default(), 1.0 / hz as f32);
            }
            results.push(display.peaks[20]);
        }
        assert!((results[0] - results[1]).abs() < 0.00001);
        assert!((results[1] - results[2]).abs() < 0.00001);
    }
}
