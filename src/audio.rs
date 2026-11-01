use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_SAMPLES: usize = 48_000 * 4;
pub const FRESH_AUDIO_FOR: Duration = Duration::from_millis(350);
const STDERR_TAIL_BYTES: usize = 2_048;
const DIAGNOSTIC_LINE_BYTES: usize = 4_096;
const SIGNAL_RMS_THRESHOLD: f32 = 0.000_01;
static MICROPHONE_STARTING: AtomicBool = AtomicBool::new(false);

mod diagnostics;
pub use diagnostics::verify_system_audio;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioMode {
    System,
    Microphone,
    Demo,
}

impl AudioMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System Audio",
            Self::Microphone => "Microphone",
            Self::Demo => "Demo Signal",
        }
    }
}

/// Transport state and audible signal are separate. An engine starting (or a
/// permission dialog closing) never establishes that music was received.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CapturePhase {
    Starting,
    AwaitingSamples,
    ReceivingSignal,
    ReceivingSilence,
    Stalled,
    PermissionDenied,
    TransportError,
    Unavailable,
    Stopped,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureHealth {
    pub phase: CapturePhase,
    pub backend: Option<String>,
    pub message: String,
    pub frames_received: u64,
    pub sample_rate: u32,
    pub last_block_rms: f32,
    pub last_block_peak: f32,
    pub fresh: bool,
}

// Samples, their format, source identity and freshness share one lock. A callback
// from a retired source can neither append samples nor overwrite its successor's
// status. The UI only reads small metadata; the DSP worker copies the ring.
struct AudioBuffer {
    samples: VecDeque<f32>,
    sample_rate: u32,
    epoch: u64,
    sequence: u64,
    total_frames: u64,
    last_sample_at: Option<Instant>,
    accepting: bool,
    mode: AudioMode,
    started_at: Instant,
    status: String,
    phase: CapturePhase,
    backend: Option<String>,
    last_block_rms: f32,
    last_block_peak: f32,
}

#[derive(Clone)]
pub struct SharedAudio {
    state: Arc<Mutex<AudioBuffer>>,
}

#[derive(Clone, Copy)]
pub struct SourceState {
    pub epoch: u64,
    pub sequence: u64,
    pub sample_rate: u32,
    pub total_frames: u64,
    pub last_sample_at: Option<Instant>,
    pub accepting: bool,
}

impl SourceState {
    pub fn is_fresh(self) -> bool {
        self.accepting
            && self
                .last_sample_at
                .is_some_and(|time| time.elapsed() <= FRESH_AUDIO_FOR)
    }
}

pub struct AudioSnapshot {
    pub samples: Vec<f32>,
    pub source: SourceState,
}

impl AudioBuffer {
    fn source_state(&self) -> SourceState {
        SourceState {
            epoch: self.epoch,
            sequence: self.sequence,
            sample_rate: self.sample_rate,
            total_frames: self.total_frames,
            last_sample_at: self.last_sample_at,
            accepting: self.accepting,
        }
    }

    fn health(&self) -> CaptureHealth {
        let fresh = self.source_state().is_fresh();
        let phase = if fresh {
            if self.last_block_rms > SIGNAL_RMS_THRESHOLD {
                CapturePhase::ReceivingSignal
            } else {
                CapturePhase::ReceivingSilence
            }
        } else if self.accepting && self.last_sample_at.is_some() {
            CapturePhase::Stalled
        } else {
            self.phase
        };
        let backend = self.backend.as_deref().unwrap_or(match self.mode {
            AudioMode::Demo => "synthetic demo",
            AudioMode::Microphone => "default input",
            AudioMode::System => "capture",
        });
        let message = match phase {
            CapturePhase::ReceivingSignal => format!(
                "{} · {backend}, {} Hz audio received",
                self.mode.label(),
                self.sample_rate
            ),
            CapturePhase::ReceivingSilence => format!(
                "{} · connected, receiving silence; play audio in another app",
                self.mode.label()
            ),
            CapturePhase::Stalled => format!(
                "{} · waiting for fresh audio; stream stalled",
                self.mode.label()
            ),
            CapturePhase::Starting if self.started_at.elapsed() > Duration::from_secs(3) => {
                format!(
                    "{} · startup has not completed; no audio received",
                    self.mode.label()
                )
            }
            _ => self.status.clone(),
        };
        CaptureHealth {
            phase,
            backend: self.backend.clone(),
            message,
            frames_received: self.total_frames,
            sample_rate: self.sample_rate,
            last_block_rms: self.last_block_rms,
            last_block_peak: self.last_block_peak,
            fresh,
        }
    }
}

