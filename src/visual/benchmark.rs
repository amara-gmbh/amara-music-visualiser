//! Bounded offscreen rendering latency check: no audio capture, image readback,
//! surface presentation or display-FPS claim. Times include CPU/driver work and
//! waiting for submitted GPU work to complete, rather than GPU timestamp queries.
use super::{AudioFeatures, HISTORY_FORMAT, SCENES, ShaderUniforms, VisualResources, wgpu};
use crate::{
    choreography::Choreographer,
    math_journey::MathJourney,
    presets::{MathForm, VisualSettings},
};
use anyhow::Context;
use serde::Serialize;
use std::time::{Duration, Instant};

const WARMUP_FRAMES: usize = 6;
const MEASURED_FRAMES: usize = 30;
const DT: f32 = 1.0 / 60.0;

#[derive(Serialize)]
struct TimingSummary {
    mean_ms: f64,
    p50_ms: f64,
    p95_ms: f64,
    maximum_ms: f64,
}

pub fn benchmark_render() -> anyhow::Result<()> {
    benchmark_filtered(None, false)
}

pub fn benchmark_attractor() -> anyhow::Result<()> {
    benchmark_filtered(Some(&[5]), false)
}

pub fn benchmark_new_worlds() -> anyhow::Result<()> {
    benchmark_filtered(Some(&[11, 12, 13]), false)
}

pub fn benchmark_disco_silk() -> anyhow::Result<()> {
    benchmark_filtered(Some(&[12]), true)
}

pub fn benchmark_aurora() -> anyhow::Result<()> {
    benchmark_filtered(Some(&[9]), true)
}

pub fn benchmark_neon_coast() -> anyhow::Result<()> {
    benchmark_filtered(Some(&[14]), true)
}

pub fn benchmark_specialization() -> anyhow::Result<()> {
    benchmark_filtered(None, true)
}

