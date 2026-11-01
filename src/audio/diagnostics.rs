//! Bounded system-input check. Only aggregate levels/DSP values are printed;
//! captured samples remain in memory and no recording is saved.
use super::{AudioController, AudioMode, CapturePhase};
use crate::dsp::{Analyzer, FFT_SIZE};
use std::{
    thread,
    time::{Duration, Instant},
};

const MEANINGFUL_RMS: f32 = 0.000_1;

pub fn verify_system_audio(duration: Duration) -> anyhow::Result<()> {
    anyhow::ensure!(
        (Duration::from_secs(1)..=Duration::from_secs(30)).contains(&duration),
        "audio verification duration must be 1 through 30 seconds"
    );
    let controller = AudioController::with_mode(AudioMode::System);
    let shared = controller.shared();
    let mut analyzer = Analyzer::new();
    let started = Instant::now();
    let mut sequence = u64::MAX;
    let mut sample_time = 0.0_f64;
    let mut windows = 0_u64;
    let mut maximum_rms = 0.0_f32;
    let mut maximum_peak = 0.0_f32;
    let mut maximum_bands = [0.0_f32; 3];
    while started.elapsed() < duration {
        let snapshot = shared.snapshot(FFT_SIZE);
        if snapshot.source.is_fresh()
            && snapshot.source.sequence != sequence
            && snapshot.samples.len() >= FFT_SIZE
        {
            sequence = snapshot.source.sequence;
            let next_time =
                snapshot.source.total_frames as f64 / f64::from(snapshot.source.sample_rate.max(1));
            let features = analyzer.analyze_at(
                &snapshot.samples,
                snapshot.source.sample_rate,
                (next_time - sample_time).clamp(0.001, 0.25) as f32,
                next_time,
            );
            sample_time = next_time;
            windows += 1;
            let square_sum: f64 = snapshot
                .samples
                .iter()
                .map(|sample| f64::from(*sample).powi(2))
                .sum();
            maximum_rms =
                maximum_rms.max((square_sum / snapshot.samples.len() as f64).sqrt() as f32);
            maximum_peak = snapshot
                .samples
                .iter()
                .fold(maximum_peak, |peak, sample| peak.max(sample.abs()));
            for (maximum, value) in
                maximum_bands
                    .iter_mut()
                    .zip([features.bass, features.mids, features.treble])
            {
                *maximum = maximum.max(value);
            }
        }
        if !shared.source_state().accepting {
            break;
        }
        thread::sleep(Duration::from_millis(20).min(duration.saturating_sub(started.elapsed())));
    }
    let health = controller.health();
    let signal_received = windows > 0
        && maximum_rms >= MEANINGFUL_RMS
        && maximum_bands.iter().any(|value| *value > 0.01);
    let passed = signal_received && health.fresh;
    let result = if passed {
        "non_silent_system_audio_received"
    } else if health.phase == CapturePhase::PermissionDenied {
        "permission_denied"
    } else if matches!(
        health.phase,
        CapturePhase::TransportError | CapturePhase::Unavailable
    ) {
        "capture_failed"
    } else if health.frames_received > 0 && !signal_received {
        "pcm_received_but_silent"
    } else if signal_received {
        "signal_received_but_stream_not_fresh_at_end"
    } else {
        "no_pcm_received"
    };
    let summary = serde_json::json!({
        "result": result,
        "passed": passed,
        "requested_seconds": duration.as_secs_f64(),
        "observed_seconds": started.elapsed().as_secs_f64(),
        "source": "mac_system_audio",
        "backend": health.backend,
        "capture_state": health.phase,
        "status": health.message,
        "pcm_frames_received": health.frames_received,
        "sample_rate": health.sample_rate,
        "fresh_at_end": health.fresh,
        "dsp_windows_analyzed": windows,
        "maximum_window_rms": maximum_rms,
        "maximum_sample_peak": maximum_peak,
        "maximum_band_levels": {"bass": maximum_bands[0], "mids": maximum_bands[1], "treble": maximum_bands[2]},
        "signal_rms_threshold": MEANINGFUL_RMS,
        "privacy": "Aggregate in-memory measurements only. No audio recording, track metadata or file is saved.",
        "interpretation": "Non-silent PCM proves capture and DSP transport, not song/genre recognition. Silence does not prove permission denial."
    });
    // Cancel and reap the helper before printing the final diagnostic result.
    drop(controller);
    println!("{}", serde_json::to_string_pretty(&summary)?);
    anyhow::ensure!(
        passed,
        "No fresh non-silent Mac system audio was verified; see the capture summary above"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_durations_are_rejected_before_capture_starts() {
        assert!(verify_system_audio(Duration::ZERO).is_err());
        assert!(verify_system_audio(Duration::from_secs(31)).is_err());
    }
}
