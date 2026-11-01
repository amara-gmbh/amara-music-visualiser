//! An original, short musical passage played through the real default output.
//! This module never writes to the capture/Demo buffer and saves no audio file.
use anyhow::{Context, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::{
    f64::consts::TAU,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const TRACK_SECONDS: f64 = 15.0;
const START_TIMEOUT: Duration = Duration::from_millis(750);
const STOP_TIMEOUT: Duration = Duration::from_millis(200);
// An OS output initializer cannot always be interrupted. Never stack another
// pending initializer behind one which has outlived its controller.
static OUTPUT_STARTING: AtomicBool = AtomicBool::new(false);

#[derive(Default)]
struct PlaybackState {
    cancelled: AtomicBool,
    playing: AtomicBool,
    finished: AtomicBool,
}

pub struct TestTrack {
    worker: Option<(Arc<PlaybackState>, JoinHandle<()>)>,
}

impl TestTrack {
    pub fn new() -> Self {
        Self { worker: None }
    }

    /// Starts actual speaker/headphone playback, not synthetic capture input.
    /// A stalled output initializer is cancelled rather than playing later.
    pub fn play(&mut self) -> anyhow::Result<()> {
        if self
            .worker
            .as_ref()
            .is_some_and(|(_, worker)| !worker.is_finished())
        {
            bail!("The short test passage is already playing or starting");
        }
        self.stop();
        if OUTPUT_STARTING
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            bail!("Audio output startup is still pending; wait a moment before retrying");
        }
        let state = Arc::new(PlaybackState::default());
        let target = state.clone();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let worker = match thread::Builder::new()
            .name("amara-test-playback".into())
            .spawn(move || run_output(target, ready_tx))
        {
            Ok(worker) => worker,
            Err(error) => {
                OUTPUT_STARTING.store(false, Ordering::Release);
                return Err(error).context("Could not start the test playback worker");
            }
        };
        self.worker = Some((state, worker));
        match ready_rx.recv_timeout(START_TIMEOUT) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                self.stop();
                bail!("Could not play the test passage: {error}");
            }
            Err(error) => {
                self.stop();
                bail!("Audio output did not start in time; playback cancelled ({error})");
            }
        }
    }

    pub fn is_playing(&self) -> bool {
        self.worker.as_ref().is_some_and(|(state, _)| {
            state.playing.load(Ordering::Acquire)
                && !state.cancelled.load(Ordering::Acquire)
                && !state.finished.load(Ordering::Acquire)
        })
    }

    pub fn stop(&mut self) {
        if let Some((state, worker)) = self.worker.take() {
            state.cancelled.store(true, Ordering::Release);
            state.playing.store(false, Ordering::Release);
            let started = Instant::now();
            while !worker.is_finished() && started.elapsed() < STOP_TIMEOUT {
                thread::sleep(Duration::from_millis(5));
            }
            if worker.is_finished() {
                let _ = worker.join();
            }
            // A blocked OS initializer is detached. Its cancellation flag makes
            // eventual startup inert; OUTPUT_STARTING stays latched meanwhile.
        }
    }
}

impl Drop for TestTrack {
    fn drop(&mut self) {
        self.stop();
    }
}

struct StartupGuard;
impl Drop for StartupGuard {
    fn drop(&mut self) {
        OUTPUT_STARTING.store(false, Ordering::Release);
    }
}

struct PlaybackGuard(Arc<PlaybackState>);
impl Drop for PlaybackGuard {
    fn drop(&mut self) {
        self.0.playing.store(false, Ordering::Release);
        self.0.finished.store(true, Ordering::Release);
    }
}