fn benchmark_filtered(worlds: Option<&[usize]>, specialized: bool) -> anyhow::Result<()> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: if cfg!(target_os = "macos") {
            wgpu::Backends::METAL
        } else {
            wgpu::Backends::all()
        },
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .context("no GPU adapter; run the render benchmark in a logged-in desktop session")?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("amara bounded render benchmark"),
        ..Default::default()
    }))?;
    let mut renderer = VisualResources::new(&device, HISTORY_FORMAT);
    let build_start = Instant::now();
    let mut preparation_frames = Vec::new();
    let mut first_prepared_ms = None;
    let coast_latency_only = specialized && worlds.is_some_and(|worlds| worlds == [14]);
    if coast_latency_only {
        // This diagnostic measures Coast latency, not whole-app startup. Silk
        // is raster, so its Coast transition shares the held Coast mask.
        renderer.specialize(
            &device,
            &[1 << 14, (1 << 14) | (1 << 11), (1 << 14) | (1 << 9)],
        );
    } else if specialized {
        let progress = renderer.prepare_async(&device, 5);
        let visual = VisualSettings {
            scene: 5,
            ..Default::default()
        };
        let audio = synthetic_audio();
        let mappings = crate::presets::AudioMappings::default();
        loop {
            let start = Instant::now();
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
            let input = ShaderUniforms::new(12.0, DT, 5, 5, 1.0, &visual, &mappings, &audio);
            renderer.render(&device, &queue, &mut encoder, input, [640, 400]);
            let submission = queue.submit([encoder.finish()]);
            device.poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(20)),
            })?;
            preparation_frames.push(start.elapsed().as_secs_f64() * 1000.0);
            if first_prepared_ms.is_none()
                && renderer.prepared_pipelines.iter().any(|(key, _)| *key == 0)
            {
                first_prepared_ms = Some(build_start.elapsed().as_secs_f64() * 1000.0);
            }
            if progress.load(std::sync::atomic::Ordering::Relaxed) >= super::pipelines::COUNT {
                renderer.receive_prepared(0);
                anyhow::ensure!(
                    renderer.prepared_pipelines.len() == super::pipelines::COUNT,
                    "not all finite pipeline keys prepared successfully"
                );
                break;
            }
            anyhow::ensure!(
                build_start.elapsed() < Duration::from_secs(90),
                "finite preparation timed out"
            );
            std::thread::sleep(Duration::from_millis(8));
        }
    }
    let specialization_build_ms = build_start.elapsed().as_secs_f64() * 1_000.0;
    let audio = synthetic_audio();
    let mappings = crate::presets::AudioMappings::default();
    let mut cases = Vec::new();

    // Fixed mathematical families and a real two-family midpoint expose the
    // cost of automatic variety; other outer worlds remain directly comparable.
    let workloads = [
        (1, 0, 0, 1.0, "Julia"),
        (1, 1, 1, 1.0, "Inversion Lace"),
        (1, 2, 2, 1.0, "Harmonic Sculpture"),
        (1, 3, 3, 1.0, "Vortex Weave"),
        (1, 0, 1, 0.5, "Julia → Inversion Lace midpoint"),
        (1, 1, 2, 0.5, "Inversion Lace → Harmonic Sculpture midpoint"),
        (1, 2, 3, 0.5, "Harmonic Sculpture → Vortex Weave midpoint"),
        (1, 3, 0, 0.5, "Vortex Weave → Julia midpoint"),
        (0, 0, 0, 1.0, "Deep Flight"),
        (2, 0, 0, 1.0, "Glass Tide"),
        (3, 0, 0, 1.0, "Feedback Reverie"),
        (4, 0, 0, 1.0, "Particle Constellation"),
        (5, 0, 0, 1.0, "Strange Attractor"),
        (6, 0, 0, 1.0, "Spectral Landscape"),
        (7, 0, 0, 1.0, "Dot Equaliser"),
        (8, 0, 0, 1.0, "Water Ripples"),
        (9, 0, 0, 1.0, "Northern Lights"),
        (10, 0, 0, 1.0, "Mandelbrot"),
        (11, 0, 0, 1.0, "Liquid Stained Glass"),
        (12, 0, 0, 1.0, "Disco Silk"),
        (13, 0, 0, 1.0, "Neon Memory"),
        (14, 0, 0, 1.0, "Neon Coast"),
        (14, 11, 0, 1.0, "Liquid Stained Glass → Neon Coast midpoint"),
        (9, 14, 0, 1.0, "Neon Coast → Northern Lights midpoint"),
        (14, 12, 0, 1.0, "Disco Silk → Neon Coast midpoint"),
        (12, 11, 0, 1.0, "Liquid Stained Glass → Disco Silk midpoint"),
        (13, 12, 0, 1.0, "Disco Silk → Neon Memory midpoint"),
        (7, 5, 0, 1.0, "Strange Attractor → Dot Equaliser midpoint"),
        (11, 0, 0, 1.0, "Deep Flight → Liquid Stained Glass midpoint"),
        (9, 2, 0, 1.0, "Glass Tide → Northern Lights midpoint"),
        (
            13,
            11,
            0,
            1.0,
            "Liquid Stained Glass → Neon Memory midpoint",
        ),
        (13, 9, 0, 1.0, "Northern Lights → Neon Memory midpoint"),
        (3, 2, 0, 1.0, "Glass Tide → Feedback Reverie bridge"),
        (8, 2, 0, 1.0, "Glass Tide → Water Ripples bridge"),
    ];
    let family_names = [
        "Julia",
        "Inversion Lace",
        "Harmonic Sculpture",
        "Vortex Weave",
    ];
    for (scene, math_from, math_to, math_blend, workload) in workloads {
        let outer_morph = scene != 1 && workload.contains('→');
        if worlds.is_some_and(|only| {
            !only.contains(&scene) && !(specialized && outer_morph && only.contains(&math_from))
        }) {
            continue;
        }
        let source_world = if outer_morph { math_from } else { scene };
        let world_blend = if outer_morph { 0.5 } else { 1.0 };
        for size in [[1280, 800], [1600, 1000]] {
            eprintln!("Measuring {workload} at {}×{}…", size[0], size[1]);
            let visual = VisualSettings {
                scene,
                complexity: 1.5,
                ..VisualSettings::default()
            };
            renderer.history = None;
            let mut phrase = Choreographer::new();
            let mut journey = MathJourney::new();
            let mut render_time = 12.0;
            let mut encode = Vec::with_capacity(MEASURED_FRAMES);
            let mut submitted_wait = Vec::with_capacity(MEASURED_FRAMES);
            let mut completed = Vec::with_capacity(MEASURED_FRAMES);
            for frame in 0..WARMUP_FRAMES + MEASURED_FRAMES {
                let choreography = phrase.update_mapped(&audio, &mappings, DT);
                let mathematics = journey.update(&audio, &mappings, MathForm::Journey, DT);
                render_time += DT * visual.motion * choreography.travel_speed;
                let uniforms = ShaderUniforms::new(
                    render_time,
                    DT,
                    scene,
                    source_world,
                    world_blend,
                    &visual,
                    &mappings,
                    &audio,
                )
                .with_choreography(choreography)
                .with_math_blend(math_from, math_to, math_blend, 12.0 + mathematics.phase)
                .with_auto_mix(if outer_morph { 0.75 } else { 0.0 }, 1.7);
                let start = Instant::now();
                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("amara measured scene frame"),
                });
                renderer.render(&device, &queue, &mut encoder, uniforms, size);
                let commands = encoder.finish();
                let encoded = Instant::now();
                let submission = queue.submit([commands]);
                device
                    .poll(wgpu::PollType::Wait {
                        submission_index: Some(submission),
                        timeout: Some(Duration::from_secs(20)),
                    })
                    .context("GPU completion wait failed during render benchmark")?;
                let finished = Instant::now();
                if frame >= WARMUP_FRAMES {
                    encode.push(encoded.duration_since(start).as_secs_f64() * 1_000.0);
                    submitted_wait.push(finished.duration_since(encoded).as_secs_f64() * 1_000.0);
                    completed.push(finished.duration_since(start).as_secs_f64() * 1_000.0);
                }
            }
            cases.push(serde_json::json!({
                "scene": SCENES[scene],
                "scene_index": scene,
                "source_world": source_world,
                "world_blend": world_blend,
                "prepared_mask": specialized.then(|| super::pipelines::frame_mask(source_world as f32, scene as f32, world_blend)),
                "workload": workload,
                "mathematics": if scene == 1 { Some(serde_json::json!({
                    "from": family_names[math_from],
                    "to": family_names[math_to],
                    "blend": math_blend,
                    "phase_offset": 12.0
                })) } else { None },
                "canvas_pixels": { "width": size[0], "height": size[1] },
                "detail": visual.complexity,
                "palette": format!("{:?}", visual.palette),
                "feedback": visual.feedback,
                "warmup_frames": WARMUP_FRAMES,
                "measured_frames": MEASURED_FRAMES,
                "simulation_delta_seconds": DT,
                "cpu_encode": summarize(&encode),
                "submission_and_completion_wait": summarize(&submitted_wait),
                "completed_frame_wall": summarize(&completed)
            }));
        }
    }
    let report = serde_json::json!({
        "status": "complete",
        "specialized": specialized,
        "specialization_build_ms": specialization_build_ms,
        "preparation": if coast_latency_only { Some(serde_json::json!({
            "mode":"three Coast workload masks prepared before timing; diagnostic only, not app startup",
            "keys":renderer.prepared_pipelines.len()
        })) } else if specialized { Some(serde_json::json!({
            "mode":"actual finite worker while 640×400 frames continue; cache state is inherited from earlier validation, not claimed cold",
            "keys":renderer.prepared_pipelines.len(),"initial_world_first_prepared_ms":first_prepared_ms,
            "frames":preparation_frames.len(),"frame_completion_wall":summarize(&preparation_frames)
        })) } else {None},
        "adapter": {
            "name": info.name,
            "backend": format!("{:?}", info.backend),
            "device_type": format!("{:?}", info.device_type)
        },
        "measurement": "Unpaced, serial offscreen scene+feedback rendering with CPU encoding, queue submission and an explicit wait for each GPU submission to complete.",
        "time_unit": "milliseconds",
        "percentile": "nearest-rank p95 of 30 measured frames per case, after six warmup frames",
        "limitations": [
            "Wall-clock completion latency includes CPU/driver/scheduler overhead; it is not a pure GPU timestamp measurement.",
            "The timed pass is the app's actual procedural scene and feedback renderer; native UI, final presentation, compositor and vsync are excluded.",
            "Serial completion waits do not measure pipelined throughput, display FPS or perceptual smoothness.",
            "A bounded 30-frame sample does not establish rare-stutter behavior or every time/parameter combination."
        ],
        "privacy": "Synthetic in-memory audio features only. No capture, playback, image readback, recording or permission request.",
        "cases": cases
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn summarize(milliseconds: &[f64]) -> TimingSummary {
    debug_assert!(!milliseconds.is_empty());
    let mut sorted = milliseconds.to_vec();
    sorted.sort_by(f64::total_cmp);
    let p95_index = ((sorted.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
    let middle = sorted.len() / 2;
    let p50 = if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) * 0.5
    } else {
        sorted[middle]
    };
    TimingSummary {
        mean_ms: milliseconds.iter().sum::<f64>() / milliseconds.len() as f64,
        p50_ms: p50,
        p95_ms: sorted[p95_index],
        maximum_ms: sorted[sorted.len() - 1],
    }
}

fn synthetic_audio() -> AudioFeatures {
    AudioFeatures {
        bass: 0.85,
        mids: 0.6,
        treble: 0.7,
        energy: 0.8,
        beat: 0.65,
        transient: 0.4,
        spectrum: (0..48)
            .map(|band| {
                let position = band as f32 / 47.0;
                let low = (-((position - 0.1) / 0.07).powi(2)).exp();
                let mid = (-((position - 0.42) / 0.12).powi(2)).exp();
                (0.08 + low * 0.55 + mid * 0.38 + (band as f32 * 1.37).sin().abs() * 0.22)
                    .clamp(0.0, 1.0)
            })
            .collect(),
        ..AudioFeatures::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latency_summary_uses_nearest_rank_p95_without_hiding_an_outlier() {
        let mut timings: Vec<f64> = (1..=29).map(f64::from).collect();
        timings.push(1_000.0);
        let summary = summarize(&timings);
        assert!((summary.mean_ms - 47.833_333_333).abs() < 0.000_001);
        assert_eq!(summary.p50_ms, 15.5);
        assert_eq!(summary.p95_ms, 29.0);
        assert_eq!(summary.maximum_ms, 1_000.0);
    }
}