impl SharedAudio {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(AudioBuffer {
                samples: VecDeque::with_capacity(MAX_SAMPLES),
                sample_rate: 48_000,
                epoch: 0,
                sequence: 0,
                total_frames: 0,
                last_sample_at: None,
                accepting: false,
                mode: AudioMode::Demo,
                started_at: Instant::now(),
                status: String::new(),
                phase: CapturePhase::Stopped,
                backend: None,
                last_block_rms: 0.0,
                last_block_peak: 0.0,
            })),
        }
    }

    fn begin(&self, mode: AudioMode) -> Source {
        let mut state = self.state.lock().expect("audio state lock");
        state.epoch = state.epoch.wrapping_add(1);
        state.sequence = 0;
        state.total_frames = 0;
        state.sample_rate = 48_000;
        state.samples.clear();
        state.last_sample_at = None;
        state.accepting = true;
        state.mode = mode;
        state.started_at = Instant::now();
        state.status = format!("Starting {}…", mode.label());
        state.phase = CapturePhase::Starting;
        state.backend = None;
        state.last_block_rms = 0.0;
        state.last_block_peak = 0.0;
        Source {
            shared: self.clone(),
            epoch: state.epoch,
        }
    }

    fn stop(&self) {
        let mut state = self.state.lock().expect("audio state lock");
        state.epoch = state.epoch.wrapping_add(1);
        state.accepting = false;
        state.samples.clear();
        state.last_sample_at = None;
        state.phase = CapturePhase::Stopped;
    }

    pub fn source_state(&self) -> SourceState {
        self.state.lock().expect("audio state lock").source_state()
    }

    pub fn snapshot(&self, count: usize) -> AudioSnapshot {
        let state = self.state.lock().expect("audio state lock");
        let skip = state.samples.len().saturating_sub(count);
        AudioSnapshot {
            samples: state.samples.iter().skip(skip).copied().collect(),
            source: state.source_state(),
        }
    }

    pub fn health(&self) -> CaptureHealth {
        self.state.lock().expect("audio state lock").health()
    }

    fn status(&self) -> String {
        self.health().message
    }
}

#[derive(Clone)]
struct Source {
    shared: SharedAudio,
    epoch: u64,
}

impl Source {
    fn active(&self) -> bool {
        let state = self.shared.state.lock().expect("audio state lock");
        state.epoch == self.epoch && state.accepting
    }

    fn push_mono(&self, data: impl IntoIterator<Item = f32>, sample_rate: u32) {
        if sample_rate == 0 {
            return;
        }
        let mut state = self.shared.state.lock().expect("audio state lock");
        if state.epoch != self.epoch || !state.accepting {
            return;
        }
        if state.sample_rate != sample_rate {
            state.samples.clear();
            state.total_frames = 0;
            state.sample_rate = sample_rate;
        }
        let mut received = 0_u64;
        let mut square_sum = 0.0_f64;
        let mut peak = 0.0_f32;
        for sample in data {
            let sample = if sample.is_finite() {
                sample.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            if state.samples.len() == MAX_SAMPLES {
                state.samples.pop_front();
            }
            state.samples.push_back(sample);
            square_sum += f64::from(sample) * f64::from(sample);
            peak = peak.max(sample.abs());
            received += 1;
        }
        if received > 0 {
            state.total_frames = state.total_frames.saturating_add(received);
            state.sequence = state.sequence.wrapping_add(1);
            state.last_sample_at = Some(Instant::now());
            state.last_block_rms = (square_sum / received as f64).sqrt() as f32;
            state.last_block_peak = peak;
        }
    }

    fn status(&self, message: impl Into<String>) {
        let mut state = self.shared.state.lock().expect("audio state lock");
        if state.epoch == self.epoch && state.accepting {
            state.status = message.into();
        }
    }

    fn fail(&self, message: impl Into<String>) {
        self.fail_as(CapturePhase::TransportError, message);
    }

    fn fail_as(&self, phase: CapturePhase, message: impl Into<String>) {
        let mut state = self.shared.state.lock().expect("audio state lock");
        if state.epoch == self.epoch && state.accepting {
            state.status = message.into();
            state.phase = phase;
            state.accepting = false;
            state.samples.clear();
            state.last_sample_at = None;
        }
    }

    fn diagnostic(&self, diagnostic: HelperDiagnostic) {
        let mut state = self.shared.state.lock().expect("audio state lock");
        if state.epoch != self.epoch || !state.accepting {
            return;
        }
        state.backend = Some(diagnostic.backend);
        let detail = diagnostic.detail.as_deref().unwrap_or("");
        match diagnostic.state.as_str() {
            "running" => {
                state.phase = CapturePhase::AwaitingSamples;
                state.status = format!(
                    "System Audio · {} started; awaiting PCM samples",
                    state.backend.as_deref().unwrap_or("capture")
                );
            }
            "starting" | "fallback" => {
                state.phase = CapturePhase::Starting;
                state.status = format!("System Audio · {}: {detail}", diagnostic.state);
            }
            "error" => {
                state.phase = if diagnostic.category.as_deref() == Some("permission") {
                    CapturePhase::PermissionDenied
                } else if diagnostic.category.as_deref() == Some("unsupported") {
                    CapturePhase::Unavailable
                } else {
                    CapturePhase::TransportError
                };
                state.status = format!(
                    "System Audio · {}: {detail}",
                    diagnostic.category.as_deref().unwrap_or("transport")
                );
                state.last_sample_at = None;
                state.samples.clear();
            }
            _ => {}
        }
    }
}

pub struct AudioController {
    shared: SharedAudio,
    mode: AudioMode,
    worker: Option<(AudioMode, JoinHandle<()>)>,
}

impl AudioController {
    pub fn with_mode(mode: AudioMode) -> Self {
        let mut controller = Self {
            shared: SharedAudio::new(),
            mode,
            worker: None,
        };
        controller.select(mode);
        controller
    }