fn run_output(state: Arc<PlaybackState>, ready: mpsc::SyncSender<Result<(), String>>) {
    let startup = StartupGuard;
    let _playback = PlaybackGuard(state.clone());
    let result = (|| -> anyhow::Result<cpal::Stream> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("No default speaker or headphone output is available")?;
        let supported = device
            .default_output_config()
            .context("Could not read the default output format")?;
        let config = supported.config();
        anyhow::ensure!(
            config.sample_rate > 0 && config.channels > 0,
            "The default output has an invalid sample rate or channel count"
        );
        anyhow::ensure!(
            !state.cancelled.load(Ordering::Acquire),
            "Playback was cancelled during output startup"
        );
        let stream = match supported.sample_format() {
            cpal::SampleFormat::F32 => build_stream::<f32>(&device, config, state.clone()),
            cpal::SampleFormat::I16 => build_stream::<i16>(&device, config, state.clone()),
            cpal::SampleFormat::U16 => build_stream::<u16>(&device, config, state.clone()),
            format => bail!("The default output uses unsupported sample format {format:?}"),
        }?;
        anyhow::ensure!(
            !state.cancelled.load(Ordering::Acquire),
            "Playback was cancelled during stream creation"
        );
        stream
            .play()
            .context("Could not start the default output")?;
        Ok(stream)
    })();
    let stream = match result {
        Ok(stream) if !state.cancelled.load(Ordering::Acquire) => stream,
        Ok(_) => {
            let _ = ready.send(Err("Playback was cancelled".into()));
            return;
        }
        Err(error) => {
            let _ = ready.send(Err(format!("{error:#}")));
            return;
        }
    };
    state.playing.store(true, Ordering::Release);
    drop(startup);
    if ready.send(Ok(())).is_err() {
        state.cancelled.store(true, Ordering::Release);
    }
    let started = Instant::now();
    while !state.cancelled.load(Ordering::Acquire)
        && !state.finished.load(Ordering::Acquire)
        && started.elapsed() < Duration::from_secs_f64(TRACK_SECONDS + 2.0)
    {
        thread::sleep(Duration::from_millis(10));
    }
    drop(stream); // The worker owns, stops and drops the native output stream.
}

fn build_stream<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    state: Arc<PlaybackState>,
) -> anyhow::Result<cpal::Stream>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let channels = usize::from(config.channels);
    let mut synth = Synth::new(config.sample_rate);
    let errors = state.clone();
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _| {
                for frame in output.chunks_mut(channels) {
                    let sample = if state.cancelled.load(Ordering::Relaxed) {
                        0.0
                    } else {
                        synth.next()
                    };
                    // Identical phase in every output channel cannot cancel in
                    // the capture helper's stereo-to-mono downmix.
                    for channel in frame {
                        *channel = T::from_sample(sample);
                    }
                }
                if synth.finished() {
                    state.finished.store(true, Ordering::Release);
                }
            },
            move |error| {
                log::warn!("Test playback output stopped: {error}");
                errors.cancelled.store(true, Ordering::Release);
                errors.playing.store(false, Ordering::Release);
            },
            None,
        )
        .context("Could not create the default output stream")
}

struct Synth {
    frame: u64,
    sample_rate: u32,
}

impl Synth {
    fn new(sample_rate: u32) -> Self {
        Self {
            frame: 0,
            sample_rate,
        }
    }

    fn finished(&self) -> bool {
        self.frame as f64 >= TRACK_SECONDS * f64::from(self.sample_rate)
    }

    fn next(&mut self) -> f32 {
        let sample = passage_sample(self.frame as f64 / f64::from(self.sample_rate));
        self.frame += 1;
        sample
    }
}

fn note_frequency(note: u8) -> f64 {
    440.0 * 2.0_f64.powf((f64::from(note) - 69.0) / 12.0)
}

fn fade(time: f64) -> f64 {
    if !(0.0..TRACK_SECONDS).contains(&time) {
        return 0.0;
    }
    let attack = (time / 0.25).clamp(0.0, 1.0);
    let release = ((TRACK_SECONDS - time) / 0.85).clamp(0.0, 1.0);
    // Smooth cosine edges; maximum combined signal stays well below clipping.
    (0.5 - 0.5 * (std::f64::consts::PI * attack).cos())
        * (0.5 - 0.5 * (std::f64::consts::PI * release).cos())
}

