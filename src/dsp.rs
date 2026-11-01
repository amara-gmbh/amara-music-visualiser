use crate::audio::SharedAudio;
use crate::voice::{VoiceActivity, VoiceDetector};
use rustfft::{Fft, FftPlanner, num_complex::Complex32};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub const FFT_SIZE: usize = 2048;
const ANALYSIS_INTERVAL: Duration = Duration::from_millis(16);
pub const HEURISTIC_METHOD: &str = "Signal heuristics · not ML";

#[derive(Debug, Clone)]
pub struct AudioFeatures {
    pub bass: f32,
    pub mids: f32,
    pub treble: f32,
    pub energy: f32,
    pub beat: f32,
    pub transient: f32,
    pub bpm: Option<f32>,
    pub spectrum: Vec<f32>,
    pub voice: VoiceActivity,
    pub dna: MusicDna,
}

impl Default for AudioFeatures {
    fn default() -> Self {
        Self {
            bass: 0.0,
            mids: 0.0,
            treble: 0.0,
            energy: 0.0,
            beat: 0.0,
            transient: 0.0,
            bpm: None,
            spectrum: vec![0.0; 48],
            voice: VoiceActivity::default(),
            dna: MusicDna::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MusicDna {
    pub genre_hint: String,
    pub mood: String,
    pub instrument_hints: String,
    pub brightness: f32,
    pub danceability: f32,
    pub inference_method: &'static str,
}

impl Default for MusicDna {
    fn default() -> Self {
        Self {
            genre_hint: "waiting for audio".into(),
            mood: "no fresh signal".into(),
            instrument_hints: "No reliable instrument evidence".into(),
            brightness: 0.0,
            danceability: 0.0,
            inference_method: HEURISTIC_METHOD,
        }
    }
}

// A future local model implements this small boundary and runs on this worker.
// It must report its method honestly; a signal-derived score is not accuracy.
pub trait MusicInference: Send {
    fn infer(&mut self, features: &AudioFeatures) -> MusicDna;
}

struct HeuristicInference;

impl MusicInference for HeuristicInference {
    fn infer(&mut self, features: &AudioFeatures) -> MusicDna {
        infer_music_dna(features)
    }
}

const DNA_EVIDENCE_SECONDS: f64 = 3.0;
const DNA_EVALUATION_SECONDS: f64 = 0.5;
const DNA_SILENCE_CLEAR_SECONDS: f64 = 0.75;

#[derive(Default)]
struct StableDescription {
    current: Option<String>,
    candidate: Option<(String, f64)>,
    changed_at: Option<f64>,
}

impl StableDescription {
    fn update(&mut self, value: String, time: f64, persistence: f64, dwell: f64) {
        if self.current.as_ref() == Some(&value) {
            self.candidate = None;
            return;
        }
        if self
            .candidate
            .as_ref()
            .is_none_or(|(label, _)| label != &value)
        {
            self.candidate = Some((value, time));
        }
        let candidate_ready = self
            .candidate
            .as_ref()
            .is_some_and(|(_, since)| time - since + 1e-6 >= persistence);
        let dwell_complete = self
            .changed_at
            .is_none_or(|changed| time - changed + 1e-6 >= dwell);
        if candidate_ready && dwell_complete {
            self.current = self.candidate.take().map(|(label, _)| label);
            self.changed_at = Some(time);
        }
    }
}

/// Descriptions summarize a musical passage, not each FFT frame. This layer is
/// deliberately separate from the fast bands, onsets and beat envelopes below.
/// All timing uses received-PCM seconds, never render rate or frame counts.
#[derive(Default)]
struct DnaStabilizer {
    averaged: AudioFeatures,
    evidence_started: bool,
    last_time: Option<f64>,
    sample_rate: Option<u32>,
    evaluation_tick: Option<u64>,
    silence_since: Option<f64>,
    tempo_support: f32,
    tempo: f32,
    genre: StableDescription,
    mood: StableDescription,
    instruments: StableDescription,
    result: MusicDna,
}

impl DnaStabilizer {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn clear_evidence(&mut self) {
        self.averaged = AudioFeatures::default();
        self.evidence_started = false;
        self.evaluation_tick = None;
        self.tempo_support = 0.0;
        self.tempo = 0.0;
        self.genre = StableDescription::default();
        self.mood = StableDescription::default();
        self.instruments = StableDescription::default();
        self.result = quiet_music_dna();
    }

    fn update(
        &mut self,
        inference: &mut dyn MusicInference,
        features: &AudioFeatures,
        raw_energy: f32,
        sample_rate: u32,
        time: f64,
    ) -> MusicDna {
        let discontinuity = self.sample_rate.is_some_and(|rate| rate != sample_rate)
            || self
                .last_time
                .is_some_and(|last| time < last || time - last > 0.75);
        if discontinuity {
            self.reset();
        }
        let elapsed = self.last_time.map_or(0.0, |last| (time - last).max(0.0));
        self.last_time = Some(time);
        self.sample_rate = Some(sample_rate);
        if raw_energy < 0.01 {
            let since = *self.silence_since.get_or_insert(time);
            if !self.evidence_started || time - since + 1e-6 >= DNA_SILENCE_CLEAR_SECONDS {
                self.clear_evidence();
            }
            return self.result.clone();
        }
        self.silence_since = None;
        let tick = (time / DNA_EVALUATION_SECONDS + 1e-6).floor() as u64;
        if !self.evidence_started {
            self.averaged = features.clone();
            self.tempo_support = if features.bpm.is_some() { 1.0 } else { 0.0 };
            self.tempo = features.bpm.unwrap_or(0.0);
            self.evidence_started = true;
            self.evaluation_tick = Some(tick);
            self.result = MusicDna {
                genre_hint: "listening / gathering musical context".into(),
                mood: "gathering musical context".into(),
                ..MusicDna::default()
            };
        } else {
            let blend = (1.0 - (-elapsed / DNA_EVIDENCE_SECONDS).exp()) as f32;
            for (average, value) in [
                (&mut self.averaged.bass, features.bass),
                (&mut self.averaged.mids, features.mids),
                (&mut self.averaged.treble, features.treble),
                (&mut self.averaged.energy, features.energy),
                (&mut self.averaged.transient, features.transient),
            ] {
                *average += (value - *average) * blend;
            }
            let tempo_support = if features.bpm.is_some() { 1.0 } else { 0.0 };
            self.tempo_support += (tempo_support - self.tempo_support) * blend;
            if let Some(tempo) = features.bpm {
                self.tempo += (tempo - self.tempo) * blend;
            }
            self.averaged.bpm = (self.tempo_support >= 0.55).then_some(self.tempo);
        }
        if self.evaluation_tick.is_none_or(|previous| tick > previous) {
            self.evaluation_tick = Some(tick);
            let candidate = inference.infer(&self.averaged);
            let decision_time = tick as f64 * DNA_EVALUATION_SECONDS;
            self.genre
                .update(candidate.genre_hint, decision_time, 5.0, 15.0);
            self.mood.update(candidate.mood, decision_time, 3.0, 10.0);
            self.instruments
                .update(candidate.instrument_hints, decision_time, 4.0, 12.0);
            if let Some(label) = &self.genre.current {
                self.result.genre_hint = label.clone();
            }
            if let Some(label) = &self.mood.current {
                self.result.mood = label.clone();
            }
            if let Some(label) = &self.instruments.current {
                self.result.instrument_hints = label.clone();
            }
            self.result.brightness = candidate.brightness;
            self.result.danceability = candidate.danceability;
            self.result.inference_method = candidate.inference_method;
        }
        self.result.clone()
    }
}

#[derive(Clone)]
pub struct FeatureSnapshot {
    pub features: AudioFeatures,
    pub source_epoch: u64,
    pub is_fresh: bool,
    pub last_sample_age: Option<Duration>,
}

struct FeatureState {
    features: AudioFeatures,
    source_epoch: u64,
}

/// Owns FFT, envelopes, beat history and inference outside the UI/render path.
/// Consumers clone only a 48-band feature snapshot, never raw PCM or an FFT.
pub struct FeatureWorker {
    shared: SharedAudio,
    output: Arc<Mutex<FeatureState>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl FeatureWorker {
    pub fn new(shared: SharedAudio) -> Self {
        let output = Arc::new(Mutex::new(FeatureState {
            features: AudioFeatures::default(),
            source_epoch: shared.source_state().epoch,
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let target = output.clone();
        let audio = shared.clone();
        let cancelled = stop.clone();
        let worker = thread::Builder::new()
            .name("music-feature-analysis".into())
            .spawn(move || {
                let mut analyzer = Analyzer::new();
                let mut epoch = u64::MAX;
                let mut sequence = u64::MAX;
                let mut sample_rate = 0;
                let mut sample_time = 0.0_f64;
                let mut last_tick = Instant::now();
                while !cancelled.load(Ordering::Relaxed) {
                    let now = Instant::now();
                    let elapsed = now.duration_since(last_tick).as_secs_f32().min(0.25);
                    last_tick = now;
                    let metadata = audio.source_state();
                    if metadata.epoch != epoch || metadata.sample_rate != sample_rate {
                        // A sample-rate renegotiation is not a new source. Keep
                        // its entry counter monotonic while clearing evidence.
                        let continued_voice =
                            (metadata.epoch == epoch).then(|| std::mem::take(&mut analyzer.voice));
                        analyzer = Analyzer::new();
                        if let Some(mut voice) = continued_voice {
                            analyzer.smooth.voice = voice.clear();
                            analyzer.voice = voice;
                        }
                        epoch = metadata.epoch;
                        sample_rate = metadata.sample_rate;
                        sequence = u64::MAX;
                        sample_time = 0.0;
                    }
                    let mut features = if metadata.is_fresh() && metadata.sequence != sequence {
                        let snapshot = audio.snapshot(FFT_SIZE);
                        // A selection may race the metadata read. Do not analyze
                        // a new source with the previous source's beat history.
                        if snapshot.source.epoch != epoch
                            || snapshot.source.sample_rate != sample_rate
                        {
                            continue;
                        }
                        sequence = snapshot.source.sequence;
                        let next_time =
                            snapshot.source.total_frames as f64 / sample_rate.max(1) as f64;
                        let dt = (next_time - sample_time).clamp(0.001, 0.25) as f32;
                        sample_time = next_time;
                        analyzer.analyze_at(&snapshot.samples, sample_rate, dt, next_time)
                    } else if metadata.is_fresh() {
                        // Never run FFT/onset detection twice on the same block.
                        analyzer.current()
                    } else {
                        analyzer.decay(elapsed)
                    };
                    if !metadata.is_fresh() {
                        features.bpm = None;
                        features.dna = MusicDna::default();
                    }
                    *target.lock().expect("feature snapshot lock") = FeatureState {
                        features,
                        source_epoch: epoch,
                    };
                    thread::sleep(ANALYSIS_INTERVAL);
                }
            })
            .expect("feature worker should start");
        Self {
            shared,
            output,
            stop,
            worker: Some(worker),
        }
    }

    pub fn latest(&self) -> FeatureSnapshot {
        let source = self.shared.source_state();
        let output = self.output.lock().expect("feature snapshot lock");
        let matching = output.source_epoch == source.epoch;
        let is_fresh = matching && source.is_fresh();
        let mut features = if matching {
            output.features.clone()
        } else {
            AudioFeatures::default()
        };
        if !is_fresh {
            features.bpm = None;
            features.dna = MusicDna::default();
            features.voice.presence = 0.0;
        }
        FeatureSnapshot {
            features,
            source_epoch: source.epoch,
            is_fresh,
            last_sample_age: source.last_sample_at.map(|time| time.elapsed()),
        }
    }
}

impl Drop for FeatureWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub struct Analyzer {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    scratch: Vec<Complex32>,
    magnitudes: Vec<f32>,
    previous_magnitudes: Vec<f32>,
    energy_history: VecDeque<f32>,
    beat_times: VecDeque<f64>,
    last_beat: Option<f64>,
    smooth: AudioFeatures,
    inference: Box<dyn MusicInference>,
    dna: DnaStabilizer,
    voice: VoiceDetector,
}

impl Analyzer {
    pub fn new() -> Self {
        Self::with_inference(Box::new(HeuristicInference))
    }

    pub fn with_inference(inference: Box<dyn MusicInference>) -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let window = (0..FFT_SIZE)
            .map(|index| {
                0.5 - 0.5 * (std::f32::consts::TAU * index as f32 / (FFT_SIZE - 1) as f32).cos()
            })
            .collect();
        Self {
            fft,
            window,
            scratch: vec![Complex32::default(); FFT_SIZE],
            magnitudes: vec![0.0; FFT_SIZE / 2],
            previous_magnitudes: vec![0.0; FFT_SIZE / 2],
            energy_history: VecDeque::with_capacity(72),
            beat_times: VecDeque::with_capacity(12),
            last_beat: None,
            smooth: AudioFeatures::default(),
            inference,
            dna: DnaStabilizer::default(),
            voice: VoiceDetector::default(),
        }
    }

    fn current(&self) -> AudioFeatures {
        self.smooth.clone()
    }

    fn decay(&mut self, dt: f32) -> AudioFeatures {
        let dt = finite_dt(dt);
        let decay = (-dt * 4.5).exp();
        self.smooth.bass *= decay;
        self.smooth.mids *= decay;
        self.smooth.treble *= decay;
        self.smooth.energy *= decay;
        self.smooth.transient *= decay;
        self.smooth.beat *= (-dt * 7.0).exp();
        for value in &mut self.smooth.spectrum {
            *value *= decay;
        }
        self.smooth.bpm = None;
        self.smooth.dna = MusicDna::default();
        self.smooth.voice = self.voice.clear();
        self.dna.reset();
        self.beat_times.clear();
        self.last_beat = None;
        self.previous_magnitudes.fill(0.0);
        self.smooth.clone()
    }

    // sample_time is seconds of PCM received from the current source, not wall
    // or frame time. Beat timing therefore does not depend on GPU frame rate.
    pub fn analyze_at(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
        dt: f32,
        sample_time: f64,
    ) -> AudioFeatures {
        if samples.len() < FFT_SIZE || sample_rate == 0 || !sample_time.is_finite() {
            return self.decay(dt);
        }
        let dt = finite_dt(dt);
        let start = samples.len() - FFT_SIZE;
        let frame = &samples[start..];
        let mut square_sum = 0.0;
        for (index, sample) in frame.iter().enumerate() {
            let sample = if sample.is_finite() {
                sample.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            square_sum += sample * sample;
            self.scratch[index] = Complex32::new(sample * self.window[index], 0.0);
        }
        self.fft.process(&mut self.scratch);
        let scale = 2.0 / FFT_SIZE as f32;
        for (magnitude, value) in self.magnitudes.iter_mut().zip(&self.scratch) {
            *magnitude = value.norm() * scale;
        }
        let rms = (square_sum / FFT_SIZE as f32).sqrt();
        let energy = (rms * 4.0).clamp(0.0, 1.0);
        let bass = band_energy(&self.magnitudes, sample_rate, 35.0, 220.0);
        let mids = band_energy(&self.magnitudes, sample_rate, 220.0, 2_400.0);
        let treble = band_energy(&self.magnitudes, sample_rate, 2_400.0, 12_000.0);
        let flux = self
            .magnitudes
            .iter()
            .zip(&self.previous_magnitudes)
            .map(|(now, before)| (now - before).max(0.0))
            .sum::<f32>()
            / self.magnitudes.len() as f32;
        self.previous_magnitudes.copy_from_slice(&self.magnitudes);
        let baseline = self.energy_history.iter().copied().sum::<f32>()
            / self.energy_history.len().max(1) as f32;
        let refractory_ok = self.last_beat.is_none_or(|last| sample_time - last > 0.23);
        let is_beat =
            refractory_ok && energy > 0.045 && energy > baseline * 1.18 && flux > 0.000_12;
        self.energy_history.push_back(energy);
        if self.energy_history.len() > 72 {
            self.energy_history.pop_front();
        }
        if is_beat {
            self.last_beat = Some(sample_time);
            self.beat_times.push_back(sample_time);
            if self.beat_times.len() > 12 {
                self.beat_times.pop_front();
            }
        }
        // Silence invalidates tempo instead of displaying the previous song's
        // last estimate forever. Small pauses preserve timing for two seconds.
        if self.last_beat.is_some_and(|last| sample_time - last > 2.0) {
            self.beat_times.clear();
            self.last_beat = None;
        }
        let attack = 1.0 - (-dt * 16.0).exp();
        let release = 1.0 - (-dt * 4.5).exp();
        self.smooth.bass = envelope(self.smooth.bass, normalize_band(bass), attack, release);
        self.smooth.mids = envelope(self.smooth.mids, normalize_band(mids), attack, release);
        self.smooth.treble = envelope(self.smooth.treble, normalize_band(treble), attack, release);
        self.smooth.energy = envelope(self.smooth.energy, energy, attack, release);
        self.smooth.transient = envelope(
            self.smooth.transient,
            (flux * 500.0).clamp(0.0, 1.0),
            attack,
            release,
        );
        self.smooth.beat = if is_beat {
            1.0
        } else {
            self.smooth.beat * (-dt * 6.0).exp()
        };
        self.smooth.bpm = estimate_bpm(&self.beat_times);
        self.smooth.spectrum = compress_spectrum(&self.magnitudes, 48);
        self.smooth.voice = self.voice.update(&self.magnitudes, sample_rate, rms, dt);
        self.smooth.dna = self.dna.update(
            self.inference.as_mut(),
            &self.smooth,
            energy,
            sample_rate,
            sample_time,
        );
        self.smooth.clone()
    }
}

fn finite_dt(dt: f32) -> f32 {
    if dt.is_finite() {
        dt.clamp(0.0, 0.25)
    } else {
        0.0
    }
}

fn envelope(previous: f32, target: f32, attack: f32, release: f32) -> f32 {
    previous + (target - previous) * if target > previous { attack } else { release }
}

fn normalize_band(value: f32) -> f32 {
    (value * 18.0).sqrt().clamp(0.0, 1.0)
}

fn band_energy(magnitudes: &[f32], sample_rate: u32, low: f32, high: f32) -> f32 {
    if sample_rate == 0 || magnitudes.is_empty() {
        return 0.0;
    }
    let bin_hz = sample_rate as f32 / FFT_SIZE as f32;
    let start = (low / bin_hz).floor() as usize;
    let end = ((high / bin_hz).ceil() as usize).min(magnitudes.len());
    let slice = &magnitudes[start.min(end)..end];
    if slice.is_empty() {
        0.0
    } else {
        slice.iter().copied().sum::<f32>() / slice.len() as f32
    }
}

fn compress_spectrum(magnitudes: &[f32], bands: usize) -> Vec<f32> {
    if magnitudes.is_empty() {
        return vec![0.0; bands];
    }
    (0..bands)
        .map(|band| {
            let t0 = band as f32 / bands as f32;
            let t1 = (band + 1) as f32 / bands as f32;
            let start =
                ((t0.powf(2.4) * magnitudes.len() as f32) as usize).min(magnitudes.len() - 1);
            let end = ((t1.powf(2.4) * magnitudes.len() as f32) as usize)
                .max(start + 1)
                .min(magnitudes.len());
            let value = magnitudes[start..end].iter().copied().sum::<f32>() / (end - start) as f32;
            (value * 28.0).sqrt().clamp(0.0, 1.0)
        })
        .collect()
}

fn estimate_bpm(beats: &VecDeque<f64>) -> Option<f32> {
    if beats.len() < 4 {
        return None;
    }
    let mut intervals: Vec<f64> = beats
        .iter()
        .zip(beats.iter().skip(1))
        .map(|(a, b)| b - a)
        .filter(|interval| (0.25..=1.5).contains(interval))
        .collect();
    if intervals.len() < 3 {
        return None;
    }
    intervals.sort_by(f64::total_cmp);
    let median = intervals[intervals.len() / 2];
    // Withhold a tempo when the recent onsets do not form a plausible pulse.
    let consistent = intervals
        .iter()
        .filter(|interval| (**interval - median).abs() / median < 0.22)
        .count();
    if consistent * 3 < intervals.len() * 2 {
        return None;
    }
    let mut bpm = (60.0 / median) as f32;
    while bpm < 75.0 {
        bpm *= 2.0;
    }
    while bpm > 180.0 {
        bpm *= 0.5;
    }
    Some(bpm)
}

fn infer_music_dna(features: &AudioFeatures) -> MusicDna {
    if features.energy < 0.01 {
        return quiet_music_dna();
    }
    let total = features.bass + features.mids + features.treble + 0.001;
    let brightness = (features.treble / total * 2.2).clamp(0.0, 1.0);
    let danceability = (features.bass * 0.55
        + features.transient * 0.25
        + features
            .bpm
            .map_or(0.1, |bpm| if bpm > 105.0 { 0.35 } else { 0.15 }))
    .clamp(0.0, 1.0);
    let genre_hint = if features.energy < 0.13 && features.transient < 0.2 {
        "ambient-like / spacious"
    } else if danceability > 0.58 && features.bass > features.mids * 0.85 {
        "electronic-like / dance"
    } else if features.transient > 0.5 && features.mids > features.treble {
        "rock-like / percussive"
    } else if features.energy < 0.3 && features.mids > features.bass * 1.2 {
        "acoustic-like / harmonic"
    } else {
        "hybrid / cinematic"
    };
    let mood = match (features.energy > 0.48, brightness > 0.46) {
        (false, false) => "deep & contemplative",
        (false, true) => "airy & luminous",
        (true, false) => "dark & driving",
        (true, true) => "bright & intense",
    };
    let instrument_hints = if features.transient > 0.55 && features.bass > 0.45 {
        "Drum-like transients · low-frequency bed"
    } else if features.mids > 0.5 {
        "Midrange lead/vocal-like texture · harmonic bed"
    } else if features.treble > 0.48 {
        "Cymbal-like shimmer · bright texture"
    } else {
        "Sustained texture · low-frequency bed"
    };
    MusicDna {
        genre_hint: genre_hint.into(),
        mood: mood.into(),
        instrument_hints: instrument_hints.into(),
        brightness,
        danceability,
        inference_method: HEURISTIC_METHOD,
    }
}

fn quiet_music_dna() -> MusicDna {
    MusicDna {
        genre_hint: "quiet / no genre estimate".into(),
        mood: "quiet".into(),
        ..MusicDna::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ambient_evidence() -> AudioFeatures {
        AudioFeatures {
            bass: 0.12,
            mids: 0.18,
            treble: 0.02,
            energy: 0.08,
            transient: 0.04,
            ..AudioFeatures::default()
        }
    }

    fn dance_evidence() -> AudioFeatures {
        AudioFeatures {
            bass: 0.92,
            mids: 0.35,
            treble: 0.25,
            energy: 0.8,
            transient: 0.85,
            bpm: Some(125.0),
            ..AudioFeatures::default()
        }
    }

    fn describe(tracker: &mut DnaStabilizer, evidence: &AudioFeatures, time: f64) -> MusicDna {
        tracker.update(
            &mut HeuristicInference,
            evidence,
            evidence.energy,
            48_000,
            time,
        )
    }

    fn warm_ambient() -> DnaStabilizer {
        let mut tracker = DnaStabilizer::default();
        for frame in 0..=1_200 {
            describe(&mut tracker, &ambient_evidence(), f64::from(frame) / 60.0);
        }
        assert_eq!(tracker.result.genre_hint, "ambient-like / spacious");
        tracker
    }

    #[test]
    fn rapidly_alternating_genre_evidence_does_not_flicker() {
        let mut tracker = warm_ambient();
        let mut previous = tracker.result.genre_hint.clone();
        let mut changes = Vec::new();
        for frame in 1..=3_600 {
            let time = 20.0 + f64::from(frame) / 60.0;
            let evidence = if (frame / 60) % 2 == 0 {
                ambient_evidence()
            } else {
                dance_evidence()
            };
            let dna = describe(&mut tracker, &evidence, time);
            if dna.genre_hint != previous {
                changes.push(time);
                previous = dna.genre_hint;
            }
        }
        assert!(changes.len() <= 2, "genre flickered at {changes:?}");
        for pair in changes.windows(2) {
            assert!(pair[1] - pair[0] >= 15.0);
        }
    }

    #[test]
    fn sustained_new_character_eventually_updates_after_multi_second_evidence() {
        let mut tracker = warm_ambient();
        let original_mood = tracker.result.mood.clone();
        let original_instruments = tracker.result.instrument_hints.clone();
        for frame in 1..=120 {
            let dna = describe(
                &mut tracker,
                &dance_evidence(),
                20.0 + f64::from(frame) / 60.0,
            );
            assert_eq!(dna.genre_hint, "ambient-like / spacious");
            assert_eq!(dna.mood, original_mood);
            assert_eq!(dna.instrument_hints, original_instruments);
        }
        for frame in 121..=1_500 {
            describe(
                &mut tracker,
                &dance_evidence(),
                20.0 + f64::from(frame) / 60.0,
            );
        }
        assert_eq!(tracker.result.genre_hint, "electronic-like / dance");
        assert_eq!(tracker.result.mood, "dark & driving");
        assert_eq!(
            tracker.result.instrument_hints,
            "Drum-like transients · low-frequency bed"
        );
        assert_eq!(tracker.result.inference_method, HEURISTIC_METHOD);
    }

    #[test]
    fn description_candidates_and_minimum_dwell_are_time_based() {
        let mut label = StableDescription::default();
        for half_second in 0..=10 {
            label.update("first".into(), f64::from(half_second) * 0.5, 5.0, 15.0);
        }
        assert_eq!(label.current.as_deref(), Some("first"));
        for half_second in 11..40 {
            label.update("second".into(), f64::from(half_second) * 0.5, 5.0, 15.0);
            assert_eq!(label.current.as_deref(), Some("first"));
        }
        label.update("second".into(), 20.0, 5.0, 15.0);
        assert_eq!(label.current.as_deref(), Some("second"));
    }

    #[test]
    fn description_results_match_at_thirty_and_sixty_hz_sample_clocks() {
        let run = |hz: u32| {
            let mut tracker = DnaStabilizer::default();
            let mut descriptions = Vec::new();
            for frame in 0..=hz * 60 {
                let time = f64::from(frame) / f64::from(hz);
                let evidence = if time < 18.0 {
                    ambient_evidence()
                } else if time < 36.0 {
                    dance_evidence()
                } else {
                    AudioFeatures {
                        bass: 0.15,
                        mids: 0.8,
                        treble: 0.15,
                        energy: 0.8,
                        transient: 0.85,
                        ..AudioFeatures::default()
                    }
                };
                let dna = describe(&mut tracker, &evidence, time);
                if frame % hz == 0 {
                    descriptions.push((dna.genre_hint, dna.mood, dna.instrument_hints));
                }
            }
            descriptions
        };
        assert_eq!(run(30), run(60));
    }

    #[test]
    fn silence_clears_labels_and_resume_requires_new_context() {
        let mut tracker = warm_ambient();
        for frame in 1..=60 {
            describe(
                &mut tracker,
                &AudioFeatures::default(),
                20.0 + f64::from(frame) / 60.0,
            );
        }
        assert_eq!(tracker.result.genre_hint, "quiet / no genre estimate");
        assert_eq!(tracker.result.mood, "quiet");
        assert_eq!(tracker.result.brightness, 0.0);
        let resumed = describe(&mut tracker, &dance_evidence(), 21.02);
        assert_eq!(resumed.genre_hint, "listening / gathering musical context");
    }

    #[test]
    fn source_clock_rate_and_stale_reset_clear_descriptive_history() {
        let mut tracker = warm_ambient();
        let restarted = describe(&mut tracker, &dance_evidence(), 0.02);
        assert_eq!(
            restarted.genre_hint,
            "listening / gathering musical context"
        );
        tracker = warm_ambient();
        let changed_rate = tracker.update(
            &mut HeuristicInference,
            &dance_evidence(),
            0.8,
            44_100,
            20.02,
        );
        assert_eq!(
            changed_rate.genre_hint,
            "listening / gathering musical context"
        );
        tracker = warm_ambient();
        let interrupted = describe(&mut tracker, &dance_evidence(), 22.0);
        assert_eq!(
            interrupted.genre_hint,
            "listening / gathering musical context"
        );
        let mut analyzer = Analyzer::new();
        analyzer.dna = warm_ambient();
        assert_eq!(analyzer.decay(0.1).dna.genre_hint, "waiting for audio");
        assert!(!analyzer.dna.evidence_started);
    }

    #[test]
    fn stable_inference_does_not_slow_or_modify_raw_bands_beats_or_transients() {
        struct NoClassification;
        impl MusicInference for NoClassification {
            fn infer(&mut self, _: &AudioFeatures) -> MusicDna {
                MusicDna::default()
            }
        }
        let mut normal = Analyzer::new();
        let mut other_inference = Analyzer::with_inference(Box::new(NoClassification));
        let mut history = VecDeque::from(vec![0.0; FFT_SIZE]);
        let mut onsets = 0;
        for block in 0..150 {
            for index in block * 960..(block + 1) * 960 {
                let time = index as f32 / 48_000.0;
                let pulse = (-((time * 2.0).fract()) * 60.0).exp();
                let sample = pulse * (std::f32::consts::TAU * 80.0 * time).sin() * 0.9;
                history.pop_front();
                history.push_back(sample);
            }
            let frame: Vec<f32> = history.iter().copied().collect();
            let time = f64::from(block + 1) * 0.02;
            let actual = normal.analyze_at(&frame, 48_000, 0.02, time);
            let comparison = other_inference.analyze_at(&frame, 48_000, 0.02, time);
            assert_eq!(actual.bass, comparison.bass);
            assert_eq!(actual.mids, comparison.mids);
            assert_eq!(actual.treble, comparison.treble);
            assert_eq!(actual.energy, comparison.energy);
            assert_eq!(actual.beat, comparison.beat);
            assert_eq!(actual.transient, comparison.transient);
            assert_eq!(actual.spectrum, comparison.spectrum);
            assert_eq!(actual.bpm, comparison.bpm);
            if actual.beat == 1.0 {
                onsets += 1;
                assert!(actual.transient > 0.0);
            }
        }
        assert!(onsets >= 5);
        assert_eq!(
            normal.current().dna.genre_hint,
            "listening / gathering musical context"
        );
    }

    fn tone(hz: f32, start: usize, count: usize) -> Vec<f32> {
        (start..start + count)
            .map(|index| (std::f32::consts::TAU * hz * index as f32 / 48_000.0).sin() * 0.5)
            .collect()
    }

    #[test]
    fn fft_distinguishes_bass_mid_and_treble_tones() {
        for (hz, band) in [(80.0, 0), (1_000.0, 1), (6_000.0, 2)] {
            let features = Analyzer::new().analyze_at(&tone(hz, 0, FFT_SIZE), 48_000, 0.1, 0.1);
            let values = [features.bass, features.mids, features.treble];
            assert!(
                values[band] > values[(band + 1) % 3] * 2.0,
                "{hz}: {values:?}"
            );
            assert!(
                values[band] > values[(band + 2) % 3] * 2.0,
                "{hz}: {values:?}"
            );
            assert!(features.energy > 0.1);
        }
    }

    #[test]
    fn voice_evidence_is_computed_from_pcm_and_clears_on_stale_or_new_source() {
        let mut analyzer = Analyzer::new();
        for tick in 0..90 {
            let samples: Vec<f32> = (0..FFT_SIZE)
                .map(|index| {
                    let time = (tick * 768 + index) as f32 / 48_000.0;
                    (1..=18)
                        .map(|harmonic| {
                            let hz = 180.0 * harmonic as f32;
                            let envelope = (-((hz - 700.0) / 180.0).powi(2)).exp()
                                + (-((hz - 1450.0) / 250.0).powi(2)).exp() * 0.8
                                + (-((hz - 2500.0) / 320.0).powi(2)).exp() * 0.35;
                            (time * hz * std::f32::consts::TAU).sin() * envelope * 0.035
                        })
                        .sum()
                })
                .collect();
            analyzer.analyze_at(&samples, 48_000, 0.016, (tick + 1) as f64 * 0.016);
        }
        let live = analyzer.current();
        assert_eq!(live.voice.entry, 1);
        assert!(live.voice.presence > 0.65);
        assert!(live.spectrum.iter().any(|value| *value > 0.01));
        let stale = analyzer.decay(0.016);
        assert_eq!(
            stale.voice,
            VoiceActivity {
                presence: 0.0,
                entry: 1
            }
        );
        assert_eq!(Analyzer::new().current().voice, VoiceActivity::default());
    }

    #[test]
    fn silence_invalid_input_and_stale_frames_stay_finite_and_quiet() {
        let mut analyzer = Analyzer::new();
        let features = analyzer.analyze_at(&vec![0.0; FFT_SIZE], 48_000, 0.02, 0.02);
        assert_eq!(features.energy, 0.0);
        assert_eq!(features.beat, 0.0);
        assert!(features.bpm.is_none());
        let invalid = analyzer.analyze_at(&vec![f32::NAN; FFT_SIZE], 48_000, f32::NAN, 0.04);
        assert!(invalid.spectrum.iter().all(|value| value.is_finite()));
        analyzer.analyze_at(&tone(80.0, 0, FFT_SIZE), 48_000, 0.1, 0.2);
        for _ in 0..20 {
            analyzer.decay(0.25);
        }
        let stale = analyzer.current();
        assert!(stale.energy < 0.001);
        assert!(stale.bpm.is_none());
        assert_eq!(stale.dna.genre_hint, "waiting for audio");
    }

    #[test]
    fn beat_tempo_uses_sample_time_and_expires_after_silence() {
        let mut analyzer = Analyzer::new();
        let hop = 960;
        let mut history = VecDeque::from(vec![0.0; FFT_SIZE]);
        let mut onsets = 0;
        let mut estimated = None;
        for block in 0..500 {
            for index in block * hop..(block + 1) * hop {
                let t = index as f32 / 48_000.0;
                let phase = (t * 2.0).fract();
                let sample = if t < 8.0 {
                    (-phase * 60.0).exp() * (std::f32::consts::TAU * 80.0 * t).sin() * 0.9
                } else {
                    0.0
                };
                history.pop_front();
                history.push_back(sample);
            }
            let frame: Vec<f32> = history.iter().copied().collect();
            let result = analyzer.analyze_at(&frame, 48_000, 0.02, (block + 1) as f64 * 0.02);
            if result.beat == 1.0 {
                onsets += 1;
            }
            if block > 200 && block < 350 {
                estimated = result.bpm.or(estimated);
            }
        }
        assert!(onsets >= 12, "only {onsets} detected onsets");
        assert!((estimated.expect("regular pulse should produce a tempo") - 120.0).abs() < 4.0);
        assert!(
            analyzer.current().bpm.is_none(),
            "tempo must expire after silence"
        );
    }

    #[test]
    fn irregular_onsets_do_not_invent_a_reliable_tempo() {
        let beats = VecDeque::from([0.0, 0.26, 0.7, 1.5, 2.8, 3.15]);
        assert!(estimate_bpm(&beats).is_none());
    }

    #[test]
    fn worker_publishes_and_invalidates_freshness_on_source_failure() {
        let (audio, push, fail) = crate::audio::test_source();
        let worker = FeatureWorker::new(audio);
        push(&tone(80.0, 0, FFT_SIZE), 48_000);
        let deadline = Instant::now() + Duration::from_secs(1);
        while worker.latest().features.energy <= 0.01 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        let live = worker.latest();
        assert!(live.is_fresh);
        assert!(live.features.energy > 0.01);
        assert!(live.last_sample_age.is_some());
        fail();
        let stopped = worker.latest();
        assert!(!stopped.is_fresh);
        assert!(stopped.features.bpm.is_none());
        assert_eq!(stopped.features.dna.genre_hint, "waiting for audio");
    }
}