    pub fn shared(&self) -> SharedAudio {
        self.shared.clone()
    }
    pub fn mode(&self) -> AudioMode {
        self.mode
    }
    pub fn status(&self) -> String {
        self.shared.status()
    }
    pub fn health(&self) -> CaptureHealth {
        self.shared.health()
    }

    pub fn select(&mut self, mode: AudioMode) {
        self.mode = mode;
        let source = self.shared.begin(mode);
        self.reap_worker();
        let failure = source.clone();
        match thread::Builder::new()
            .name(format!("audio-{}", mode.label()))
            .spawn(move || match mode {
                AudioMode::System => run_system_audio(source),
                AudioMode::Microphone => run_microphone(source),
                AudioMode::Demo => run_demo(source),
            }) {
            Ok(worker) => self.worker = Some((mode, worker)),
            Err(error) => failure.fail(format!("Audio worker could not start · {error}")),
        }
    }

    fn reap_worker(&mut self) {
        if let Some((mode, worker)) = self.worker.take() {
            // macOS can block input initialization inside its permission API.
            // Retire that generation immediately; its eventual callbacks are
            // inert. Our demo/system loops are cancellable and always joined.
            if mode != AudioMode::Microphone || worker.is_finished() {
                let _ = worker.join();
            }
        }
    }
}

impl Drop for AudioController {
    fn drop(&mut self) {
        self.shared.stop();
        self.reap_worker();
    }
}

fn run_demo(source: Source) {
    source.status("Demo Signal · synthetic music pulse");
    let mut phase = 0_u64;
    while source.active() {
        let mut block = Vec::with_capacity(960);
        for i in 0..960 {
            let t = (phase + i as u64) as f32 / 48_000.0;
            let beat_phase = (t * 2.0).fract();
            let kick = (-beat_phase * 18.0).exp()
                * (std::f32::consts::TAU * (48.0 + 32.0 * (-beat_phase * 8.0).exp()) * t).sin();
            let bass = (std::f32::consts::TAU * 55.0 * t).sin() * 0.22;
            let pad = (std::f32::consts::TAU * (110.0 + 8.0 * (t * 0.13).sin()) * t).sin() * 0.13
                + (std::f32::consts::TAU * 277.18 * t).sin() * 0.07;
            let shimmer = ((phase + i as u64) as f32 * 0.618_034).sin()
                * (std::f32::consts::TAU * 1_760.0 * t).sin()
                * 0.035;
            block.push(kick * 0.55 + bass + pad + shimmer);
        }
        phase += 960;
        source.push_mono(block, 48_000);
        thread::sleep(Duration::from_millis(20));
    }
}

fn run_microphone(source: Source) {
    // Permission APIs are not cancellable. Bound the number of pending
    // initializations even if sources are changed repeatedly during a prompt.
    if MICROPHONE_STARTING
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        source.fail_as(
            CapturePhase::Unavailable,
            "Microphone initialization is already pending · wait for it to finish, then retry",
        );
        return;
    }
    struct StartupGuard;
    impl Drop for StartupGuard {
        fn drop(&mut self) {
            MICROPHONE_STARTING.store(false, Ordering::Release);
        }
    }
    let startup_guard = StartupGuard;
    let host = cpal::default_host();
    let Some(device) = host.default_input_device() else {
        source.fail_as(
            CapturePhase::Unavailable,
            "Microphone unavailable · no default input device",
        );
        return;
    };
    let device_name = device.to_string();
    let supported = match device.default_input_config() {
        Ok(supported) => supported,
        Err(error) => {
            source.fail(format!("Could not read the microphone format · {error}"));
            return;
        }
    };
    if !source.active() {
        return;
    }
    let sample_rate = supported.sample_rate();
    let channels = supported.channels() as usize;
    let stream_config = supported.config();
    let target = source.clone();
    let errors = source.clone();
    let error_callback = move |error| errors.fail(format!("Microphone error · {error}"));
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            stream_config,
            move |data: &[f32], _| target.push_mono(mix_f32(data, channels), sample_rate),
            error_callback,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            stream_config,
            move |data: &[i16], _| target.push_mono(mix_i16(data, channels), sample_rate),
            error_callback,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            stream_config,
            move |data: &[u16], _| target.push_mono(mix_u16(data, channels), sample_rate),
            error_callback,
            None,
        ),
        format => {
            source.fail(format!(
                "Unsupported microphone sample format: {format:?} · choose Demo Signal"
            ));
            return;
        }
    };
    let stream = match stream {
        Ok(stream) => stream,
        Err(error) => {
            let phase = if known_permission_denial(&error.to_string()) {
                CapturePhase::PermissionDenied
            } else {
                CapturePhase::TransportError
            };
            source.fail_as(phase, format!("Microphone startup failed · {error}"));
            return;
        }
    };
    if !source.active() {
        return;
    }
    if let Err(error) = stream.play() {
        source.fail(format!("Microphone could not start · {error}"));
        return;
    }
    drop(startup_guard);
    source.status(format!("Microphone · {device_name}"));
    while source.active() {
        thread::sleep(Duration::from_millis(20));
    }
}