fn passage_sample(time: f64) -> f32 {
    let envelope = fade(time);
    if envelope == 0.0 {
        return 0.0;
    }
    // Six gentle four-beat bars at 96 BPM: Cmaj9, Am7, Fmaj7, G6,
    // Dm7, G6. This melody/passage is generated here, not a recorded song.
    const CHORDS: [[u8; 4]; 6] = [
        [60, 64, 67, 74],
        [57, 60, 64, 67],
        [53, 57, 60, 64],
        [55, 59, 62, 64],
        [50, 57, 60, 65],
        [55, 59, 62, 64],
    ];
    const BEAT: f64 = 60.0 / 96.0;
    const BAR: f64 = BEAT * 4.0;
    let bar = (time / BAR).floor() as usize;
    let chord = CHORDS[bar.min(CHORDS.len() - 1)];
    let bar_time = time % BAR;
    let crossfade = (bar_time / 0.18).clamp(0.0, 1.0);
    let previous = CHORDS[bar.saturating_sub(1)];
    let pad = |notes: [u8; 4]| -> f64 {
        notes
            .into_iter()
            .map(|note| {
                let phase = TAU * note_frequency(note) * time;
                phase.sin() + 0.17 * (phase * 2.0 + 0.4).sin()
            })
            .sum::<f64>()
            * 0.018
    };
    let warm_pad = pad(previous) * (1.0 - crossfade) + pad(chord) * crossfade;
    let step = (time / (BEAT * 0.5)).floor() as usize;
    let pluck_time = time % (BEAT * 0.5);
    let arp_note = chord[[0, 2, 1, 3, 2, 1, 3, 2][step % 8]] + 12;
    let arp_phase = TAU * note_frequency(arp_note) * pluck_time;
    let arp = (arp_phase.sin() + 0.22 * (2.0 * arp_phase).sin())
        * (pluck_time / 0.008).min(1.0)
        * (-pluck_time * 11.0).exp()
        * 0.055;
    let beat_time = time % BEAT;
    let bass_phase = TAU * note_frequency(chord[0] - 12) * time;
    let bass = bass_phase.sin() * (beat_time / 0.012).min(1.0) * (-beat_time * 2.8).exp() * 0.055;
    let kick_phase = TAU * (45.0 * beat_time + 50.0 * 0.035 * (1.0 - (-beat_time / 0.035).exp()));
    let kick = kick_phase.sin() * (beat_time / 0.002).min(1.0) * (-beat_time * 16.0).exp() * 0.10;
    ((warm_pad + arp + bass + kick) * envelope) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passage_is_finite_and_has_modest_headroom_at_common_rates() {
        for rate in [44_100, 48_000, 96_000] {
            for frame in (0..rate * 15).step_by(37) {
                let sample = passage_sample(f64::from(frame) / f64::from(rate));
                assert!(sample.is_finite());
                assert!(sample.abs() < 0.32, "sample={sample} rate={rate}");
            }
        }
    }

    #[test]
    fn passage_has_smooth_edges_and_finite_duration() {
        assert_eq!(passage_sample(-1.0), 0.0);
        assert_eq!(passage_sample(0.0), 0.0);
        assert_eq!(passage_sample(TRACK_SECONDS), 0.0);
        assert_eq!(passage_sample(TRACK_SECONDS + 1.0), 0.0);
        assert!(fade(0.001) < 0.0001);
        assert!(fade(TRACK_SECONDS - 0.001) < 0.0001);
        let mut synth = Synth::new(100);
        for _ in 0..1_500 {
            assert!(!synth.finished());
            synth.next();
        }
        assert!(synth.finished());
        assert_eq!(synth.next(), 0.0);
    }

    #[test]
    fn passage_contains_changing_musical_signal_not_a_constant_or_demo_buffer() {
        let rms = |start: f64| {
            let samples = (0..2_400).map(|frame| {
                f64::from(passage_sample(start + f64::from(frame) / 48_000.0)).powi(2)
            });
            (samples.sum::<f64>() / 2_400.0).sqrt()
        };
        assert!(rms(1.0) > 0.01);
        assert!(rms(6.0) > 0.01);
        assert!(rms(14.95) < rms(7.0) * 0.02);
        let variation: f32 = (0..1_000)
            .map(|frame| {
                let time = 0.7 + f64::from(frame) / 48_000.0;
                (passage_sample(time) - passage_sample(time + 5.0)).abs()
            })
            .sum();
        assert!(variation > 1.0);
    }

    #[test]
    fn new_track_is_stopped_and_stop_is_idempotent_without_device_access() {
        let mut track = TestTrack::new();
        assert!(!track.is_playing());
        track.stop();
        track.stop();
        assert!(!track.is_playing());
    }
}