fn mix_f32(data: &[f32], channels: usize) -> impl Iterator<Item = f32> + '_ {
    let channels = channels.max(1);
    data.chunks_exact(channels).map(move |frame| {
        frame
            .iter()
            .map(|sample| {
                if sample.is_finite() {
                    sample.clamp(-1.0, 1.0)
                } else {
                    0.0
                }
            })
            .sum::<f32>()
            / channels as f32
    })
}

fn mix_i16(data: &[i16], channels: usize) -> impl Iterator<Item = f32> + '_ {
    let channels = channels.max(1);
    data.chunks_exact(channels).map(move |frame| {
        frame
            .iter()
            .map(|sample| *sample as f32 / 32_768.0)
            .sum::<f32>()
            / channels as f32
    })
}

fn mix_u16(data: &[u16], channels: usize) -> impl Iterator<Item = f32> + '_ {
    let channels = channels.max(1);
    data.chunks_exact(channels).map(move |frame| {
        frame
            .iter()
            .map(|sample| (*sample as f32 - 32_768.0) / 32_768.0)
            .sum::<f32>()
            / channels as f32
    })
}

enum CaptureEvent {
    Data(Vec<u8>),
    Eof,
    Error(String),
}

#[derive(Debug, Deserialize)]
struct HelperDiagnostic {
    backend: String,
    state: String,
    rate: Option<u32>,
    channels: Option<u16>,
    category: Option<String>,
    detail: Option<String>,
}

fn parse_helper_diagnostic(line: &[u8]) -> Result<Option<HelperDiagnostic>, String> {
    let Some(json) = line.strip_prefix(b"AMARA_CAPTURE ") else {
        return Ok(None);
    };
    let diagnostic: HelperDiagnostic =
        serde_json::from_slice(json).map_err(|error| format!("invalid capture status: {error}"))?;
    if !matches!(
        diagnostic.state.as_str(),
        "starting" | "running" | "fallback" | "error"
    ) {
        return Err("unknown capture status state".into());
    }
    if !matches!(
        diagnostic.backend.as_str(),
        "coreaudio" | "screencapturekit"
    ) {
        return Err("unknown capture backend".into());
    }
    if diagnostic.rate.is_some_and(|rate| rate != 48_000)
        || diagnostic.channels.is_some_and(|channels| channels != 1)
    {
        return Err("capture protocol requires 48 kHz mono PCM".into());
    }
    Ok(Some(diagnostic))
}

#[derive(Default)]
struct PcmDecoder {
    pending: Vec<u8>,
}

impl PcmDecoder {
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<f32>, &'static str> {
        self.pending.extend_from_slice(bytes);
        let aligned = self.pending.len() - self.pending.len() % 4;
        let mut samples = Vec::with_capacity(aligned / 4);
        for bytes in self.pending[..aligned].chunks_exact(4) {
            let sample = f32::from_le_bytes(bytes.try_into().expect("four bytes"));
            if !sample.is_finite() {
                return Err("capture helper emitted non-finite PCM");
            }
            samples.push(sample);
        }
        self.pending.drain(..aligned);
        Ok(samples)
    }
}

fn known_permission_denial(detail: &str) -> bool {
    let detail = detail.to_lowercase();
    [
        "permission denied",
        "not authorized",
        "not authorised",
        "declined tcc",
        "user declined",
        "not permitted",
    ]
    .iter()
    .any(|needle| detail.contains(needle))
}

fn run_system_audio(source: Source) {
    let Some(helper) = system_audio_helper() else {
        source.fail_as(
            CapturePhase::Unavailable,
            "System Audio helper missing · run the macOS build script or choose a source",
        );
        return;
    };
    run_capture_helper(source, Command::new(helper));
}

fn run_capture_helper(source: Source, mut command: Command) {
    if !source.active() {
        return;
    }
    source.status("System Audio · starting capture; no PCM received yet");
    let mut child = match command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            source.fail(format!("System Audio could not start · {error}"));
            return;
        }
    };
    let (Some(mut stdout), Some(mut stderr)) = (child.stdout.take(), child.stderr.take()) else {
        source.fail("System Audio helper returned no stream");
        let _ = child.kill();
        let _ = child.wait();
        return;
    };
    // Blocking pipe reads live in readers, never in the cancellation loop. Both
    // pipes are drained concurrently; memory and queued PCM are bounded.
    let (sender, receiver) = mpsc::sync_channel(4);
    let stdout_reader = thread::spawn(move || {
        let mut bytes = [0_u8; 16_384];
        loop {
            let event = match stdout.read(&mut bytes) {
                Ok(0) => CaptureEvent::Eof,
                Ok(count) => CaptureEvent::Data(bytes[..count].to_vec()),
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(error) => CaptureEvent::Error(error.to_string()),
            };
            let terminal = !matches!(event, CaptureEvent::Data(_));
            if sender.send(event).is_err() || terminal {
                break;
            }
        }
    });
    let stderr_tail = Arc::new(Mutex::new(VecDeque::with_capacity(STDERR_TAIL_BYTES)));
    let tail = stderr_tail.clone();
    let diagnostics_source = source.clone();
    let stderr_reader = thread::spawn(move || {
        let mut bytes = [0_u8; 4_096];
        let mut line = Vec::with_capacity(DIAGNOSTIC_LINE_BYTES);
        let mut oversized = false;
        loop {
            match stderr.read(&mut bytes) {
                Ok(0) => break,
                Ok(count) => {
                    {
                        let mut tail = tail.lock().expect("capture diagnostics lock");
                        for byte in &bytes[..count] {
                            if tail.len() == STDERR_TAIL_BYTES {
                                tail.pop_front();
                            }
                            tail.push_back(*byte);
                        }
                    }
                    for byte in &bytes[..count] {
                        if *byte == b'\n' {
                            if !oversized {
                                match parse_helper_diagnostic(&line) {
                                    Ok(Some(diagnostic)) => {
                                        diagnostics_source.diagnostic(diagnostic)
                                    }
                                    Ok(None) => {}
                                    Err(error) => diagnostics_source
                                        .fail_as(CapturePhase::TransportError, error),
                                }
                            }
                            line.clear();
                            oversized = false;
                        } else if line.len() < DIAGNOSTIC_LINE_BYTES {
                            line.push(*byte);
                        } else {
                            oversized = true;
                        }
                    }
                }
                Err(error) if error.kind() == ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
    });
    let mut decoder = PcmDecoder::default();
    let mut failure = None;
    while source.active() {
        match receiver.recv_timeout(Duration::from_millis(25)) {
            Ok(CaptureEvent::Data(bytes)) => {
                let samples = match decoder.push(&bytes) {
                    Ok(samples) => samples,
                    Err(error) => {
                        failure = Some(error.to_owned());
                        break;
                    }
                };
                source.push_mono(samples, 48_000);
            }
            Ok(CaptureEvent::Error(error)) => {
                failure = Some(format!("System Audio read error · {error}"));
                break;
            }
            Ok(CaptureEvent::Eof) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {}
                Err(error) => {
                    failure = Some(format!("System Audio process error · {error}"));
                    break;
                }
            },
        }
    }
    // Always terminate and reap, even during silent playback or a permission
    // wait. Closing the receiver releases a reader waiting on a full queue.
    let _ = child.kill();
    let exit = child.wait().ok();
    drop(receiver);
    let _ = stdout_reader.join();
    let _ = stderr_reader.join();
    if source.active() {
        let bytes: Vec<u8> = stderr_tail
            .lock()
            .expect("capture diagnostics lock")
            .iter()
            .copied()
            .collect();
        let detail = String::from_utf8_lossy(&bytes);
        let detail = detail.trim();
        let health = source.shared.health();
        let permission =
            health.phase == CapturePhase::PermissionDenied || known_permission_denial(detail);
        let phase = if permission {
            CapturePhase::PermissionDenied
        } else if health.phase == CapturePhase::Unavailable {
            CapturePhase::Unavailable
        } else {
            CapturePhase::TransportError
        };
        let message = if permission {
            "System Audio permission denied · allow Audio Capture or Screen & System Audio Recording in System Settings, then retry".to_owned()
        } else if let Some(failure) = failure {
            format!("System Audio transport error · {failure}")
        } else if !decoder.pending.is_empty() {
            "System Audio transport error · truncated float32 PCM".to_owned()
        } else if matches!(
            health.phase,
            CapturePhase::TransportError | CapturePhase::Unavailable
        ) {
            health.message
        } else if detail.is_empty() {
            format!(
                "System Audio transport stopped · {}",
                exit.map(|status| status.to_string())
                    .unwrap_or_else(|| "unknown exit".into())
            )
        } else {
            format!("System Audio transport stopped · {detail}")
        };
        source.fail_as(phase, message);
    }
}

fn system_audio_helper() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("AMARA_SYSTEM_AUDIO_HELPER") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    if let Ok(executable) = std::env::current_exe() {
        let candidates = helper_candidates(&executable);
        if let Some(found) = candidates.into_iter().find(|path| path.is_file()) {
            return Some(found);
        }
    }
    option_env!("AMARA_AUDIO_HELPER_BUILD_PATH")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
}

fn helper_candidates(executable: &Path) -> [PathBuf; 3] {
    let contents = executable.parent().and_then(Path::parent);
    [
        contents
            .map(|path| path.join("Helpers/amaraAudioCapture"))
            .unwrap_or_default(),
        executable.with_file_name("amaraAudioCapture"),
        contents
            .map(|path| path.join("Resources/amaraAudioCapture"))
            .unwrap_or_default(),
    ]
}

#[cfg(test)]
pub(crate) fn test_source() -> (SharedAudio, impl Fn(&[f32], u32), impl Fn()) {
    let shared = SharedAudio::new();
    let source = shared.begin(AudioMode::Demo);
    let writer = source.clone();
    (
        shared,
        move |samples, rate| writer.push_mono(samples.iter().copied(), rate),
        move || source.fail("Test source stopped"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helper_resolution_prefers_signed_nested_code_and_keeps_legacy_fallback() {
        let executable =
            Path::new("/Applications/amaraMusicVisualiser.app/Contents/MacOS/amaraMusicVisualiser");
        let candidates = helper_candidates(executable);
        assert_eq!(
            candidates[0],
            Path::new("/Applications/amaraMusicVisualiser.app/Contents/Helpers/amaraAudioCapture")
        );
        assert_eq!(
            candidates[1],
            executable.with_file_name("amaraAudioCapture")
        );
        assert_eq!(
            candidates[2],
            Path::new(
                "/Applications/amaraMusicVisualiser.app/Contents/Resources/amaraAudioCapture"
            )
        );
    }

    #[test]
    fn source_change_rejects_retired_callbacks_and_status() {
        let shared = SharedAudio::new();
        let old = shared.begin(AudioMode::Microphone);
        old.push_mono([0.8; 32], 44_100);
        let current = shared.begin(AudioMode::Demo);
        old.push_mono([1.0; 32], 44_100);
        old.fail("old failure");
        current.push_mono([0.2; 8], 48_000);
        assert_eq!(shared.snapshot(100).samples, vec![0.2; 8]);
        assert_eq!(shared.source_state().sample_rate, 48_000);
        assert!(shared.source_state().is_fresh());
        assert!(!shared.status().contains("old failure"));
    }

    #[test]
    fn failure_and_age_cannot_report_fresh_audio() {
        let shared = SharedAudio::new();
        let source = shared.begin(AudioMode::System);
        source.push_mono([0.8; 8], 48_000);
        shared.state.lock().unwrap().last_sample_at = Some(Instant::now() - Duration::from_secs(1));
        assert!(!shared.source_state().is_fresh());
        assert!(shared.status().contains("waiting for fresh audio"));
        source.fail("permission denied");
        source.push_mono([1.0; 8], 48_000);
        assert!(shared.snapshot(100).samples.is_empty());
        assert!(!shared.source_state().is_fresh());
    }

    #[test]
    fn ring_is_bounded_sanitized_and_format_consistent() {
        let shared = SharedAudio::new();
        let source = shared.begin(AudioMode::Demo);
        source.push_mono(std::iter::repeat_n(2.0, MAX_SAMPLES + 17), 48_000);
        assert_eq!(
            shared.snapshot(MAX_SAMPLES + 100).samples.len(),
            MAX_SAMPLES
        );
        source.push_mono([f32::NAN, f32::INFINITY, -2.0], 44_100);
        let snapshot = shared.snapshot(100);
        assert_eq!(snapshot.samples, vec![0.0, 0.0, -1.0]);
        assert_eq!(snapshot.source.sample_rate, 44_100);
        assert_eq!(snapshot.source.total_frames, 3);
    }

    #[test]
    fn mixing_averages_channels_without_integer_overflow_or_partial_frames() {
        assert_eq!(mix_i16(&[i16::MAX; 64], 64).count(), 1);
        assert!(mix_i16(&[i16::MAX; 64], 64).next().unwrap() > 0.99);
        assert_eq!(mix_u16(&[0, u16::MAX], 2).next().unwrap(), -1.0 / 65_536.0);
        assert_eq!(mix_f32(&[1.0, -1.0, 0.5], 2).collect::<Vec<_>>(), vec![0.0]);
        assert_eq!(mix_f32(&[f32::NAN, 1.0], 2).next().unwrap(), 0.5);
        assert_eq!(mix_f32(&[0.25], 0).next().unwrap(), 0.25);
    }

    #[test]
    fn silent_helper_is_cancelled_and_reaped_promptly() {
        let shared = SharedAudio::new();
        let source = shared.begin(AudioMode::System);
        let mut command = Command::new("/bin/sh");
        // exec leaves no shell child holding the pipes open.
        command.args(["-c", "exec /bin/sleep 30"]);
        let worker = thread::spawn(move || run_capture_helper(source, command));
        thread::sleep(Duration::from_millis(50));
        let start = Instant::now();
        shared.stop();
        worker.join().unwrap();
        assert!(start.elapsed() < Duration::from_secs(1));
        assert!(!shared.source_state().is_fresh());
    }

    #[test]
    fn helper_diagnostics_are_drained_and_bounded() {
        let shared = SharedAudio::new();
        let source = shared.begin(AudioMode::System);
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "i=0; while [ $i -lt 4000 ]; do echo diagnostic-line >&2; i=$((i+1)); done",
        ]);
        let start = Instant::now();
        run_capture_helper(source, command);
        assert!(start.elapsed() < Duration::from_secs(3));
        assert!(shared.status().len() <= STDERR_TAIL_BYTES + 100);
        assert!(shared.status().contains("diagnostic-line"));
    }

    #[test]
    fn pcm_decoder_preserves_partial_frames_and_rejects_invalid_samples() {
        let bytes: Vec<_> = [0.25_f32, -0.75, 0.0]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect();
        let mut decoder = PcmDecoder::default();
        assert!(decoder.push(&bytes[..3]).unwrap().is_empty());
        assert_eq!(decoder.push(&bytes[3..7]).unwrap(), vec![0.25]);
        assert_eq!(decoder.pending.len(), 3);
        assert_eq!(decoder.push(&bytes[7..]).unwrap(), vec![-0.75, 0.0]);
        assert!(decoder.pending.is_empty());
        assert!(decoder.push(&f32::NAN.to_le_bytes()).is_err());
    }

    #[test]
    fn helper_protocol_validates_format_without_misclassifying_startup_as_live() {
        let parsed = parse_helper_diagnostic(br#"AMARA_CAPTURE {"backend":"coreaudio","state":"running","rate":48000,"channels":1,"source_rate":44100}"#)
            .unwrap().unwrap();
        let shared = SharedAudio::new();
        let source = shared.begin(AudioMode::System);
        source.diagnostic(parsed);
        let health = shared.health();
        assert_eq!(health.phase, CapturePhase::AwaitingSamples);
        assert!(!health.fresh);
        assert_eq!(health.frames_received, 0);
        assert_eq!(health.backend.as_deref(), Some("coreaudio"));
        assert!(
            parse_helper_diagnostic(b"a legacy unstructured diagnostic")
                .unwrap()
                .is_none()
        );
        assert!(parse_helper_diagnostic(br#"AMARA_CAPTURE {"backend":"coreaudio","state":"running","rate":44100,"channels":2}"#).is_err());
        assert!(
            parse_helper_diagnostic(br#"AMARA_CAPTURE {"backend":"coreaudio","state":"unknown"}"#)
                .is_err()
        );
        assert!(!known_permission_denial(
            "permission preflight completed; output device unavailable"
        ));
        assert!(known_permission_denial(
            "The user declined TCCs for the application"
        ));
    }

    #[test]
    fn silence_signal_stall_and_permission_failure_are_distinct() {
        let shared = SharedAudio::new();
        let source = shared.begin(AudioMode::System);
        source.push_mono([0.0; 128], 48_000);
        assert_eq!(shared.health().phase, CapturePhase::ReceivingSilence);
        assert!(shared.health().fresh);
        source.push_mono([0.2, -0.2], 48_000);
        assert_eq!(shared.health().phase, CapturePhase::ReceivingSignal);
        assert!((shared.health().last_block_rms - 0.2).abs() < 0.000_001);
        shared.state.lock().unwrap().last_sample_at = Some(Instant::now() - Duration::from_secs(1));
        assert_eq!(shared.health().phase, CapturePhase::Stalled);
        assert!(!shared.health().fresh);
        let diagnostic = parse_helper_diagnostic(br#"AMARA_CAPTURE {"backend":"screencapturekit","state":"error","category":"permission","detail":"User declined access"}"#).unwrap().unwrap();
        source.diagnostic(diagnostic);
        assert_eq!(shared.health().phase, CapturePhase::PermissionDenied);
        assert!(!shared.health().fresh);
        assert!(shared.snapshot(128).samples.is_empty());
        // Recoverable fallback diagnostics do not poison a later valid stream.
        source.diagnostic(parse_helper_diagnostic(br#"AMARA_CAPTURE {"backend":"screencapturekit","state":"running","rate":48000,"channels":1}"#).unwrap().unwrap());
        source.push_mono([0.15; 128], 48_000);
        assert_eq!(shared.health().phase, CapturePhase::ReceivingSignal);
    }

    #[test]
    fn structured_helper_failure_survives_noisy_diagnostics_and_is_not_live() {
        let shared = SharedAudio::new();
        let source = shared.begin(AudioMode::System);
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf '%s\\n' 'AMARA_CAPTURE {\"backend\":\"screencapturekit\",\"state\":\"error\",\"category\":\"permission\",\"detail\":\"User declined access\"}' >&2; i=0; while [ $i -lt 4000 ]; do echo trailing-diagnostic >&2; i=$((i+1)); done; exit 4"]);
        run_capture_helper(source, command);
        assert_eq!(shared.health().phase, CapturePhase::PermissionDenied);
        assert!(!shared.health().fresh);
        assert!(shared.status().contains("permission denied"));
    }

    #[test]
    fn actual_helper_pipe_decodes_pcm_and_cancels_without_faking_silence() {
        let shared = SharedAudio::new();
        let source = shared.begin(AudioMode::System);
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf '%s\\n' 'AMARA_CAPTURE {\"backend\":\"coreaudio\",\"state\":\"running\",\"rate\":48000,\"channels\":1}' >&2; printf '\\000\\000'; printf '\\000\\077'; exec /bin/sleep 30"]);
        let worker = thread::spawn(move || run_capture_helper(source, command));
        let deadline = Instant::now() + Duration::from_secs(1);
        while shared.health().frames_received == 0 && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(shared.snapshot(10).samples, vec![0.5]);
        assert_eq!(shared.health().phase, CapturePhase::ReceivingSignal);
        assert!(shared.health().fresh);
        shared.stop();
        worker.join().unwrap();
        assert!(!shared.health().fresh);
    }

    #[test]
    fn truncated_helper_pcm_is_a_transport_error_not_permission_denial() {
        let shared = SharedAudio::new();
        let source = shared.begin(AudioMode::System);
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "printf '\\001\\002\\003'"]);
        run_capture_helper(source, command);
        assert_eq!(shared.health().phase, CapturePhase::TransportError);
        assert!(shared.status().contains("truncated float32"));
        assert!(!shared.health().fresh);
    }
}
