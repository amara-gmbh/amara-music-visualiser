//! An optional real-GPU check. It exercises the same renderer used by the app,
//! reads only synthetic frames, and saves reviewable images and measurements.
use super::*;
use crate::choreography::Choreographer;
use anyhow::Context;
use std::{fs, path::Path, sync::mpsc, time::Duration};

const SIZE: [u32; 2] = [640, 400];
const PREVIEW_TIMES: [f32; 4] = [2.0, 12.0, 32.0, 72.0];
const MIN_DARK_FRACTION: f64 = 0.05;
const DARK_LUMA: f64 = 32.0;
const MIN_SPATIAL_LUMA_DEVIATION: f64 = 5.0;
const MAX_TEMPORAL_STEP: f64 = 18.0;
const MIN_WORLD_DIFFERENCE: f64 = 4.0;
const MIN_DETAIL_DIFFERENCE: f64 = 0.25;
const MAX_HIGH_DETAIL_DARK_FRACTION: f64 = 0.90;
const PHRASE_HZ: u32 = 60;
const LANDSCAPE_PROBE_SHADER: &str = r#"
    @group(0) @binding(3) var geometry: texture_2d_array<f32>;
    @vertex fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
        let positions = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0),
            vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
        return vec4<f32>(positions[index], 0.0, 1.0);
    }
    @fragment fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
        let light = max(textureLoad(geometry, vec2<i32>(position.xy), 2, 0).rgb,
            vec3<f32>(0.0));
        return vec4<f32>(vec3<f32>(1.0) - exp(-light * 3.0), 1.0);
    }
"#;

struct PhraseSnapshot {
    seconds: u32,
    stage: &'static str,
    audio: AudioFeatures,
    phrase: Choreography,
    paused_phrase: Choreography,
    render_time: f32,
}

struct PhraseSequence {
    snapshots: Vec<PhraseSnapshot>,
    reset_before: Choreography,
    reset_after: Choreography,
    max_phase_step: f32,
    max_render_step: f32,
}

pub fn verify_render(destination: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
    // A failed rerun must not leave an older success report looking current.
    fs::write(
        destination.join("renderer-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "status": "incomplete",
            "reason": "Verification started; only a completed run writes status pass.",
            "input": "synthetic feature values; no personal audio read"
        }))?,
    )?;
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
    .context("no GPU adapter; run this check in a logged-in desktop session")?;
    let info = adapter.get_info();
    println!("Checking real GPU: {} ({:?})", info.name, info.backend);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("amara renderer verification"),
        ..Default::default()
    }))?;
    let mut renderer = VisualResources::new(&device, HISTORY_FORMAT);
    let profile = crate::presets::Profile::astral_bloom();
    let mut visual = profile.visual.clone();
    visual.feedback = 0.0;
    let quiet = AudioFeatures::default();
    let music = AudioFeatures {
        bass: 0.85,
        mids: 0.6,
        treble: 0.7,
        energy: 0.8,
        beat: 0.85,
        // A deterministic uneven contour, not a flat stand-in for FFT data.
        spectrum: (0..48)
            .map(|band| {
                let position = band as f32 / 47.0;
                let low_peak = (-((position - 0.1) / 0.07).powi(2)).exp();
                let mid_peak = (-((position - 0.42) / 0.12).powi(2)).exp();
                let detail = (band as f32 * 1.37).sin().abs();
                (0.08 + low_peak * 0.55 + mid_peak * 0.38 + detail * 0.22).clamp(0.0, 1.0)
            })
            .collect(),
        ..Default::default()
    };
    let mut no_spectrum = music.clone();
    no_spectrum.spectrum.fill(0.0);
    let mut report = Vec::new();
    let mut previews = Vec::new();
    let mut quiet_previews = Vec::new();
    let mut timeline_previews = Vec::new();
    let mut detail_previews = Vec::new();
    for (scene, name) in SCENES.iter().enumerate() {
        let quiet_uniforms = ShaderUniforms::new(
            12.0,
            1.0 / 60.0,
            scene,
            scene,
            1.0,
            &visual,
            &profile.mappings,
            &quiet,
        );
        renderer.history = None;
        let quiet_pixels = render_frame(&mut renderer, &device, &queue, quiet_uniforms, SIZE)?;
        write_png(
            &destination.join(format!("world-{scene}-quiet.png")),
            &quiet_pixels,
            SIZE,
        )?;
        quiet_previews.push(quiet_pixels.clone());
        let music_uniforms = ShaderUniforms::new(
            12.0,
            1.0 / 60.0,
            scene,
            scene,
            1.0,
            &visual,
            &profile.mappings,
            &music,
        );
        renderer.history = None;
        let music_pixels = render_frame(&mut renderer, &device, &queue, music_uniforms, SIZE)?;
        // Keep the failing frame inspectable too, rather than leaving the old
        // preview in place if a contrast or continuity assertion fails.
        write_png(
            &destination.join(format!("world-{scene}.png")),
            &music_pixels,
            SIZE,
        )?;
        previews.push(music_pixels.clone());
        let statistics = stats(&music_pixels);
        let spatial_detail = luma_deviation(&music_pixels);
        let audio_change = difference(&quiet_pixels, &music_pixels);
        anyhow::ensure!(
            statistics.0 > 2.0 && statistics.0 < 240.0,
            "{name}: frame is black or overexposed"
        );
        anyhow::ensure!(
            statistics.1 > 8.0 && spatial_detail > MIN_SPATIAL_LUMA_DEVIATION,
            "{name}: frame has no useful spatial variation"
        );
        anyhow::ensure!(
            audio_change > 0.6,
            "{name}: synthetic music did not change the GPU output"
        );
        let music_dark_fraction = dark_fraction(&music_pixels);
        let quiet_dark_fraction = dark_fraction(&quiet_pixels);
        anyhow::ensure!(
            music_dark_fraction >= MIN_DARK_FRACTION && quiet_dark_fraction >= MIN_DARK_FRACTION,
            "{name}: quiet/music frames lack useful dark negative space (quiet {quiet_dark_fraction:.3}, music {music_dark_fraction:.3})"
        );
        // With every mapping zero the audio signal must be disconnected.
        let disconnected = AudioMappings {
            bass_to_pulse: 0.0,
            mids_to_geometry: 0.0,
            treble_to_sparkle: 0.0,
            beat_to_flash: 0.0,
        };
        let off_quiet = ShaderUniforms::new(
            12.0,
            1.0 / 60.0,
            scene,
            scene,
            1.0,
            &visual,
            &disconnected,
            &quiet,
        );
        let off_music = ShaderUniforms::new(
            12.0,
            1.0 / 60.0,
            scene,
            scene,
            1.0,
            &visual,
            &disconnected,
            &music,
        );
        let a = render_frame(&mut renderer, &device, &queue, off_quiet, SIZE)?;
        let b = render_frame(&mut renderer, &device, &queue, off_music, SIZE)?;
        anyhow::ensure!(
            difference(&a, &b) < 0.02,
            "{name}: zero mappings still respond to audio"
        );

        // Scalar envelopes, beat, scene and time stay identical. Only the
        // measured-spectrum contour changes, so band-energy modulation alone
        // cannot satisfy this check.
        let (spectrum_change, spectrum_routes) = {
            let mut render_audio = |mappings: &AudioMappings, features: &AudioFeatures| {
                renderer.history = None;
                render_frame(
                    &mut renderer,
                    &device,
                    &queue,
                    ShaderUniforms::new(
                        12.0,
                        1.0 / 60.0,
                        scene,
                        scene,
                        1.0,
                        &visual,
                        mappings,
                        features,
                    ),
                    SIZE,
                )
            };
            let without_spectrum = render_audio(&profile.mappings, &no_spectrum)?;
            let spectrum_change = difference(&without_spectrum, &music_pixels);
            anyhow::ensure!(
                spectrum_change > 0.05,
                "{name}: changing only the FFT spectrum did not affect the GPU output"
            );

            let mut routes = Vec::new();
            for (band, start, end) in [("bass", 0, 8), ("mids", 8, 24), ("treble", 24, 48)] {
                let mut isolated = no_spectrum.clone();
                isolated.spectrum[start..end].copy_from_slice(&music.spectrum[start..end]);
                let enabled = AudioMappings {
                    bass_to_pulse: f32::from(start == 0),
                    mids_to_geometry: f32::from(start == 8),
                    treble_to_sparkle: f32::from(start == 24),
                    beat_to_flash: 0.0,
                };
                let baseline = render_audio(&enabled, &no_spectrum)?;
                let routed = render_audio(&enabled, &isolated)?;
                let enabled_difference = difference(&baseline, &routed);
                anyhow::ensure!(
                    enabled_difference > 0.02,
                    "{name}: {band} spectrum bands did not reach their enabled visual mapping"
                );

                // Leave the other mappings enabled: disabling this band's
                // own mapping must disconnect its spectrum bands without cross-talk.
                let disabled = AudioMappings {
                    bass_to_pulse: 1.0 - enabled.bass_to_pulse,
                    mids_to_geometry: 1.0 - enabled.mids_to_geometry,
                    treble_to_sparkle: 1.0 - enabled.treble_to_sparkle,
                    beat_to_flash: 1.0,
                };
                let baseline = render_audio(&disabled, &no_spectrum)?;
                let disconnected = render_audio(&disabled, &isolated)?;
                let disabled_difference = difference(&baseline, &disconnected);
                anyhow::ensure!(
                    disabled_difference < 0.02,
                    "{name}: {band} spectrum bands leak through another band's mapping"
                );
                routes.push(serde_json::json!({
                    "band": band,
                    "spectrum_bands": {"start": start, "end_exclusive": end},
                    "enabled_difference": enabled_difference,
                    "disabled_difference": disabled_difference
                }));
            }
            (spectrum_change, routes)
        };

        // With the audio, scene and view settings held fixed, eight consecutive
        // 60 Hz steps must move without a frame-wide discontinuity. This guard
        // does not replace native playback inspection or measure frame rate.
        let mut previous = music_pixels.clone();
        let mut max_temporal_step = 0.0_f64;
        for step in 1..=8 {
            renderer.history = None;
            let next = render_frame(
                &mut renderer,
                &device,
                &queue,
                ShaderUniforms::new(
                    12.0 + step as f32 / 60.0,
                    1.0 / 60.0,
                    scene,
                    scene,
                    1.0,
                    &visual,
                    &profile.mappings,
                    &music,
                ),
                SIZE,
            )?;
            max_temporal_step = max_temporal_step.max(difference(&previous, &next));
            previous = next;
        }
        let temporal_motion = difference(&music_pixels, &previous);
        anyhow::ensure!(
            max_temporal_step < MAX_TEMPORAL_STEP,
            "{name}: small time step causes a large pixel jump ({max_temporal_step:.3})"
        );
        anyhow::ensure!(
            temporal_motion > 0.02,
            "{name}: fixed musical input produces a static world over eight time steps"
        );

        // The user-facing Detail range must change layered geometry/material,
        // not merely report a different slider value. These fixed-time pairs
        // remain reviewable; pixel distance is a regression guard, not a
        // judgment that one image is more beautiful than the other.
        let mut detail_frames = Vec::new();
        for (label, complexity) in [("low", 0.15), ("high", 1.5)] {
            let mut settings = visual.clone();
            settings.complexity = complexity;
            renderer.history = None;
            let pixels = render_frame(
                &mut renderer,
                &device,
                &queue,
                ShaderUniforms::new(
                    12.0,
                    1.0 / 60.0,
                    scene,
                    scene,
                    1.0,
                    &settings,
                    &profile.mappings,
                    &music,
                ),
                SIZE,
            )?;
            write_png(
                &destination.join(format!("world-{scene}-detail-{label}.png")),
                &pixels,
                SIZE,
            )?;
            detail_frames.push(pixels);
        }
        let detail_difference = difference(&detail_frames[0], &detail_frames[1]);
        let high_detail_dark_fraction = dark_fraction(&detail_frames[1]);
        anyhow::ensure!(
            detail_difference > MIN_DETAIL_DIFFERENCE,
            "{name}: low/max Detail does not meaningfully change GPU output"
        );
        anyhow::ensure!(
            high_detail_dark_fraction < MAX_HIGH_DETAIL_DARK_FRACTION,
            "{name}: maximum Detail remains over 90% dark with synthetic music ({high_detail_dark_fraction:.3})"
        );
        detail_previews.extend(detail_frames);

        let mut timeline = Vec::new();
        for time in PREVIEW_TIMES {
            let pixels = if time == 12.0 {
                music_pixels.clone()
            } else {
                renderer.history = None;
                render_frame(
                    &mut renderer,
                    &device,
                    &queue,
                    ShaderUniforms::new(
                        time,
                        1.0 / 60.0,
                        scene,
                        scene,
                        1.0,
                        &visual,
                        &profile.mappings,
                        &music,
                    ),
                    SIZE,
                )?
            };
            let space = dark_fraction(&pixels);
            anyhow::ensure!(
                space >= MIN_DARK_FRACTION,
                "{name}: preview at {time}s loses dark negative space ({space:.3})"
            );
            let (mean, variation) = stats(&pixels);
            timeline.push(serde_json::json!({
                "time_seconds": time,
                "mean_rgb": mean,
                "standard_deviation": variation,
                "spatial_luma_standard_deviation": luma_deviation(&pixels),
                "dark_fraction": space
            }));
            timeline_previews.push(pixels);
        }
        report.push(serde_json::json!({
            "scene": name,
            "mean_rgb": statistics.0,
            "standard_deviation": statistics.1,
            "spatial_luma_standard_deviation": spatial_detail,
            "audio_difference": audio_change,
            "zero_mappings_difference": difference(&a, &b),
            "spectrum_only_difference": spectrum_change,
            "spectrum_routing": spectrum_routes,
            "dark_fraction": music_dark_fraction,
            "quiet_dark_fraction": quiet_dark_fraction,
            "detail_slider": {
                "low": 0.15,
                "high": 1.5,
                "rgb_difference": detail_difference,
                "high_dark_fraction": high_detail_dark_fraction
            },
            "temporal_continuity": {
                "steps": 8,
                "seconds_per_step": 1.0 / 60.0,
                "max_step_rgb_difference": max_temporal_step,
                "first_last_rgb_difference": temporal_motion
            },
            "timeline": timeline
        }));
        println!(
            "PASS {name}: detail + music/FFT routing + mapping isolation + negative space + temporal steps"
        );
    }

    // Identical settings, audio and time must still yield four different
    // worlds. A pixel-distance floor only catches obvious convergence; the
    // contact sheets remain the evidence for human aesthetic judgment.
    let mut world_pairs = Vec::new();
    for first in 0..SCENES.len() {
        for second in first + 1..SCENES.len() {
            let change = difference(&previews[first], &previews[second]);
            anyhow::ensure!(
                change > MIN_WORLD_DIFFERENCE,
                "{} and {} have converged visually (difference {change:.3})",
                SCENES[first],
                SCENES[second]
            );
            world_pairs.push(serde_json::json!({
                "first": SCENES[first],
                "second": SCENES[second],
                "music_rgb_difference": change,
                "quiet_rgb_difference": difference(&quiet_previews[first], &quiet_previews[second])
            }));
        }
    }

    // Morph endpoints must equal the corresponding unmixed world exactly.
    let from = render_frame(
        &mut renderer,
        &device,
        &queue,
        ShaderUniforms::new(
            12.0,
            1.0 / 60.0,
            0,
            0,
            1.0,
            &visual,
            &profile.mappings,
            &music,
        ),
        SIZE,
    )?;
    let endpoint = render_frame(
        &mut renderer,
        &device,
        &queue,
        ShaderUniforms::new(
            12.0,
            1.0 / 60.0,
            2,
            0,
            0.0,
            &visual,
            &profile.mappings,
            &music,
        ),
        SIZE,
    )?;
    anyhow::ensure!(
        difference(&from, &endpoint) < 0.02,
        "morph start is discontinuous"
    );
    let midpoint = render_frame(
        &mut renderer,
        &device,
        &queue,
        ShaderUniforms::new(
            12.0,
            1.0 / 60.0,
            2,
            0,
            0.5,
            &visual,
            &profile.mappings,
            &music,
        ),
        SIZE,
    )?;
    let to = render_frame(
        &mut renderer,
        &device,
        &queue,
        ShaderUniforms::new(
            12.0,
            1.0 / 60.0,
            2,
            2,
            1.0,
            &visual,
            &profile.mappings,
            &music,
        ),
        SIZE,
    )?;
    let end = render_frame(
        &mut renderer,
        &device,
        &queue,
        ShaderUniforms::new(
            12.0,
            1.0 / 60.0,
            2,
            0,
            1.0,
            &visual,
            &profile.mappings,
            &music,
        ),
        SIZE,
    )?;
    anyhow::ensure!(difference(&to, &end) < 0.02, "morph end is discontinuous");
    anyhow::ensure!(
        difference(&midpoint, &from) > 0.6 && difference(&midpoint, &to) > 0.6,
        "morph midpoint is not a distinct blend"
    );
    write_png(
        &destination.join("transition-midpoint.png"),
        &midpoint,
        SIZE,
    )?;

    // Feedback must use previous pixels; a cold frame ignores history.
    renderer.history = None;
    let initial = ShaderUniforms::new(
        4.0,
        1.0 / 60.0,
        0,
        0,
        1.0,
        &visual,
        &profile.mappings,
        &quiet,
    );
    let _ = render_frame(&mut renderer, &device, &queue, initial, SIZE)?;
    visual.feedback = 0.88;
    let echo_uniforms = ShaderUniforms::new(
        8.0,
        1.0 / 60.0,
        0,
        0,
        1.0,
        &visual,
        &profile.mappings,
        &music,
    );
    let echoed = render_frame(&mut renderer, &device, &queue, echo_uniforms, SIZE)?;
    renderer.history = None;
    let cold = render_frame(&mut renderer, &device, &queue, echo_uniforms, SIZE)?;
    let feedback_change = difference(&echoed, &cold);
    anyhow::ensure!(
        feedback_change > 1.0,
        "feedback is not sampling the previous GPU frame"
    );
    write_png(&destination.join("feedback.png"), &echoed, SIZE)?;
    // Fresh filaments must not turn a warmed, high-retention frame into white.
    renderer.history = None;
    let mut warmed = Vec::new();
    for frame in 0..120 {
        warmed = render_frame(
            &mut renderer,
            &device,
            &queue,
            ShaderUniforms::new(
                8.0 + frame as f32 / 60.0,
                1.0 / 60.0,
                1,
                1,
                1.0,
                &visual,
                &profile.mappings,
                &music,
            ),
            SIZE,
        )?;
    }
    let warm_stats = stats(&warmed);
    anyhow::ensure!(
        warm_stats.0 > 2.0
            && warm_stats.0 < 225.0
            && warm_stats.1 > 8.0
            && luma_deviation(&warmed) > MIN_SPATIAL_LUMA_DEVIATION,
        "high-feedback frame loses exposure or spatial detail after 120 frames"
    );
    write_png(&destination.join("feedback-warm.png"), &warmed, SIZE)?;
    let resized = render_frame(&mut renderer, &device, &queue, echo_uniforms, [320, 240])?;
    anyhow::ensure!(resized.len() == 320 * 240 * 4, "history resize failed");

    // Advance the real phrase controller at 60 Hz, with the same mapped API
    // used by the app. Only checkpoint images are rendered here: this is not
    // a claim that GPU feedback was simulated for the intervening 28 seconds.
    let mut phrase_music = music.clone();
    phrase_music.transient = 0.75;
    let mut phrase_visual = profile.visual.clone();
    phrase_visual.feedback = 0.0;
    let phrase_sequence =
        build_phrase_sequence(&profile.mappings, &phrase_visual, &quiet, &phrase_music)?;
    let disconnected = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let disconnected_sequence =
        build_phrase_sequence(&disconnected, &phrase_visual, &quiet, &phrase_music)?;
    let quiet_phrase = phrase_sequence.snapshots[1].phrase;
    let loud_phrase = phrase_sequence.snapshots[2].phrase;
    let recovered_phrase = phrase_sequence.snapshots[3].phrase;
    anyhow::ensure!(
        loud_phrase.drive > quiet_phrase.drive + 0.4
            && loud_phrase.travel_speed > quiet_phrase.travel_speed + 0.25
            && recovered_phrase.drive < loud_phrase.drive * 0.05,
        "mapped quiet/loud/silence sequence did not build and release its phrase"
    );
    for snapshot in &disconnected_sequence.snapshots {
        anyhow::ensure!(
            snapshot.phrase.drive == 0.0
                && snapshot.phrase.accent == 0.0
                && snapshot.phrase.travel_speed == Choreography::default().travel_speed,
            "all-zero mappings influenced the actual phrase controller"
        );
    }
    let mut phrase_previews = Vec::new();
    let mut phrase_report = Vec::new();
    for (scene, name) in SCENES.iter().enumerate() {
        let mut snapshots = Vec::new();
        let mut paused_difference = 0.0;
        for snapshot in &phrase_sequence.snapshots {
            let uniforms = ShaderUniforms::new(
                snapshot.render_time,
                0.0,
                scene,
                scene,
                1.0,
                &phrase_visual,
                &profile.mappings,
                &snapshot.audio,
            )
            .with_choreography(snapshot.phrase);
            anyhow::ensure!(
                uniforms
                    .values
                    .iter()
                    .chain(&uniforms.spectrum)
                    .flatten()
                    .all(|value| value.is_finite()),
                "{name}: non-finite phrase uniforms at {} seconds",
                snapshot.seconds
            );
            renderer.history = None;
            let pixels = render_frame(&mut renderer, &device, &queue, uniforms, SIZE)?;
            write_png(
                &destination.join(format!("world-{scene}-phrase-{}.png", snapshot.seconds)),
                &pixels,
                SIZE,
            )?;
            let (mean, variation) = stats(&pixels);
            let spatial = luma_deviation(&pixels);
            anyhow::ensure!(
                mean > 2.0
                    && mean < 240.0
                    && variation > 8.0
                    && spatial > MIN_SPATIAL_LUMA_DEVIATION,
                "{name}: phrase checkpoint at {} seconds loses exposure or spatial detail (mean={mean:.2}, RGB deviation={variation:.2}, spatial deviation={spatial:.2})",
                snapshot.seconds
            );
            if snapshot.seconds == 16 {
                let paused = ShaderUniforms::new(
                    snapshot.render_time,
                    0.0,
                    scene,
                    scene,
                    1.0,
                    &phrase_visual,
                    &profile.mappings,
                    &snapshot.audio,
                )
                .with_choreography(snapshot.paused_phrase);
                anyhow::ensure!(
                    bytemuck::bytes_of(&uniforms) == bytemuck::bytes_of(&paused),
                    "{name}: zero-delta phrase update changed shader uniforms"
                );
                renderer.history = None;
                let paused_pixels = render_frame(&mut renderer, &device, &queue, paused, SIZE)?;
                paused_difference = difference(&pixels, &paused_pixels);
                anyhow::ensure!(
                    paused_difference == 0.0,
                    "{name}: zero-delta phrase update changed GPU pixels"
                );
            }
            snapshots.push(serde_json::json!({
                "elapsed_seconds": snapshot.seconds,
                "stage": snapshot.stage,
                "integrated_render_time": snapshot.render_time,
                "phrase": phrase_json(snapshot.phrase),
                "mean_rgb": mean,
                "standard_deviation": variation,
                "spatial_luma_standard_deviation": spatial,
                "dark_fraction": dark_fraction(&pixels)
            }));
            phrase_previews.push(pixels);
        }
        let zero = &disconnected_sequence.snapshots[2];
        let mut render_disconnected = |audio: &AudioFeatures| {
            renderer.history = None;
            render_frame(
                &mut renderer,
                &device,
                &queue,
                ShaderUniforms::new(
                    zero.render_time,
                    0.0,
                    scene,
                    scene,
                    1.0,
                    &phrase_visual,
                    &disconnected,
                    audio,
                )
                .with_choreography(zero.phrase),
                SIZE,
            )
        };
        let zero_quiet = render_disconnected(&quiet)?;
        let zero_music = render_disconnected(&phrase_music)?;
        let zero_difference = difference(&zero_quiet, &zero_music);
        anyhow::ensure!(
            zero_difference == 0.0,
            "{name}: zero mappings leak through the integrated phrase/render path"
        );
        phrase_report.push(serde_json::json!({
            "scene": name,
            "snapshots": snapshots,
            "zero_dt_uniforms_identical": true,
            "zero_dt_pixel_difference": paused_difference,
            "zero_mappings_pixel_difference": zero_difference
        }));
        println!("PASS {name}: real mapped phrase sequence, integrated travel and paused equality");
    }
    let fractal_study = verify_fractal_study(
        &mut renderer,
        &device,
        &queue,
        &profile,
        &music,
        destination,
    )?;
    let palette_study = verify_palette_study(
        &mut renderer,
        &device,
        &queue,
        &profile,
        (&quiet, &music),
        &previews,
        destination,
    )?;
    let math_family_study = verify_math_families(
        &mut renderer,
        &device,
        &queue,
        &profile,
        (&quiet, &music),
        destination,
    )?;
    let canvas_centre_study = verify_canvas_centre(
        &mut renderer,
        &device,
        &queue,
        &profile,
        &music,
        destination,
    )?;
    write_contact_sheet(&destination.join("worlds.png"), &previews, 2)?;
    write_contact_sheet(&destination.join("worlds-quiet.png"), &quiet_previews, 2)?;
    let comparison: Vec<_> = quiet_previews
        .iter()
        .zip(&previews)
        .flat_map(|(quiet, music)| [quiet.clone(), music.clone()])
        .collect();
    write_contact_sheet(&destination.join("worlds-comparison.png"), &comparison, 2)?;
    write_contact_sheet(
        &destination.join("worlds-timeline.png"),
        &timeline_previews,
        PREVIEW_TIMES.len(),
    )?;
    write_contact_sheet(&destination.join("worlds-phrases.png"), &phrase_previews, 4)?;
    write_contact_sheet(&destination.join("worlds-detail.png"), &detail_previews, 2)?;
    fs::write(
        destination.join("preview-guide.txt"),
        format!(
            "Synthetic renderer previews; no personal audio.\n\
             worlds.png / worlds-quiet.png: scenes in row-major order.\n\
             worlds-comparison.png: one scene per row; quiet left, music right.\n\
             worlds-timeline.png: one scene per row; columns at {:?} seconds.\n\
             worlds-phrases.png: one scene per row; columns at 0, 8, 16, 28 seconds of real CPU choreography.\n\
             worlds-detail.png: one scene per row; Detail 0.15 left and 1.5 right, fixed time and audio.\n\
             fractal-study.png: Julia hero at 2, 12, 32 seconds; cold left / 120-frame warmed feedback right.\n\
             fractal-study-native.png: cold Julia hero at 1600 by 1000 pixels, not an FPS benchmark.\n\
             fractal-morph-proof.png: first row fixed camera/audio/light-sum with different phrase drive/lift; second row five seconds at Motion 0.62.\n\
             palettes.png: one world per row; Cosmic left / Mocca & Vanilla right, identical camera/audio.\n\
             mocca-quiet-music.png: one world per row; quiet left / music right. mocca-warm.png: 120-frame warmed worlds.\n\
             math-families.png: one mathematical family per row; Cosmic quiet/music then Mocca quiet/music.\n\
             math-family-times.png: one family per row; start/end small steps at 12 and 32 seconds.\n\
             math-family-morphs.png: cyclic family pairs, with exact source / midpoint / exact destination.\n\
             Math family order: Julia, Inversion Lace, Harmonic Sculpture, Vortex Weave.\n\
             canvas-centre.png: centred/translated cold frames, then centred/translated two-frame feedback.\n\
             Phrase sequence: quiet 8 seconds, loud 8 seconds, silence 12 seconds; GPU snapshots use cold history.\n\
             Scene order: {}.\n\
             Contact sheets support aesthetic review; passing pixel checks is not an aesthetic score.\n",
            PREVIEW_TIMES,
            SCENES.join(", ")
        ),
    )?;
    let result = serde_json::json!({
        "app_version": env!("CARGO_PKG_VERSION"),
        "uniform_bytes": std::mem::size_of::<ShaderUniforms>(),
        "status":"pass", "adapter":info.name, "backend":format!("{:?}",info.backend),
        "size":SIZE,"scenes":report,"morph_endpoints":"pass","feedback_difference":feedback_change,
        "world_pair_differences": world_pairs,
        "fractal_study": fractal_study,
        "palette_study": palette_study,
        "math_family_study": math_family_study,
        "canvas_centre_study": canvas_centre_study,
        "regression_limits": {
            "minimum_world_pair_rgb_difference": MIN_WORLD_DIFFERENCE,
            "minimum_detail_slider_rgb_difference": MIN_DETAIL_DIFFERENCE,
            "maximum_high_detail_dark_fraction": MAX_HIGH_DETAIL_DARK_FRACTION,
            "minimum_dark_fraction": MIN_DARK_FRACTION,
            "dark_display_luma_below": DARK_LUMA,
            "minimum_spatial_luma_standard_deviation": MIN_SPATIAL_LUMA_DEVIATION,
            "maximum_60hz_step_rgb_difference": MAX_TEMPORAL_STEP
        },
        "high_feedback_120_frames":{"mean_rgb":warm_stats.0,"standard_deviation":warm_stats.1,
            "spatial_luma_standard_deviation":luma_deviation(&warmed)},
        "choreography_sequence": {
            "cpu_updates_per_second": PHRASE_HZ,
            "segments": ["quiet 8 seconds", "loud 8 seconds", "silence 12 seconds"],
            "gpu_history": "cold at each checkpoint; not continuous 28-second GPU feedback",
            "maximum_phase_step": phrase_sequence.max_phase_step,
            "maximum_render_time_step": phrase_sequence.max_render_step,
            "reset_before": phrase_json(phrase_sequence.reset_before),
            "reset_after_zero_dt": phrase_json(phrase_sequence.reset_after),
            "worlds": phrase_report
        },
        "resize":"pass", "input":"synthetic feature values; no personal audio read"
    });
    fs::write(
        destination.join("renderer-report.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    println!("PASS morph endpoints, actual framebuffer feedback, resize and readback");
    Ok(())
}

/// Focused previews of actual geometry, ODE trajectories and spectral history.
/// Every fixture has its own synthetic source epoch; no capture device opens.
pub fn preview_media(destination: &Path) -> anyhow::Result<()> {
    const MEDIA_SIZE: [u32; 2] = [1280, 800];
    const HISTORY_WARM_FRAMES: u32 = 64;
    fs::create_dir_all(destination)?;
    fs::write(
        destination.join("media-report.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"status":"incomplete", "input":"synthetic audio only"}),
        )?,
    )?;
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
    .context("no GPU adapter for geometry previews")?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("amara actual-geometry preview"),
        ..Default::default()
    }))?;
    let mut renderer = VisualResources::new(&device, HISTORY_FORMAT);
    let profile = crate::presets::Profile::astral_bloom();
    let mut epoch = 0_u64;
    let mut fixture = |to: usize,
                       from: usize,
                       blend: f32,
                       look: usize,
                       music: bool,
                       palette: crate::presets::VisualPalette|
     -> anyhow::Result<Vec<u8>> {
        epoch += 1;
        renderer.history = None;
        let mut settings = profile.visual.clone();
        settings.feedback = 0.0;
        settings.complexity = 1.35;
        settings.palette = palette;
        let measured_history = matches!(to, 5..=7) || (matches!(from, 5..=7) && blend < 1.0);
        let warm = if measured_history && music {
            HISTORY_WARM_FRAMES
        } else {
            0
        };
        let mut final_pixels = Vec::new();
        for frame in 0..=warm {
            // Every output checkpoint is at t=18.4. History-based fixtures need
            // preceding samples: 64 real submissions with distinct phase stamps.
            let time = if warm > 0 {
                12.0 + frame as f32 * 0.1
            } else {
                18.4
            };
            let audio = if music {
                media_spectrum(time)
            } else {
                AudioFeatures::default()
            };
            let mut looks = [[0.0, 0.0, 1.0, time]; WORLD_COUNT];
            looks[to] = [look as f32, look as f32, 1.0, time];
            let uniforms = ShaderUniforms::new(
                time,
                if warm > 0 { 0.1 } else { 1.0 / 60.0 },
                to,
                from,
                blend,
                &settings,
                &profile.mappings,
                &audio,
            )
            .with_math_blend(0, 0, 1.0, time)
            .with_auto_mix(if blend < 1.0 { 1.0 } else { 0.0 }, 3.0)
            .with_looks(looks)
            .with_audio_state(music, epoch);
            if frame == warm {
                final_pixels = render_frame(&mut renderer, &device, &queue, uniforms, MEDIA_SIZE)?;
            } else {
                // Keep the actual bounded spectral history, without one CPU
                // readback/stall per sample in this short visual review.
                let mut encoder = device.create_command_encoder(&Default::default());
                renderer.render(&device, &queue, &mut encoder, uniforms, MEDIA_SIZE);
                queue.submit([encoder.finish()]);
            }
        }
        Ok(final_pixels)
    };
    let inspect = |pixels: &[u8], label: &str| -> anyhow::Result<serde_json::Value> {
        anyhow::ensure!(
            pixels.len() == (MEDIA_SIZE[0] * MEDIA_SIZE[1] * 4) as usize,
            "{label}: invalid geometry readback dimensions"
        );
        let (mean, deviation) = stats(pixels);
        let spatial = luma_deviation(pixels);
        anyhow::ensure!(
            mean.is_finite()
                && deviation.is_finite()
                && spatial.is_finite()
                && (0.0..=255.0).contains(&mean)
                && (0.0..=255.0).contains(&deviation),
            "{label}: invalid image statistics"
        );
        anyhow::ensure!(
            pixels
                .chunks_exact(4)
                .any(|p| p[..3].iter().any(|v| *v > 1)),
            "{label}: black geometry preview"
        );
        Ok(
            serde_json::json!({"mean_rgb":mean, "rgb_standard_deviation":deviation,
            "spatial_luma_standard_deviation":spatial, "nonblack":true}),
        )
    };
    let mut world_report = Vec::new();
    let mut music_frames = Vec::new();
    let mut sheet = Vec::new();
    for (scene, _) in SCENES.iter().enumerate().skip(4) {
        let quiet = fixture(
            scene,
            scene,
            1.0,
            0,
            false,
            crate::presets::VisualPalette::Cosmic,
        )?;
        let music = fixture(
            scene,
            scene,
            1.0,
            0,
            true,
            crate::presets::VisualPalette::Cosmic,
        )?;
        let quiet_stats = inspect(&quiet, SCENES[scene])?;
        let music_stats = inspect(&music, SCENES[scene])?;
        let change = difference(&quiet, &music);
        anyhow::ensure!(
            change > 0.005,
            "{}: synthetic music did not change geometry output",
            SCENES[scene]
        );
        write_png(
            &destination.join(format!("media-world-{scene}-quiet.png")),
            &quiet,
            MEDIA_SIZE,
        )?;
        write_png(
            &destination.join(format!("media-world-{scene}-music.png")),
            &music,
            MEDIA_SIZE,
        )?;
        sheet.push(media_thumbnail(&quiet)?);
        sheet.push(media_thumbnail(&music)?);
        music_frames.push(music);
        world_report.push(serde_json::json!({"scene":scene, "name":SCENES[scene],
            "quiet":quiet_stats, "music":music_stats, "music_rgb_difference":change,
            "spectral_history_warm_submissions":if matches!(scene, 5..=7) { HISTORY_WARM_FRAMES } else { 0 }}));
    }
    write_contact_sheet(&destination.join("media-quiet-music.png"), &sheet, 2)?;
    let mut pairs = Vec::new();
    for a in 0..music_frames.len() {
        for b in a + 1..music_frames.len() {
            let change = difference(&music_frames[a], &music_frames[b]);
            anyhow::ensure!(change > 0.005, "new geometry worlds have identical output");
            pairs.push(
                serde_json::json!({"from":SCENES[a+4],"to":SCENES[b+4],"rgb_difference":change}),
            );
        }
    }
    let mut variants = Vec::new();
    let mut variant_report = Vec::new();
    for (scene, look) in [
        (4, 0),
        (4, 2),
        (4, 6),
        (5, 1),
        (5, 2),
        (5, 3),
        (7, 0),
        (7, 3),
        (7, 7),
    ] {
        let image = fixture(
            scene,
            scene,
            1.0,
            look,
            true,
            crate::presets::VisualPalette::Cosmic,
        )?;
        let measured = inspect(&image, LOOKS[scene][look])?;
        let file = format!("media-world-{scene}-look-{look}.png");
        write_png(&destination.join(&file), &image, MEDIA_SIZE)?;
        variants.push(media_thumbnail(&image)?);
        variant_report.push(serde_json::json!({"scene":scene,"look":look,
            "name":LOOKS[scene][look],"file":file,"image":measured}));
    }
    write_contact_sheet(&destination.join("media-geometry-looks.png"), &variants, 3)?;
    let mut old_comparisons = Vec::new();
    for old_scene in [1, 3] {
        let old = fixture(
            old_scene,
            old_scene,
            1.0,
            0,
            true,
            crate::presets::VisualPalette::Cosmic,
        )?;
        for (index, music) in music_frames.iter().enumerate() {
            let change = difference(&old, music);
            anyhow::ensure!(
                change > 0.005,
                "old/new geometry worlds have identical output"
            );
            old_comparisons.push(
                serde_json::json!({"old":SCENES[old_scene],"new":SCENES[index+4],
                "rgb_difference":change}),
            );
        }
    }
    let mut transitions = Vec::new();
    let mut transition_report = Vec::new();
    for (from, to) in [(4, 1), (5, 6), (4, 7), (7, 0)] {
        let image = fixture(
            to,
            from,
            0.5,
            0,
            true,
            crate::presets::VisualPalette::Cosmic,
        )?;
        let measured = inspect(&image, "mixed geometry prepasses")?;
        let file = format!("media-transition-{from}-to-{to}.png");
        write_png(&destination.join(&file), &image, MEDIA_SIZE)?;
        transitions.push(media_thumbnail(&image)?);
        transition_report.push(serde_json::json!({"from":SCENES[from],"to":SCENES[to],
            "blend":0.5,"file":file,"image":measured}));
    }
    write_contact_sheet(&destination.join("media-transitions.png"), &transitions, 2)?;
    let mut palette_frames = Vec::new();
    let mut palette_report = Vec::new();
    for (scene, look) in [(4, 6), (5, 3), (7, 0)] {
        let image = fixture(
            scene,
            scene,
            1.0,
            look,
            true,
            crate::presets::VisualPalette::MoccaVanilla,
        )?;
        let measured = inspect(&image, "Mocca geometry")?;
        let file = format!("media-mocca-world-{scene}-look-{look}.png");
        write_png(&destination.join(&file), &image, MEDIA_SIZE)?;
        palette_frames.push(media_thumbnail(&image)?);
        palette_report
            .push(serde_json::json!({"scene":scene,"look":look,"file":file,"image":measured}));
    }
    write_contact_sheet(&destination.join("media-mocca.png"), &palette_frames, 2)?;
    let landscape_disconnects =
        verify_landscape_disconnects(&device, &queue, &profile, destination)?;
    let particle_morph_continuity =
        verify_particle_morph_continuity(&mut renderer, &device, &queue, &profile, destination)?;
    let dot_equaliser = verify_dot_equaliser(&device, &queue, &profile, destination)?;
    let dot_world_morph = verify_dot_world_morph(&device, &queue, &profile, destination)?;
    let flight_motion = verify_flight_motion(&device, &queue, &profile, destination, true)?;
    let all_looks = preview_all_looks(&device, &queue, &profile, destination)?;
    let result = serde_json::json!({
        "status":"pass", "app_version":env!("CARGO_PKG_VERSION"),
        "adapter":info.name,"backend":format!("{:?}",info.backend),
        "uniform_bytes":std::mem::size_of::<ShaderUniforms>(),"size":MEDIA_SIZE,
        "input":"synthetic feature snapshots; no audio input opened",
        "scope":"actual geometry prepasses and bounded image statistics; not an aesthetic score, intermediate-float proof, FPS benchmark or live capture test",
        "worlds":world_report,"new_world_pair_differences":pairs,"old_vs_new":old_comparisons,
        "geometry_looks":variant_report,"mixed_prepass_previews":transition_report,"mocca":palette_report,
        "landscape_disconnects":landscape_disconnects,
        "particle_morph_continuity":particle_morph_continuity,
        "dot_equaliser":dot_equaliser,
        "dot_world_morph":dot_world_morph,
        "flight_motion":flight_motion,
        "all_looks":all_looks,
        "measured_history":{"worlds":[SCENES[5], SCENES[6], SCENES[7]],"warm_submissions":HISTORY_WARM_FRAMES,"dt_seconds":0.1,
            "readbacks_during_warmup":0,"final_readback_frames":1,
            "note":"64 advancing synthetic 48-band snapshots plus the final checkpoint, source epoch reset per fixture"}
    });
    fs::write(
        destination.join("media-report.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    println!(
        "PASS actual particle, ODE, spectral-mesh and dot-equaliser previews on {}",
        info.name
    );
    Ok(())
}

/// Freeze real production history, audio, camera time and structural looks.
/// Compare both full compositor endpoints and the actual destination geometry
/// layer. New midpoint support outside BOTH endpoint images rejects a simple
/// image dissolve without mistaking background colour for geometry movement.
fn preview_all_looks(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    destination: &Path,
) -> anyhow::Result<serde_json::Value> {
    let folder = destination.join("all-looks");
    fs::create_dir_all(&folder)?;
    let mut renderer = VisualResources::new(device, HISTORY_FORMAT);
    let mut settings = profile.visual.clone();
    settings.feedback = 0.0;
    settings.complexity = 1.35;
    let full = media_spectrum(18.4);
    let quiet = AudioFeatures::default();
    let bands: Vec<_> = (0..3)
        .map(|band| {
            let mut audio = quiet.clone();
            let (start, end) = [(0, 8), (8, 24), (24, 48)][band];
            audio.spectrum = (0..48)
                .map(|index| {
                    if (start..end).contains(&index) {
                        0.18 + 0.40 * (index as f32 * 1.13).sin().abs()
                    } else {
                        0.0
                    }
                })
                .collect();
            match band {
                0 => audio.bass = 0.65,
                1 => audio.mids = 0.65,
                _ => audio.treble = 0.65,
            }
            audio.energy = 0.45;
            audio
        })
        .collect();
    let mut epoch = 100_000u64;
    let mut draw = |scene: usize,
                    look: usize,
                    time: f32,
                    audio: &AudioFeatures,
                    size: [u32; 2],
                    centre: f32|
     -> anyhow::Result<Vec<u8>> {
        epoch += 1;
        renderer.history = None;
        let mut looks = [[0.0, 0.0, 1.0, 12.75]; WORLD_COUNT];
        looks[scene] = [look as f32, look as f32, 1.0, 12.75];
        let input = |phase, dt| {
            ShaderUniforms::new(
                phase,
                dt,
                scene,
                scene,
                1.0,
                &settings,
                &profile.mappings,
                audio,
            )
            .with_looks(looks)
            .with_audio_state(true, epoch)
            .with_canvas_centre(centre)
        };
        // Seed the same production history for every comparison. Queue
        // updates only; no redundant full-world draws or warmup readbacks.
        if matches!(scene, 5..=7) {
            for frame in 0..64 {
                renderer
                    .raster
                    .prepare(queue, &mut input(12.0 + frame as f32 * 0.1, 0.1));
            }
        }
        render_frame(&mut renderer, device, queue, input(time, 0.0), size)
    };
    let mut report = Vec::new();
    for scene in 0..WORLD_COUNT {
        let mut sheet = Vec::new();
        for (look, look_name) in LOOKS[scene].iter().enumerate() {
            let stem = format!("world-{scene:02}-look-{look:02}");
            let music = draw(scene, look, 18.4, &full, SIZE, 0.0)?;
            let baseline = draw(scene, look, 18.4, &quiet, SIZE, 0.0)?;
            let next = draw(scene, look, 18.4 + 1.0 / 60.0, &full, SIZE, 0.0)?;
            let later = draw(scene, look, 19.15, &full, SIZE, 0.0)?;
            write_png(&folder.join(format!("{stem}.png")), &music, SIZE)?;
            write_png(&folder.join(format!("{stem}-later.png")), &later, SIZE)?;
            let mut band_frames = vec![baseline.clone()];
            let mut response = Vec::new();
            for audio in &bands {
                let image = draw(scene, look, 18.4, audio, SIZE, 0.0)?;
                response.push(difference(&baseline, &image));
                band_frames.push(image);
            }
            write_contact_sheet(&folder.join(format!("{stem}-bands.png")), &band_frames, 4)?;
            let portrait = draw(scene, look, 18.4, &full, [400, 640], 0.0)?;
            write_png(
                &folder.join(format!("{stem}-portrait.png")),
                &portrait,
                [400, 640],
            )?;
            let sidebar = draw(scene, look, 18.4, &full, SIZE, -0.42)?;
            write_png(&folder.join(format!("{stem}-sidebar.png")), &sidebar, SIZE)?;
            let (mean, deviation) = stats(&music);
            anyhow::ensure!(
                mean > 0.01 && deviation.is_finite(),
                "{stem}: empty or invalid encoded image"
            );
            report.push(serde_json::json!({"world":scene,"world_name":SCENES[scene],
                "look":look,"look_name":look_name,"image":format!("all-looks/{stem}.png"),
                "portrait":format!("all-looks/{stem}-portrait.png"),
                "sidebar":format!("all-looks/{stem}-sidebar.png"),
                "bands":format!("all-looks/{stem}-bands.png"),
                "later":format!("all-looks/{stem}-later.png"),
                "mean_rgb":mean,"rgb_deviation":deviation,
                "quiet_to_music_delta":difference(&baseline,&music),
                "isolated_band_deltas":{"bass":response[0],"mids":response[1],"highs":response[2]},
                "one_frame_delta":difference(&music,&next),
                "three_quarter_second_delta":difference(&music,&later),
                "readiness":"requires visual review; encoded RGB statistics do not establish fit or aesthetics"}));
            sheet.push(music);
        }
        write_contact_sheet(
            &folder.join(format!("world-{scene:02}-ten-looks.png")),
            &sheet,
            5,
        )?;
        println!("Rendered every look of {}", SCENES[scene]);
    }
    let result = serde_json::json!({"status":"rendered","looks":report,"count":report.len(),
        "scope":"all configured worlds and looks, synthetic fixed audio, portrait/sidebar/wide images, separated band probes and two motion intervals; no live input or subjective quality acceptance"});
    fs::write(
        folder.join("readiness-matrix.json"),
        serde_json::to_vec_pretty(&result)?,
    )?;
    Ok(result)
}

fn verify_dot_world_morph(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    destination: &Path,
) -> anyhow::Result<serde_json::Value> {
    const TIME: f32 = 18.4;
    const SOURCE_EPOCH: u64 = 96_001;
    const PROGRESS: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
    let mut production = VisualResources::new(device, HISTORY_FORMAT);
    let mut isolated = VisualResources::new(device, HISTORY_FORMAT);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("paired dot geometry finite-support probe"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
            struct Probe { a: vec4<f32>, };
            @group(0) @binding(0) var<uniform> u: Probe;
            @group(0) @binding(3) var geometry: texture_2d_array<f32>;
            @vertex fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
                let positions = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0),
                    vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
                return vec4<f32>(positions[index], 0.0, 1.0);
            }
            @fragment fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
                let raw = textureLoad(geometry, vec2<i32>(position.xy), i32(u.a.w) - 4, 0).rgb;
                // All comparisons must hold, so NaN and infinity both fail.
                // This tests stored half-float samples, not every intermediate.
                let finite = all(raw >= vec3<f32>(-65504.0))
                    && all(raw <= vec3<f32>(65504.0));
                if !finite { return vec4<f32>(1.0, 0.0, 1.0, 0.0); }
                let light = max(raw, vec3<f32>(0.0));
                return vec4<f32>(vec3<f32>(1.0) - exp(-light * 3.0), 1.0);
            }
            "#
            .into(),
        ),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("paired dot geometry probe layout"),
        bind_group_layouts: &[Some(&isolated.bind_group_layout)],
        immediate_size: 0,
    });
    isolated.scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("paired dot geometry isolated compositor"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(HISTORY_FORMAT.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let mut settings = profile.visual.clone();
    settings.feedback = 0.0;
    settings.complexity = 1.35;
    settings.palette = crate::presets::VisualPalette::Cosmic;
    let mut looks = [[0.0, 0.0, 1.0, 12.75]; WORLD_COUNT];
    looks[7] = [1.0, 1.0, 1.0, 12.75]; // Familiar Velvet Steps endpoint.
    let music = media_spectrum(TIME);
    let uniforms = |from, to, amount| {
        ShaderUniforms::new(
            TIME,
            0.0,
            to,
            from,
            amount,
            &settings,
            &profile.mappings,
            &music,
        )
        .with_auto_mix(1.0, 3.0)
        .with_looks(looks)
        .with_audio_state(true, SOURCE_EPOCH)
    };
    // Populate exactly the production CPU filters/history and queue their
    // storage updates. No redundant full-world GPU warm renders/readbacks.
    for frame in 0..64 {
        let time = 12.0 + frame as f32 * 0.1;
        let sample = media_spectrum(time);
        let mut input =
            ShaderUniforms::new(time, 0.1, 7, 7, 1.0, &settings, &profile.mappings, &sample)
                .with_looks(looks)
                .with_audio_state(true, SOURCE_EPOCH);
        production.raster.prepare(queue, &mut input);
        isolated.raster.prepare(queue, &mut input);
    }
    let draw = |renderer: &mut VisualResources, from, to, amount| {
        renderer.history = None;
        render_frame(renderer, device, queue, uniforms(from, to, amount), SIZE)
    };
    let before = draw(&mut production, 4, 4, 1.0)?;
    let after = draw(&mut production, 7, 7, 1.0)?;
    let mut frames = Vec::new();
    let mut directions = Vec::new();
    for (from, to) in [(4, 7), (7, 4)] {
        let start = frames.len();
        for (index, amount) in PROGRESS.into_iter().enumerate() {
            let pixels = draw(&mut production, from, to, amount)?;
            let (mean, variation) = stats(&pixels);
            anyhow::ensure!(
                mean.is_finite() && variation.is_finite() && mean > 0.01 && variation > 0.01,
                "dot world {from}->{to} at {amount}: black or uniform compositor output"
            );
            write_png(
                &destination.join(format!("media-dot-world-{from}-to-{to}-{index}.png")),
                &pixels,
                SIZE,
            )?;
            frames.push(pixels);
        }
        let expected_start = if from == 4 { &before } else { &after };
        let expected_end = if to == 7 { &after } else { &before };
        let start_difference = difference(&frames[start], expected_start);
        let end_difference = difference(&frames[start + 4], expected_end);
        anyhow::ensure!(
            start_difference == 0.0 && end_difference == 0.0,
            "dot world {from}->{to}: original endpoint changed ({start_difference}/{end_difference})"
        );
        let middle_from = difference(&frames[start + 2], expected_start);
        let middle_to = difference(&frames[start + 2], expected_end);
        anyhow::ensure!(
            middle_from > 0.005 && middle_to > 0.005,
            "dot world {from}->{to}: midpoint collapsed to an endpoint"
        );
        directions.push(serde_json::json!({
            "from":from,"to":to,"source_endpoint_rgb_difference":start_difference,
            "destination_endpoint_rgb_difference":end_difference,
            "midpoint_rgb_difference_from_endpoints":[middle_from,middle_to]
        }));
    }
    let reverse_midpoint = difference(&frames[2], &frames[7]);
    anyhow::ensure!(
        reverse_midpoint == 0.0,
        "dot world reverse passage takes a different midpoint ({reverse_midpoint})"
    );
    write_contact_sheet(&destination.join("media-dot-world-morphs.png"), &frames, 5)?;
    let isolated_before = draw(&mut isolated, 4, 4, 1.0)?;
    let isolated_after = draw(&mut isolated, 7, 7, 1.0)?;
    let isolated_middle = draw(&mut isolated, 4, 7, 0.5)?;
    let isolated_reverse = draw(&mut isolated, 7, 4, 0.5)?;
    for (label, pixels) in [
        ("source", &isolated_before),
        ("destination", &isolated_after),
        ("midpoint", &isolated_middle),
        ("reverse midpoint", &isolated_reverse),
    ] {
        anyhow::ensure!(
            pixels.chunks_exact(4).all(|pixel| pixel[3] == 255),
            "dot world {label}: non-finite stored geometry sample"
        );
        anyhow::ensure!(
            pixels
                .chunks_exact(4)
                .any(|pixel| pixel[..3].iter().any(|value| *value > 12)),
            "dot world {label}: black isolated geometry"
        );
    }
    let new_support = isolated_before
        .chunks_exact(4)
        .zip(isolated_after.chunks_exact(4))
        .zip(isolated_middle.chunks_exact(4))
        .filter(|((a, b), middle)| {
            a[..3].iter().all(|value| *value == 0)
                && b[..3].iter().all(|value| *value == 0)
                && middle[..3].iter().any(|value| *value > 12)
        })
        .count();
    anyhow::ensure!(
        new_support > 16,
        "dot morph has no meaningful new geometry outside endpoint support ({new_support} pixels)"
    );
    let plain_dissolve: Vec<u8> = isolated_before
        .iter()
        .zip(&isolated_after)
        .map(|(a, b)| ((u16::from(*a) + u16::from(*b)) / 2) as u8)
        .collect();
    let dissolve_difference = difference(&isolated_middle, &plain_dissolve);
    let reverse_geometry = difference(&isolated_middle, &isolated_reverse);
    anyhow::ensure!(
        reverse_geometry == 0.0,
        "dot morph reverse geometry differs at the same shape amount"
    );
    write_contact_sheet(
        &destination.join("media-dot-world-geometry-proof.png"),
        &[
            isolated_before,
            isolated_middle,
            isolated_after,
            plain_dissolve,
        ],
        4,
    )?;
    // The actual prepass must remain pixel-identical while the compositor
    // rotates its material. Empty geometry pixels also prove that the new
    // offset does not recolour the independently composed environment.
    let mut material_report = Vec::new();
    let mut material_frames = Vec::new();
    for scene in [4, 7] {
        let reference_geometry = draw(&mut isolated, scene, scene, 1.0)?;
        let reference_colour = draw(&mut production, scene, scene, 1.0)?;
        for hue in [0.0, 0.24, 0.64] {
            let input = uniforms(scene, scene, 1.0).with_material_hue(hue);
            production.history = None;
            isolated.history = None;
            let colour = render_frame(&mut production, device, queue, input, SIZE)?;
            let geometry = render_frame(&mut isolated, device, queue, input, SIZE)?;
            anyhow::ensure!(
                geometry == reference_geometry,
                "material hue {hue} changed scene {scene} geometry"
            );
            let colour_change = difference(&reference_colour, &colour);
            anyhow::ensure!(
                hue == 0.0 || colour_change > 0.005,
                "material hue {hue} did not recolour scene {scene}"
            );
            let mut empty = 0usize;
            let mut changed_empty = 0usize;
            for ((raw, before), after) in reference_geometry
                .chunks_exact(4)
                .zip(reference_colour.chunks_exact(4))
                .zip(colour.chunks_exact(4))
            {
                if raw[..3] == [0, 0, 0] {
                    empty += 1;
                    if before != after {
                        changed_empty += 1;
                    }
                }
            }
            // Sub-display-code values in the half-float layer can round to
            // zero in this diagnostic; count them rather than claiming every
            // such stored sample is literally zero.
            anyhow::ensure!(
                empty > 1000 && changed_empty * 100 < empty,
                "material hue changes empty background in scene {scene}"
            );
            write_png(
                &destination.join(format!("media-material-{scene}-{hue:.2}.png")),
                &colour,
                SIZE,
            )?;
            material_frames.push(colour);
            material_report.push(serde_json::json!({"scene":scene,"hue":hue,
                "geometry_pixel_exact":true,"mean_colour_change":colour_change,
                "empty_geometry_pixels":empty,"changed_empty_pixels":changed_empty}));
        }
    }
    write_contact_sheet(
        &destination.join("media-material-colours.png"),
        &material_frames,
        3,
    )?;
    Ok(serde_json::json!({
        "status":"pass","size":SIZE,"fixed_time":TIME,"fixed_source_epoch":SOURCE_EPOCH,
        "progress":PROGRESS,"directions":directions,
        "reverse_midpoint_rgb_difference":reverse_midpoint,
        "reverse_geometry_rgb_difference":reverse_geometry,
        "new_midpoint_pixels_outside_both_endpoint_supports":new_support,
        "minimum_new_support_pixels":16,
        "isolated_midpoint_vs_display_space_dissolve_rgb_difference":dissolve_difference,
        "stored_geometry_finite_samples":"pass",
        "history_preparation_samples":64,"history_preparation_gpu_renders":0,
        "history_preparation_readbacks":0,
        "passage_sheet":"media-dot-world-morphs.png",
        "passage_sheet_rows":["Constellation to Dot Equaliser","Dot Equaliser to Constellation"],
        "geometry_sheet":"media-dot-world-geometry-proof.png",
        "geometry_sheet_columns":["source","actual 3-D midpoint","destination","plain display-space dissolve"],
        "material_colour":material_report,
        "scope":"production endpoint pixels and a diagnostic compositor of the actual target geometry layer; new occupied support rules out plain endpoint-image blending. Fixed synthetic audio/history, finite stored half-float samples only; not every intermediate float, live audio, FPS or perceived smoothness."
    }))
}

/// Dot colour evidence uses the actual 50,000-point prepass and compositor.
/// Fixed synthetic frequency levels make every look/palette directly comparable;
/// these images do not establish live-audio or perceived-motion acceptance.
fn preview_dot_equaliser_material(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    destination: &Path,
) -> anyhow::Result<serde_json::Value> {
    let folder = destination.join("dot-material");
    fs::create_dir_all(&folder)?;
    let mut renderer = VisualResources::new(device, HISTORY_FORMAT);
    let mut settings = profile.visual.clone();
    settings.feedback = 0.0;
    settings.motion = 1.0;
    settings.complexity = 1.35;
    let normal = media_spectrum(18.4);
    let mut loud = normal.clone();
    for level in &mut loud.spectrum {
        *level = (*level * 1.7).min(1.5);
    }
    loud.bass = (loud.bass * 1.4).min(1.0);
    loud.mids = (loud.mids * 1.4).min(1.0);
    loud.treble = (loud.treble * 1.4).min(1.0);
    loud.energy = 1.0;
    let levels = [
        ("silent", AudioFeatures::default()),
        ("normal", normal),
        ("loud", loud),
    ];
    let mut epoch = 98_500u64;
    let mut report = Vec::new();
    for (palette, mix, hue, auto) in [
        ("original", 0.0, 0.0, 0.0),
        ("auto-tint", 0.0, 0.24, 1.0),
        ("mocca", 1.0, 0.24, 1.0),
    ] {
        for (level, audio) in &levels {
            let mut frames = Vec::new();
            for look in 0..10 {
                epoch += 1;
                renderer.history = None;
                let mut looks = [[0.0, 0.0, 1.0, 12.75]; WORLD_COUNT];
                looks[7] = [look as f32, look as f32, 1.0, 12.75];
                let uniform = |time, dt| {
                    ShaderUniforms::new(time, dt, 7, 7, 1.0, &settings, &profile.mappings, audio)
                        .with_looks(looks)
                        .with_audio_state(true, epoch)
                        .with_palette_mix(mix)
                        .with_material_hue(hue)
                        .with_auto_mix(auto, 3.0)
                };
                for frame in 0..64 {
                    renderer
                        .raster
                        .prepare(queue, &mut uniform(12.0 + frame as f32 * 0.1, 0.1));
                }
                let image = render_frame(&mut renderer, device, queue, uniform(18.4, 0.0), SIZE)?;
                write_png(
                    &folder.join(format!("{palette}-{level}-{look:02}.png")),
                    &image,
                    SIZE,
                )?;
                frames.push(image);
            }
            let sheet = format!("{palette}-{level}-ten-looks.png");
            write_contact_sheet(&folder.join(&sheet), &frames, 5)?;
            report.push(serde_json::json!({
                "palette":palette,"level":level,"looks":10,
                "sheet":format!("dot-material/{sheet}")
            }));
        }
    }
    Ok(serde_json::json!({
        "status":"rendered","world":7,"point_count":50_000,"frames":90,
        "fixed_time":18.4,"history_samples":64,"display_history":false,
        "sheets":report,
        "scope":"Production geometry and compositor with synthetic silence, normal and louder frequency input. Original, Auto hue and Mocca materials. Visual evidence only; no live input, motion-smoothness or subjective quality claim."
    }))
}

/// Focused art review of colour travel and intact projected world transitions.
/// Bounded background-only update proof; original broader diagnostics remain.
pub fn preview_background(destination: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: if cfg!(target_os = "macos") {
            wgpu::Backends::METAL
        } else {
            wgpu::Backends::all()
        },
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
    let profile = crate::presets::Profile::astral_bloom();
    let motion = verify_flight_motion(&device, &queue, &profile, destination, false)?;
    let mut renderer = VisualResources::new(&device, HISTORY_FORMAT);
    renderer.specialize(&device, &[0]);
    let audio = media_spectrum(18.4);
    let mut frames = Vec::new();
    for world in [4, 5, 7, 12] {
        let mut settings = profile.visual.clone();
        settings.scene = world;
        settings.feedback = 0.0;
        for time in [12.0, 24.0, 36.0, 48.0] {
            renderer.history = None;
            let input = ShaderUniforms::new(
                time,
                1.0 / 60.0,
                world,
                world,
                1.0,
                &settings,
                &profile.mappings,
                &audio,
            )
            .with_audio_state(true, 1)
            .with_auto_mix(0.75, 1.7);
            frames.push(render_frame(&mut renderer, &device, &queue, input, SIZE)?);
        }
    }
    write_contact_sheet(&destination.join("background-foregrounds.png"), &frames, 4)?;
    fs::write(
        destination.join("background-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "status":"pass","motion":motion,"composite_worlds":[4,5,7,12],"composite_times":[12,24,36,48],
            "scope":"Actual shared colour environment, isolated bass/disconnect and depth-boundary motion; four prepared foreground composites. Synthetic input, not live-music or display-FPS acceptance."
        }))?,
    )?;
    Ok(())
}

pub fn preview_chromatic(destination: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
    fs::write(
        destination.join("chromatic-report.json"),
        "{\"status\":\"incomplete\"}",
    )?;
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
    .context("no GPU adapter for chromatic previews")?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("amara chromatic and intact-world preview"),
        ..Default::default()
    }))?;
    let profile = crate::presets::Profile::astral_bloom();
    let flight = verify_flight_motion(&device, &queue, &profile, destination, true)?;
    let composites = preview_flight_composites(&device, &queue, &profile, destination)?;
    let dot_material = preview_dot_equaliser_material(&device, &queue, &profile, destination)?;
    let mut renderer = VisualResources::new(&device, HISTORY_FORMAT);
    let music = media_spectrum(18.4);
    let centre = verify_canvas_centre(
        &mut renderer,
        &device,
        &queue,
        &profile,
        &music,
        destination,
    )?;
    let mut settings = profile.visual.clone();
    settings.feedback = 0.0;
    settings.motion = 1.0;
    settings.complexity = 1.35;
    let mut epoch = 98_000u64;
    let mut transitions = Vec::new();
    let mut sheets = Vec::new();
    let mut draw =
        |renderer: &mut VisualResources, from: usize, to: usize, amount: f32, offset: f32| {
            epoch += 1;
            renderer.history = None;
            let mut looks = [[0.0, 0.0, 1.0, 12.75]; WORLD_COUNT];
            looks[7] = [1.0, 1.0, 1.0, 12.75];
            let input = |time, dt| {
                ShaderUniforms::new(
                    time,
                    dt,
                    to,
                    from,
                    amount,
                    &settings,
                    &profile.mappings,
                    &music,
                )
                .with_looks(looks)
                .with_auto_mix(1.0, 3.0)
                .with_canvas_centre(offset)
                .with_audio_state(true, epoch)
            };
            for frame in 0..64 {
                renderer
                    .raster
                    .prepare(&queue, &mut input(12.0 + frame as f32 * 0.1, 0.1));
            }
            render_frame(renderer, &device, &queue, input(18.4, 0.0), SIZE)
        };
    for (from, to) in [(1, 7), (7, 6), (6, 9), (2, 7), (4, 7)] {
        for amount in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let image = draw(&mut renderer, from, to, amount, 0.35)?;
            let file = format!("chromatic-morph-{from}-{to}-{amount:.2}.png");
            write_png(&destination.join(&file), &image, SIZE)?;
            sheets.push(image);
        }
        transitions.push(serde_json::json!({"from":SCENES[from], "to":SCENES[to]}));
    }
    write_contact_sheet(
        &destination.join("chromatic-intact-transitions.png"),
        &sheets,
        5,
    )?;

    // Keep the complete production compositor, replacing only its final display
    // output with the already-composited foreground marks. This tests the real
    // transition's pixel support, independently of background colours/exposure.
    let mut isolated = VisualResources::new(&device, HISTORY_FORMAT);
    let production_return = "return vec4<f32>(clamp(colour, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);";
    anyhow::ensure!(
        super::WORLD_SHADER_SOURCE.contains(production_return),
        "foreground probe output hook changed"
    );
    let source = super::WORLD_SHADER_SOURCE.replace(
        production_return,
        "return vec4<f32>(vec3<f32>(1.0) - exp(-radiance.marks), 1.0);",
    );
    // Expose the actual incoming coordinate path after the production morph
    // transport. At fixed time it must match its settled native world exactly,
    // even when a liquid source is being deformed independently alongside it.
    let incoming_source = super::WORLD_SHADER_SOURCE.replace(
        production_return,
        "let incoming = world(incoming_view, u.a.w); return vec4<f32>(vec3<f32>(1.0) - exp(-(incoming.light + incoming.marks) * 2.0), 1.0);",
    );
    let mut incoming = VisualResources::new(&device, HISTORY_FORMAT);
    let incoming_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("native world incoming projection probe"),
        source: wgpu::ShaderSource::Wgsl(incoming_source.into()),
    });
    let incoming_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("native world incoming projection layout"),
        bind_group_layouts: &[Some(&incoming.bind_group_layout)],
        immediate_size: 0,
    });
    incoming.scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("native world incoming projection compositor"),
        layout: Some(&incoming_layout),
        vertex: wgpu::VertexState {
            module: &incoming_module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &incoming_module,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(HISTORY_FORMAT.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let mut native_entry = Vec::new();
    let mut hidden_projections = Vec::new();
    let mut native_centre_registration = Vec::new();
    for (layout, offset) in [("hidden", 0.0), ("right-sidebar", -0.4)] {
        for to in [9, 10] {
            let settled = draw(&mut incoming, to, to, 1.0, offset)?;
            if offset == 0.0 {
                hidden_projections.push(settled.clone());
            } else {
                let hidden = &hidden_projections[to - 9];
                let mut error = 0.0;
                let mut samples = 0;
                for y in 16..SIZE[1] as usize - 16 {
                    for x in 100..SIZE[0] as usize - 20 {
                        let a = (y * SIZE[0] as usize + x) * 4;
                        let b = (y * SIZE[0] as usize + x - 80) * 4;
                        for channel in 0..3 {
                            error += (f64::from(hidden[a + channel])
                                - f64::from(settled[b + channel]))
                            .abs();
                            samples += 1;
                        }
                    }
                }
                error /= f64::from(samples);
                anyhow::ensure!(
                    error < 1.0,
                    "native world {to} sidebar translation did not follow the stage centre ({error})"
                );
                native_centre_registration.push(serde_json::json!({"world":SCENES[to],"expected_horizontal_translation_pixels":-80,"registered_rgb_difference":error}));
            }
            let mut native_frames = Vec::new();
            let mut projection_frames = Vec::new();
            for from in [1, 2, 7] {
                let mut differences = Vec::new();
                for amount in [0.20, 0.50, 1.0] {
                    let image = draw(&mut renderer, from, to, amount, offset)?;
                    let file = format!("native-entry-{layout}-{from}-{to}-{amount:.2}.png");
                    write_png(&destination.join(&file), &image, SIZE)?;
                    native_frames.push(image);
                    let projection = draw(&mut incoming, from, to, amount, offset)?;
                    let change = difference(&settled, &projection);
                    anyhow::ensure!(
                        change == 0.0,
                        "{layout} world {from}->{to} amount {amount}: incoming camera or terrain shifted ({change})"
                    );
                    differences.push(change);
                    projection_frames.push(projection);
                }
                native_entry.push(serde_json::json!({
                    "layout":layout,"from":SCENES[from],"to":SCENES[to],
                    "progress":[0.20,0.50,1.0],"incoming_projection_rgb_differences":differences,
                    "canvas_centre_uniform":offset,"stage_centre_pixel_x":320.0+offset*200.0
                }));
            }
            write_contact_sheet(
                &destination.join(format!("native-entry-{layout}-{to}.png")),
                &native_frames,
                3,
            )?;
            write_contact_sheet(
                &destination.join(format!("native-projection-{layout}-{to}.png")),
                &projection_frames,
                3,
            )?;
        }
    }
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("production intact foreground probe"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("intact foreground probe layout"),
        bind_group_layouts: &[Some(&isolated.bind_group_layout)],
        immediate_size: 0,
    });
    isolated.scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("production intact foreground compositor"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(HISTORY_FORMAT.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let mut support = Vec::new();
    let mut proof = Vec::new();
    for (from, to) in [(7, 6), (4, 6), (6, 5)] {
        let before = draw(&mut isolated, from, from, 1.0, 0.35)?;
        let after = draw(&mut isolated, to, to, 1.0, 0.35)?;
        let middle = draw(&mut isolated, from, to, 0.5, 0.35)?;
        let escaped = middle
            .chunks_exact(4)
            .zip(before.chunks_exact(4))
            .zip(after.chunks_exact(4))
            .filter(|((mid, first), last)| {
                (0..3).any(|c| u16::from(mid[c]) > u16::from(first[c].max(last[c])) + 2)
            })
            .count();
        anyhow::ensure!(
            escaped == 0,
            "rigid world {from}->{to} moved {escaped} foreground pixels outside its endpoint support"
        );
        proof.extend([before, middle, after]);
        support.push(serde_json::json!({"from":SCENES[from], "to":SCENES[to], "escaped_foreground_pixels":escaped}));
    }
    write_contact_sheet(
        &destination.join("chromatic-geometry-support.png"),
        &proof,
        3,
    )?;
    fs::write(
        destination.join("chromatic-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "status":"pass", "gpu":adapter.get_info().name, "flight":flight, "centre":centre,
            "background_composites":composites,
            "dot_material":dot_material,
            "transition_rows":transitions, "columns":[0.0,0.25,0.5,0.75,1.0], "geometry_support":support,
            "native_world_entry":native_entry,
            "native_centre_registration":native_centre_registration,
            "scope":"Actual production shader, synthetic fixed music/phase, cold display history, prepared spectral history. Intact raster support excludes the intentionally geometric dot-to-dot correspondence. Not live audio or perceived-forward-motion acceptance."
        }))?,
    )?;
    println!("PASS focused chromatic travel and intact-world transitions");
    Ok(())
}

/// Directly expose the production chromatic environment for a short synthetic
/// time series. This isolates travelling colour strata from foreground animation.
fn verify_flight_motion(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    destination: &Path,
    write_frames: bool,
) -> anyhow::Result<serde_json::Value> {
    let mut renderer = VisualResources::new(device, HISTORY_FORMAT);
    let source = format!(
        "{}\n{}",
        super::WORLD_SHADER_SOURCE,
        r#"
        @fragment fn fs_flight_probe(in: VertexOut) -> @location(0) vec4<f32> {
            let aspect = max(u.a.x, 1.0) / max(u.a.y, 1.0);
            let uv = (in.uv * 2.0 - 1.0) * vec2<f32>(aspect, 1.0)
                - vec2<f32>(u.g.w, 0.0);
            return vec4<f32>(1.0 - exp(-chromatic_space(uv) * 2.0), 1.0);
        }
    "#
    );
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("production chromatic travel probe"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("flight motion probe layout"),
        bind_group_layouts: &[Some(&renderer.bind_group_layout)],
        immediate_size: 0,
    });
    renderer.scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("isolated flight motion"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_flight_probe"),
            compilation_options: Default::default(),
            targets: &[Some(HISTORY_FORMAT.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let mut settings = profile.visual.clone();
    settings.feedback = 0.0;
    let audio = media_spectrum(18.4);
    let quiet = AudioFeatures::default();
    let mut draw = |time: f32, mappings: &AudioMappings, features: &AudioFeatures, fresh: bool| {
        renderer.history = None;
        render_frame(
            &mut renderer,
            device,
            queue,
            ShaderUniforms::new(time, 0.0, 7, 7, 1.0, &settings, mappings, features)
                .with_audio_state(fresh, 97_001),
            SIZE,
        )
    };
    // A two-second sample could prove colour change while missing the broader
    // turn. These two twelve-second passages expose opposite parts of the
    // continuous bank/curl journey, using fixed audio to isolate the scenery.
    let mut passage_report = Vec::new();
    for (start, prefix) in [(12.0, "media-flight"), (36.0, "media-swirl")] {
        let mut sequence = Vec::new();
        let mut first = None;
        let mut last = Vec::new();
        for frame in 0..144 {
            let image = draw(start + frame as f32 / 12.0, &profile.mappings, &audio, true)?;
            if write_frames {
                write_png(
                    &destination.join(format!("{prefix}-frame-{frame:03}.png")),
                    &image,
                    SIZE,
                )?;
            }
            if frame == 0 {
                first = Some(image.clone());
            }
            if frame % 24 == 0 {
                sequence.push(image.clone());
            }
            last = image;
        }
        let motion = difference(first.as_ref().expect("first passage frame"), &last);
        anyhow::ensure!(motion > 0.1, "chromatic environment did not move visibly");
        let sheet = format!("{prefix}-time-series.png");
        write_contact_sheet(&destination.join(&sheet), &sequence, 3)?;
        passage_report.push(serde_json::json!({"start_time":start,
            "duration_seconds":12,"frames":144,"frame_interval_seconds":1.0/12.0,
            "first_to_last_rgb_motion":motion,"sheet":sheet,"frame_prefix":prefix}));
    }
    let mut journey_frames = Vec::new();
    for time in [0.0, 12.0, 20.0, 36.0, 52.0, 72.0] {
        journey_frames.push(draw(time, &profile.mappings, &audio, true)?);
    }
    write_contact_sheet(
        &destination.join("media-flight-direction-journey.png"),
        &journey_frames,
        3,
    )?;
    let mut wraps = Vec::new();
    let depth_period = 4.8_f32 / 2.3;
    for crossing in [
        depth_period,
        depth_period * 2.0,
        12.0,
        20.0,
        36.0,
        52.0,
        depth_period * 2096.0,
    ] {
        let before = draw(crossing - 1.0 / 60.0, &profile.mappings, &audio, true)?;
        let at = draw(crossing, &profile.mappings, &audio, true)?;
        let after = draw(crossing + 1.0 / 60.0, &profile.mappings, &audio, true)?;
        let entering = difference(&before, &at);
        let leaving = difference(&at, &after);
        anyhow::ensure!(
            entering < 6.0 && leaving < 6.0,
            "chromatic travel has a large frame step at {crossing}: {entering}/{leaving}"
        );
        wraps.push(serde_json::json!({"time":crossing,
            "before_to_boundary":entering,"boundary_to_after":leaving}));
    }
    let zero = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        ..profile.mappings.clone()
    };
    let disconnected = draw(12.0, &zero, &audio, true)?;
    let quiet_background = draw(12.0, &zero, &quiet, true)?;
    let mut low_only = quiet.clone();
    low_only.bass = 0.85;
    let quiet_mapped = draw(12.0, &profile.mappings, &quiet, true)?;
    let bass_mapped = draw(12.0, &profile.mappings, &low_only, true)?;
    let stale_mapped = draw(12.0, &profile.mappings, &audio, false)?;
    anyhow::ensure!(
        stale_mapped == quiet_mapped,
        "stale background audio differs from quiet"
    );
    let bass_difference = difference(&quiet_mapped, &bass_mapped);
    anyhow::ensure!(bass_difference > 0.10, "background bass route is too quiet");
    write_contact_sheet(
        &destination.join("background-quiet-bass.png"),
        &[quiet_mapped, bass_mapped],
        2,
    )?;
    anyhow::ensure!(
        disconnected == quiet_background,
        "chromatic background responds to disconnected audio bands"
    );
    anyhow::ensure!(
        disconnected
            .chunks_exact(4)
            .any(|pixel| pixel[..3] != [0, 0, 0]),
        "quiet chromatic environment unexpectedly disappeared"
    );
    // A bounded actual rendered clip makes the continuous curl and the slow
    // quiet→bass→quiet envelope reviewable without a custom player/frame dump.
    let mut video = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-v",
            "error",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            "640x400",
            "-framerate",
            "30",
            "-i",
            "pipe:0",
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-crf",
            "20",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(destination.join("background-bass-flight-12s.mp4"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok();
    let ease = |t: f32| {
        let x = t.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let mut pulse_frames = Vec::new();
    let mut previous = Vec::new();
    let mut maximum_pulse_step = 0.0_f64;
    let mut maximum_white_fraction = 0.0_f64;
    for frame in 0..=360 {
        let elapsed = frame as f32 / 30.0;
        let mut phrase = quiet.clone();
        phrase.bass = 0.95 * ease((elapsed - 2.0) / 3.0) * (1.0 - ease((elapsed - 7.0) / 3.0));
        let pixels = draw(12.0 + elapsed, &profile.mappings, &phrase, true)?;
        if !previous.is_empty() {
            maximum_pulse_step = maximum_pulse_step.max(difference(&previous, &pixels));
        }
        let white = pixels
            .chunks_exact(4)
            .filter(|p| p[0] > 245 && p[1] > 245 && p[2] > 245)
            .count();
        maximum_white_fraction =
            maximum_white_fraction.max(white as f64 / f64::from(SIZE[0] * SIZE[1]));
        if frame % 60 == 0 {
            pulse_frames.push(pixels.clone());
        }
        if let Some(encoder) = &mut video
            && let Some(stdin) = &mut encoder.stdin
        {
            use std::io::Write as _;
            if stdin.write_all(&pixels).is_err() {
                encoder.stdin.take();
            }
        }
        previous = pixels;
    }
    let video_encoded = video.as_mut().is_some_and(|encoder| {
        encoder.stdin.take();
        encoder.wait().is_ok_and(|status| status.success())
    });
    anyhow::ensure!(
        maximum_pulse_step < 6.0 && maximum_white_fraction < 0.005,
        "background bass passage jumps or clips white"
    );
    write_contact_sheet(
        &destination.join("background-bass-flight-sequence.png"),
        &pulse_frames,
        4,
    )?;
    Ok(
        serde_json::json!({"status":"pass","passages":passage_report,
        "depth_wrap_steps":wraps,"depth_period_seconds":depth_period,
        "zero_mappings_audio_isolation_pixel_exact":true,"quiet_environment_retained":true,
        "bass_only_rgb_difference":bass_difference,"individual_frames_written":write_frames,
        "stale_audio_isolation_pixel_exact":true,
        "bass_flight_clip":{"seconds":12,"frames":361,"fps":30,"silent_video_encoded":video_encoded,
            "input":"quiet0–2s; smooth bass rise2–5s; hold5–7s; smooth fall7–10s; quiet10–12s",
            "maximum_adjacent_rgb_difference":maximum_pulse_step,"maximum_white_fraction":maximum_white_fraction},
        "direction_journey_sheet":"media-flight-direction-journey.png",
        "direction_journey_times":[0.0,12.0,20.0,36.0,52.0,72.0],
        "scope":"production chromatic_space isolated from foreground, fixed synthetic sound; twelve-second image sequences for human depth/curl review, continuous clock/depth-wrap and audio-disconnect checks. Image differences alone do not prove approaching motion, native FPS or live-music response."}),
    )
}

/// Review the very same scenery through the app's normal foreground masking,
/// exposure and world-specific attenuation. An isolated backdrop alone can look
/// convincing while vanishing behind the actual dot or line sculpture.
fn preview_flight_composites(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    destination: &Path,
) -> anyhow::Result<serde_json::Value> {
    let mut renderer = VisualResources::new(device, HISTORY_FORMAT);
    let mut settings = profile.visual.clone();
    settings.feedback = 0.0;
    settings.motion = 1.0;
    settings.complexity = 1.35;
    let audio = media_spectrum(18.4);
    let times = [12.0, 18.0, 24.0, 36.0, 42.0, 48.0];
    let mut frames = Vec::new();
    let mut rows = Vec::new();
    for world in [4, 5, 6, 7] {
        let mut looks = [[0.0, 0.0, 1.0, 12.75]; WORLD_COUNT];
        looks[4] = [1.0, 1.0, 1.0, 12.75];
        looks[5] = [2.0, 2.0, 1.0, 12.75];
        looks[7] = [1.0, 1.0, 1.0, 12.75];
        let uniform = |time, dt| {
            ShaderUniforms::new(
                time,
                dt,
                world,
                world,
                1.0,
                &settings,
                &profile.mappings,
                &audio,
            )
            .with_looks(looks)
            .with_audio_state(true, 97_100 + world as u64)
        };
        for frame in 0..64 {
            renderer
                .raster
                .prepare(queue, &mut uniform(5.6 + frame as f32 * 0.1, 0.1));
        }
        for (index, time) in times.into_iter().enumerate() {
            renderer.history = None;
            let image = render_frame(&mut renderer, device, queue, uniform(time, 0.0), SIZE)?;
            write_png(
                &destination.join(format!("flight-composite-world-{world}-{index}.png")),
                &image,
                SIZE,
            )?;
            frames.push(image);
        }
        rows.push(SCENES[world]);
    }
    write_contact_sheet(
        &destination.join("flight-foreground-composites.png"),
        &frames,
        times.len(),
    )?;
    let colour_link = verify_environment_colour_link(device, queue, profile, destination)?;
    Ok(
        serde_json::json!({"sheet":"flight-foreground-composites.png","rows":rows,
        "times":times,"colour_link":colour_link,
        "input":"fixed synthetic spectrum and real prepared frequency history",
        "scope":"full production composite, unchanged world attenuation/exposure, no display history; visual contrast review rather than a foreground-isolation or live-audio test"}),
    )
}

/// Compare the complete compositor against the identical shader with only the
/// reflected-colour branch disabled. Auto, palette, audio and geometry stay the
/// same on both sides; disabling Auto itself would also change other effects.
fn verify_environment_colour_link(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    destination: &Path,
) -> anyhow::Result<serde_json::Value> {
    let mut linked = VisualResources::new(device, HISTORY_FORMAT);
    let mut original = VisualResources::new(device, HISTORY_FORMAT);
    let marker = "if environment_gain > 0.0001 && u.i.z > 0.0 {";
    anyhow::ensure!(
        super::WORLD_SHADER_SOURCE.matches(marker).count() == 1,
        "environment colour-link diagnostic marker changed"
    );
    // Retain the dynamic branch in both programs. Constant-folding it away
    // can change Metal's upstream half-float rounding in otherwise equal ink.
    // Auto is bounded to [0, 1], so the reversed guard disables only the link.
    let reference_guard = "if environment_gain > 0.0001 && u.i.z < 0.0 {";
    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("environment reflection disabled reference"),
        source: wgpu::ShaderSource::Wgsl(
            super::WORLD_SHADER_SOURCE
                .replace(marker, reference_guard)
                .into(),
        ),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("environment reflection reference layout"),
        bind_group_layouts: &[Some(&original.bind_group_layout)],
        immediate_size: 0,
    });
    original.scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("original unlinked foreground compositor"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs_main"),
            compilation_options: Default::default(),
            targets: &[Some(HISTORY_FORMAT.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let mut settings = profile.visual.clone();
    settings.feedback = 0.0;
    settings.complexity = 1.35;
    let audio = media_spectrum(18.4);
    let mut epoch = 97_800u64;
    let mut pair = |world: usize, auto: f32, palette: f32, background: [f32; 4]| {
        epoch += 1;
        let mut looks = [[0.0, 0.0, 1.0, 12.75]; WORLD_COUNT];
        looks[4] = [1.0, 1.0, 1.0, 12.75];
        looks[5] = [2.0, 2.0, 1.0, 12.75];
        looks[7] = [1.0, 1.0, 1.0, 12.75];
        let input = |time, dt| {
            ShaderUniforms::new(
                time,
                dt,
                world,
                world,
                1.0,
                &settings,
                &profile.mappings,
                &audio,
            )
            .with_looks(looks)
            .with_audio_state(true, epoch)
            .with_auto_mix(auto, 0.0)
            .with_material_hue(0.21)
            .with_palette_mix(palette)
            .with_background(background)
        };
        for renderer in [&mut original, &mut linked] {
            renderer.history = None;
            for frame in 0..64 {
                renderer
                    .raster
                    .prepare(queue, &mut input(12.0 + frame as f32 * 0.1, 0.1));
            }
        }
        Ok::<_, anyhow::Error>([
            render_frame(&mut original, device, queue, input(18.4, 0.0), SIZE)?,
            render_frame(&mut linked, device, queue, input(18.4, 0.0), SIZE)?,
        ])
    };
    let mut frames = Vec::new();
    let mut rows = Vec::new();
    for world in [4, 5, 6, 7] {
        for (name, palette, background) in [
            ("cosmic", 0.0, [0.0; 4]),
            ("selected background", 0.0, [0.12, 0.75, 0.60, 0.85]),
            ("mocca vanilla", 1.0, [0.0; 4]),
        ] {
            let images = pair(world, 1.0, palette, background)?;
            let delta = difference(&images[0], &images[1]);
            anyhow::ensure!(
                delta < 2.5,
                "foreground reflection is too strong in world {world}"
            );
            let luminance_delta = images[0]
                .chunks_exact(4)
                .zip(images[1].chunks_exact(4))
                .map(|(before, after)| (display_luma(before) - display_luma(after)).abs())
                .sum::<f64>()
                / (SIZE[0] * SIZE[1]) as f64;
            rows.push(serde_json::json!({"world":SCENES[world],"palette":name,
                "mean_rgb_difference":delta,"mean_display_luma_difference":luminance_delta}));
            frames.extend(images);
        }
    }
    write_contact_sheet(
        &destination.join("environment-colour-link-pairs.png"),
        &frames,
        6,
    )?;
    let mut gates = Vec::new();
    for (world, scene) in SCENES.iter().enumerate() {
        let manual = pair(world, 0.0, 0.0, [0.12, 0.75, 0.60, 0.85])?;
        let delta = difference(&manual[0], &manual[1]);
        if delta != 0.0 {
            write_png(
                &destination.join(format!("manual-reference-{world}.png")),
                &manual[0],
                SIZE,
            )?;
            write_png(
                &destination.join(format!("manual-linked-{world}.png")),
                &manual[1],
                SIZE,
            )?;
        }
        anyhow::ensure!(
            delta == 0.0,
            "manual colour changed in world {world}: mean RGB delta {delta}"
        );
        gates.push(serde_json::json!({"world":scene,"manual_difference":delta}));
    }
    for world in [0, 8, 9, 10] {
        let native = pair(world, 1.0, 0.0, [0.0; 4])?;
        anyhow::ensure!(
            difference(&native[0], &native[1]) == 0.0,
            "native environment changed in world {world}"
        );
    }
    let disconnected = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let quiet = AudioFeatures::default();
    let mut disconnected_images = Vec::new();
    for features in [&audio, &quiet] {
        linked.history = None;
        epoch += 1;
        disconnected_images.push(render_frame(
            &mut linked,
            device,
            queue,
            ShaderUniforms::new(18.4, 0.0, 7, 7, 1.0, &settings, &disconnected, features)
                .with_auto_mix(1.0, 0.0)
                .with_material_hue(0.21)
                .with_audio_state(true, epoch),
            SIZE,
        )?);
    }
    let zero_audio_delta = difference(&disconnected_images[0], &disconnected_images[1]);
    anyhow::ensure!(
        zero_audio_delta == 0.0,
        "colour link bypassed disconnected audio mappings"
    );
    Ok(
        serde_json::json!({"sheet":"environment-colour-link-pairs.png",
        "rows":["Particle Constellation","Strange Attractor","Spectral Landscape","Dot Equaliser"],
        "columns":["cosmic before","cosmic linked","selected background before",
            "selected background linked","mocca before","mocca linked"],
        "pairs":rows,"manual_gates":gates,"native_optouts_exact":[0,8,9,10],
        "disconnected_audio_difference":zero_audio_delta,
        "scope":"same production compositor, synthetic spectrum and geometry history; only reflected-colour branch disabled in reference. Display luma is a tonemapped contrast proxy, not a linear radiometric measurement. No live-music or FPS claim."}),
    )
}

fn verify_particle_morph_continuity(
    renderer: &mut VisualResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    destination: &Path,
) -> anyhow::Result<serde_json::Value> {
    const TIME: f32 = 18.4;
    const PHASE: f32 = 12.75;
    const TINY_AMOUNT: f32 = 0.0001;
    const MAX_TINY_RGB_STEP: f64 = 3.0;
    let mut settings = profile.visual.clone();
    settings.feedback = 0.0;
    settings.complexity = 1.35;
    settings.palette = crate::presets::VisualPalette::Cosmic;
    let music = media_spectrum(TIME);
    let quiet = AudioFeatures::default();
    let zero = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let mut sheet = Vec::new();
    let mut probes = Vec::new();
    for (fixture, (from, to)) in [(0, 2), (2, 0), (1, 6), (6, 1)].into_iter().enumerate() {
        let epoch = 92_000 + fixture as u64;
        let mut render = |from: usize,
                          to: usize,
                          amount: f32,
                          mappings: &AudioMappings,
                          audio: &AudioFeatures| {
            let mut looks = [[0.0, 0.0, 1.0, PHASE]; WORLD_COUNT];
            looks[4] = [from as f32, to as f32, amount, PHASE];
            let uniforms =
                ShaderUniforms::new(TIME, 1.0 / 60.0, 4, 4, 1.0, &settings, mappings, audio)
                    .with_choreography(Choreography::default())
                    .with_looks(looks)
                    .with_audio_state(true, epoch);
            // Only the look endpoints/amount and deliberate audio-disconnect
            // inputs vary. No history, controller phase or time can hide a jump.
            renderer.history = None;
            render_frame(renderer, device, queue, uniforms, SIZE)
        };
        let before = render(from, from, 1.0, &profile.mappings, &music)?;
        let after = render(to, to, 1.0, &profile.mappings, &music)?;
        let at_zero = render(from, to, 0.0, &profile.mappings, &music)?;
        let at_one = render(from, to, 1.0, &profile.mappings, &music)?;
        let first = difference(&before, &at_zero);
        let last = difference(&after, &at_one);
        anyhow::ensure!(
            first == 0.0 && last == 0.0,
            "particle look {from}->{to}: endpoint FFT/geometry changed ({first}/{last})"
        );
        let middle = render(from, to, 0.5, &profile.mappings, &music)?;
        let middle_from = difference(&before, &middle);
        let middle_to = difference(&after, &middle);
        anyhow::ensure!(
            middle_from > 0.005 && middle_to > 0.005,
            "particle look {from}->{to}: midpoint collapsed to an endpoint"
        );
        let tiny = render(from, to, TINY_AMOUNT, &profile.mappings, &music)?;
        let tiny_change = difference(&before, &tiny);
        anyhow::ensure!(
            tiny_change <= MAX_TINY_RGB_STEP,
            "particle look {from}->{to}: tiny look amount causes a jump ({tiny_change})"
        );
        let off_quiet = render(from, to, 0.5, &zero, &quiet)?;
        let off_music = render(from, to, 0.5, &zero, &music)?;
        let disconnected = difference(&off_quiet, &off_music);
        anyhow::ensure!(
            disconnected == 0.0,
            "particle look {from}->{to}: zero mappings leak audio ({disconnected})"
        );
        sheet.extend([before, tiny, middle, after]);
        probes.push(serde_json::json!({
            "from":from,"from_name":LOOKS[4][from],"to":to,"to_name":LOOKS[4][to],
            "source_epoch":epoch,"zero_endpoint_rgb_difference":first,
            "one_endpoint_rgb_difference":last,"tiny_amount_rgb_difference":tiny_change,
            "middle_rgb_difference_from_endpoints":[middle_from,middle_to],
            "all_zero_mappings_rgb_difference":disconnected
        }));
    }
    write_contact_sheet(&destination.join("media-particle-morphs.png"), &sheet, 4)?;
    Ok(serde_json::json!({
        "status":"pass","size":SIZE,"time":TIME,"phase":PHASE,
        "tiny_amount":TINY_AMOUNT,"maximum_tiny_mean_rgb_step":MAX_TINY_RGB_STEP,
        "sheet_columns":["fixed source","tiny amount","midpoint","fixed destination"],
        "scope":"fixed synthetic audio/phase and default fixed choreography/inactive material responses; exact endpoints, bounded tiny-step output and all-zero mapping isolation only; no FPS or aesthetic claim",
        "pairs":probes
    }))
}

fn verify_landscape_disconnects(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    destination: &Path,
) -> anyhow::Result<serde_json::Value> {
    const SIZE: [u32; 2] = [1280, 800];
    const SOURCE_EPOCH: u64 = 90_001;
    // Disconnect assertions concern the measured terrain, not the shared
    // chromatic environment. Read production raster layer 2 directly so its
    // background cannot respond to the intentionally retained audio snapshot.
    let mut isolated = VisualResources::new(device, HISTORY_FORMAT);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("isolated landscape geometry readback"),
        source: wgpu::ShaderSource::Wgsl(LANDSCAPE_PROBE_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("isolated landscape layout"),
        bind_group_layouts: &[Some(&isolated.bind_group_layout)],
        immediate_size: 0,
    });
    isolated.scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("isolated landscape compositor"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(HISTORY_FORMAT.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let renderer = &mut isolated;
    let mut settings = profile.visual.clone();
    settings.feedback = 0.0;
    settings.complexity = 1.35;
    settings.palette = crate::presets::VisualPalette::Cosmic;
    let zero = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let quiet = AudioFeatures::default();
    let music = media_spectrum(18.4);
    let uniforms = |time, mappings: &AudioMappings, audio: &AudioFeatures, fresh| {
        ShaderUniforms::new(time, 0.1, 6, 6, 1.0, &settings, mappings, audio)
            .with_audio_state(fresh, SOURCE_EPOCH)
    };
    let warm = |renderer: &mut VisualResources| -> anyhow::Result<Vec<u8>> {
        renderer.history = None;
        for frame in 0..64 {
            let time = 12.0 + frame as f32 * 0.1;
            let mut encoder = device.create_command_encoder(&Default::default());
            renderer.render(
                device,
                queue,
                &mut encoder,
                uniforms(time, &profile.mappings, &media_spectrum(time), true),
                SIZE,
            );
            queue.submit([encoder.finish()]);
        }
        render_frame(
            renderer,
            device,
            queue,
            uniforms(18.4, &profile.mappings, &music, true),
            SIZE,
        )
    };
    let fresh_before_mapping = warm(renderer)?;
    // Mappings change without a source epoch change: both retained terrain and
    // its current FFT front must stop responding, not just the scalar meters.
    renderer.history = None;
    let mapped_off = render_frame(
        renderer,
        device,
        queue,
        uniforms(18.4, &zero, &music, true),
        SIZE,
    )?;
    renderer.history = None;
    let off_quiet = render_frame(
        renderer,
        device,
        queue,
        uniforms(18.4, &zero, &quiet, true),
        SIZE,
    )?;
    let zero_difference = difference(&mapped_off, &off_quiet);
    anyhow::ensure!(
        zero_difference == 0.0,
        "disconnected landscape retains an audio response"
    );
    let mapping_removed = difference(&fresh_before_mapping, &mapped_off);
    anyhow::ensure!(
        mapping_removed > 0.005,
        "landscape mapping fixture had no active audio geometry"
    );

    let fresh_before_stale = warm(renderer)?;
    renderer.history = None;
    // Intentionally retain the nonzero feature snapshot and the SAME epoch and
    // phase stamp. Freshness alone must suppress history AND the current front.
    let stale = render_frame(
        renderer,
        device,
        queue,
        uniforms(18.4, &profile.mappings, &music, false),
        SIZE,
    )?;
    renderer.history = None;
    let stale_quiet = render_frame(
        renderer,
        device,
        queue,
        uniforms(18.4, &profile.mappings, &quiet, false),
        SIZE,
    )?;
    let stale_difference = difference(&stale, &stale_quiet);
    anyhow::ensure!(
        stale_difference == 0.0,
        "stale landscape still uses retained or current audio"
    );
    let freshness_removed = difference(&fresh_before_stale, &stale);
    anyhow::ensure!(
        freshness_removed > 0.005,
        "landscape freshness fixture had no active audio geometry"
    );
    write_png(
        &destination.join("media-landscape-mappings-off.png"),
        &mapped_off,
        SIZE,
    )?;
    write_png(&destination.join("media-landscape-stale.png"), &stale, SIZE)?;
    write_contact_sheet(
        &destination.join("media-landscape-disconnects.png"),
        &[
            media_thumbnail(&fresh_before_mapping)?,
            media_thumbnail(&mapped_off)?,
            media_thumbnail(&fresh_before_stale)?,
            media_thumbnail(&stale)?,
        ],
        2,
    )?;
    Ok(serde_json::json!({
        "source_epoch":SOURCE_EPOCH,"same_epoch_and_phase_for_each_disconnect":true,
        "warm_submissions_per_probe":64,"dt_seconds":0.1,"warm_readbacks":0,
        "zero_mapping_rgb_difference_vs_quiet":zero_difference,
        "stale_with_retained_nonzero_features_rgb_difference_vs_quiet":stale_difference,
        "active_to_mappings_off_rgb_difference":mapping_removed,
        "active_to_stale_rgb_difference":freshness_removed,
        "scope":"production landscape raster layer 2 isolated by a diagnostic compositor; exact terrain history/current-front disconnects with synthetic features only, excluding shared background colour"
    }))
}

/// Frequency-position, warmed/startup response and disconnect checks for dots.
/// Low/high fixtures keep the scalar meters, time, look and camera identical:
/// their difference must come from the measured 48-band contour, not loudness.
fn verify_dot_equaliser(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    destination: &Path,
) -> anyhow::Result<serde_json::Value> {
    const SCENE: usize = 7;
    const SIZE: [u32; 2] = [1280, 800];
    // Reuse the actual production geometry pass, with a diagnostic compositor
    // that reads only its layer. Shared background FFT colour must not make a
    // broken dot equaliser pass frequency or disconnect assertions.
    let mut isolated = VisualResources::new(device, HISTORY_FORMAT);
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("isolated dot equaliser geometry readback"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
            @group(0) @binding(3) var geometry: texture_2d_array<f32>;
            @vertex fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
                let positions = array<vec2<f32>, 3>(vec2<f32>(-1.0, -1.0),
                    vec2<f32>(3.0, -1.0), vec2<f32>(-1.0, 3.0));
                return vec4<f32>(positions[index], 0.0, 1.0);
            }
            @fragment fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
                let light = max(textureLoad(geometry, vec2<i32>(position.xy), 3, 0).rgb,
                    vec3<f32>(0.0));
                return vec4<f32>(vec3<f32>(1.0) - exp(-light * 3.0), 1.0);
            }
            "#
            .into(),
        ),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("isolated geometry layout"),
        bind_group_layouts: &[Some(&isolated.bind_group_layout)],
        immediate_size: 0,
    });
    isolated.scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("isolated dot equaliser compositor"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(HISTORY_FORMAT.into())],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });
    let renderer = &mut isolated;
    let mut settings = profile.visual.clone();
    settings.feedback = 0.0;
    settings.complexity = 1.35;
    settings.palette = crate::presets::VisualPalette::Cosmic;
    let full = AudioMappings {
        bass_to_pulse: 1.0,
        mids_to_geometry: 1.0,
        treble_to_sparkle: 1.0,
        beat_to_flash: 1.0,
    };
    let zero = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let low = AudioFeatures {
        bass: 0.6,
        mids: 0.6,
        treble: 0.6,
        energy: 0.7,
        spectrum: (0..48)
            .map(|band| 0.9 * (-((band as f32 - 8.0) / 3.0).powi(2)).exp())
            .collect(),
        ..Default::default()
    };
    let mut high = low.clone();
    high.spectrum.reverse();
    let quiet = AudioFeatures::default();
    let mut epoch = 95_000_u64;
    let mut fixture = |warm_audio: &AudioFeatures,
                       final_audio: &AudioFeatures,
                       mappings: &AudioMappings,
                       fresh: bool,
                       warm: bool|
     -> anyhow::Result<Vec<u8>> {
        epoch += 1;
        renderer.history = None;
        let uniforms = |time, audio: &AudioFeatures, maps: &AudioMappings, active| {
            ShaderUniforms::new(time, 0.1, SCENE, SCENE, 1.0, &settings, maps, audio)
                .with_audio_state(active, epoch)
        };
        if warm {
            for frame in 0..64 {
                let time = 12.0 + frame as f32 * 0.1;
                let mut audio = warm_audio.clone();
                // An explicitly synthetic, deterministic amplitude contour;
                // every history row is actually submitted, never fabricated.
                let level = 0.45 + frame as f32 / 63.0 * 0.55;
                for band in &mut audio.spectrum {
                    *band *= level;
                }
                let mut encoder = device.create_command_encoder(&Default::default());
                renderer.render(
                    device,
                    queue,
                    &mut encoder,
                    uniforms(time, &audio, &full, true),
                    SIZE,
                );
                queue.submit([encoder.finish()]);
            }
        }
        // Disconnects deliberately keep the populated source epoch. Clearing
        // feedback does not clear the measured raster history under test.
        renderer.history = None;
        render_frame(
            renderer,
            device,
            queue,
            uniforms(18.4, final_audio, mappings, fresh),
            SIZE,
        )
    };
    let low_frame = fixture(&low, &low, &full, true, true)?;
    let high_frame = fixture(&high, &high, &full, true, true)?;
    let quiet_frame = fixture(&quiet, &quiet, &full, true, true)?;
    let cold_frame = fixture(&low, &low, &full, true, false)?;
    let mapped_off = fixture(&low, &low, &zero, true, true)?;
    let mapped_off_quiet = fixture(&low, &quiet, &zero, true, true)?;
    let stale = fixture(&low, &low, &full, false, true)?;
    let stale_quiet = fixture(&low, &quiet, &full, false, true)?;
    let frequency_difference = difference(&low_frame, &high_frame);
    let silence_difference = difference(&low_frame, &quiet_frame);
    // A new source starts with zero filter state. This compares the combined
    // warmed filter/history response, not retained history in isolation.
    let startup_difference = difference(&low_frame, &cold_frame);
    let zero_difference = difference(&mapped_off, &mapped_off_quiet);
    let stale_difference = difference(&stale, &stale_quiet);
    write_contact_sheet(
        &destination.join("media-dot-equaliser.png"),
        &[
            media_thumbnail(&low_frame)?,
            media_thumbnail(&high_frame)?,
            media_thumbnail(&quiet_frame)?,
            media_thumbnail(&cold_frame)?,
            media_thumbnail(&mapped_off)?,
            media_thumbnail(&stale)?,
        ],
        3,
    )?;
    for (label, pixels) in [
        ("low", &low_frame),
        ("high", &high_frame),
        ("quiet", &quiet_frame),
    ] {
        write_png(
            &destination.join(format!("media-dot-equaliser-{label}.png")),
            pixels,
            SIZE,
        )?;
    }
    for (label, change) in [
        ("low/high frequency position", frequency_difference),
        ("signal/silence", silence_difference),
        ("warmed response/cold startup", startup_difference),
    ] {
        anyhow::ensure!(
            change > 0.005,
            "Dot Equaliser: {label} does not change output ({change})"
        );
    }
    anyhow::ensure!(
        zero_difference == 0.0,
        "Dot Equaliser: zero mappings leak audio ({zero_difference})"
    );
    anyhow::ensure!(
        stale_difference == 0.0,
        "Dot Equaliser: stale input leaks audio ({stale_difference})"
    );
    Ok(serde_json::json!({
        "scene":SCENE,"name":SCENES[SCENE],"size":SIZE,
        "low_vs_high_rgb_difference":frequency_difference,
        "signal_vs_silence_rgb_difference":silence_difference,
        "warm_vs_cold_startup_rgb_difference":startup_difference,
        "zero_mapping_rgb_difference_vs_quiet":zero_difference,
        "stale_nonzero_features_rgb_difference_vs_quiet":stale_difference,
        "warm_submissions":64,"dt_seconds":0.1,"warm_readbacks":0,
        "sheet_columns":["low / cold startup","high / zero mappings","silence / stale"],
        "scope":"production dot geometry prepass isolated from the shared background by a diagnostic compositor; synthetic equal-scalar-energy FFT A/B, combined filter/history warmup and exact disconnects; no isolated retained-history, live capture, display FPS, subjective smoothness or recognition claim"
    }))
}

fn media_spectrum(time: f32) -> AudioFeatures {
    AudioFeatures {
        bass: 0.72 + (time * 0.37).sin() * 0.12,
        mids: 0.58 + (time * 0.43).cos() * 0.10,
        treble: 0.49 + (time * 0.29).sin() * 0.13,
        energy: 0.70,
        beat: 0.35,
        spectrum: (0..48)
            .map(|band| {
                let position = band as f32 / 47.0;
                let low = (-((position - 0.12 - (time * 0.31).sin() * 0.05) / 0.10).powi(2)).exp();
                let mid = (-((position - 0.48 - (time * 0.23).cos() * 0.14) / 0.13).powi(2)).exp();
                (0.025
                    + low * 0.55
                    + mid * 0.44
                    + (0.5 + 0.5 * (band as f32 * 0.61 + time * 0.75).sin()) * 0.19)
                    .clamp(0.0, 1.0)
            })
            .collect(),
        ..Default::default()
    }
}

fn media_thumbnail(pixels: &[u8]) -> anyhow::Result<Vec<u8>> {
    let width = SIZE[0] as usize * 2;
    let height = SIZE[1] as usize * 2;
    anyhow::ensure!(
        pixels.len() == width * height * 4,
        "invalid media thumbnail source"
    );
    let mut result = vec![0_u8; (SIZE[0] * SIZE[1] * 4) as usize];
    for y in 0..SIZE[1] as usize {
        for x in 0..SIZE[0] as usize {
            for channel in 0..4 {
                let mut sum = 0_u16;
                for dy in 0..2 {
                    for dx in 0..2 {
                        sum += pixels[((y * 2 + dy) * width + x * 2 + dx) * 4 + channel] as u16;
                    }
                }
                result[(y * SIZE[0] as usize + x) * 4 + channel] = (sum / 4) as u8;
            }
        }
    }
    Ok(result)
}

/// A short art-review pass for growing transitions and music-driven materials.
/// This uses synthetic feature/control snapshots only, never opens an audio input and is
/// deliberately separate from the longer renderer regression suite.
pub fn preview_magic(destination: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
    fs::write(
        destination.join("magic-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "status": "incomplete",
            "reason": "Material-response verification started; only a completed run writes pass.",
            "input": "synthetic features/controls only; no audio input opened"
        }))?,
    )?;
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
    .context("no GPU adapter for the visual preview")?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("amara growing-worlds preview"),
        ..Default::default()
    }))?;
    let mut renderer = VisualResources::new(&device, HISTORY_FORMAT);
    let profile = crate::presets::Profile::astral_bloom();
    let mut settings = profile.visual.clone();
    settings.feedback = 0.0;
    let music = AudioFeatures {
        bass: 0.72,
        mids: 0.58,
        treble: 0.49,
        energy: 0.64,
        beat: 0.35,
        spectrum: (0..48)
            .map(|band| 0.1 + (band as f32 * 0.73).sin().abs() * 0.53)
            .collect(),
        ..Default::default()
    };
    let base = |to, from, blend, time| {
        ShaderUniforms::new(
            time,
            1.0 / 60.0,
            to,
            from,
            blend,
            &settings,
            &profile.mappings,
            &music,
        )
        .with_math_blend(1, 3, 0.48, 12.0)
    };
    let mut growth = Vec::new();
    for (from, to, seed) in [(1, 3, 2.0), (3, 2, 10.0), (2, 0, 17.0)] {
        for blend in [0.0, 0.32, 0.66, 1.0] {
            renderer.history = None;
            growth.push(render_frame(
                &mut renderer,
                &device,
                &queue,
                base(to, from, blend, 18.0).with_auto_mix(1.0, seed),
                SIZE,
            )?);
        }
    }
    write_contact_sheet(&destination.join("growing-worlds.png"), &growth, 4)?;
    let mut families = Vec::new();
    for family in 1..4 {
        for time in [12.0, 18.0] {
            renderer.history = None;
            families.push(render_frame(
                &mut renderer,
                &device,
                &queue,
                base(1, 1, 1.0, time).with_math_blend(family, family, 1.0, time),
                SIZE,
            )?);
        }
    }
    write_contact_sheet(&destination.join("living-mathematics.png"), &families, 2)?;
    for row in families.chunks_exact(2) {
        anyhow::ensure!(
            difference(&row[0], &row[1]) > 0.5,
            "mathematics is effectively static across six seconds"
        );
    }
    // Exact family and world endpoints survive spatial growth, not just its
    // middle. Cold history isolates geometry from accumulated trails.
    let mut world_endpoints = Vec::new();
    for (from, to) in [(0, 1), (1, 2), (2, 3), (3, 2), (3, 0)] {
        for (amount, scene) in [(0.0, from), (1.0, to)] {
            renderer.history = None;
            let a = render_frame(
                &mut renderer,
                &device,
                &queue,
                base(to, from, amount, 12.0).with_auto_mix(1.0, 17.0),
                SIZE,
            )?;
            renderer.history = None;
            let b = render_frame(
                &mut renderer,
                &device,
                &queue,
                base(scene, scene, 1.0, 12.0),
                SIZE,
            )?;
            let endpoint_difference = difference(&a, &b);
            if endpoint_difference != 0.0 {
                write_contact_sheet(
                    &destination.join(format!(
                        "endpoint-failure-{from}-to-{to}-amount-{amount}.png"
                    )),
                    &[a, b],
                    2,
                )?;
            }
            anyhow::ensure!(
                endpoint_difference == 0.0,
                "world endpoint changed: {from}->{to}, amount {amount}, fixed world {scene}, mean RGB difference {endpoint_difference}"
            );
            world_endpoints.push(serde_json::json!({
                "from":from,"to":to,"amount":amount,"fixed_scene":scene,
                "rgb_difference":endpoint_difference
            }));
        }
        for (amount, family) in [(0.0, from), (1.0, to)] {
            renderer.history = None;
            let a = render_frame(
                &mut renderer,
                &device,
                &queue,
                base(1, 1, 1.0, 12.0).with_math_blend(from, to, amount, 12.0),
                SIZE,
            )?;
            renderer.history = None;
            let b = render_frame(
                &mut renderer,
                &device,
                &queue,
                base(1, 1, 1.0, 12.0).with_math_blend(family, family, 1.0, 12.0),
                SIZE,
            )?;
            anyhow::ensure!(difference(&a, &b) == 0.0, "mathematical endpoint changed");
        }
    }
    let material_responses = verify_material_responses(
        &mut renderer,
        &device,
        &queue,
        &profile,
        &music,
        destination,
    )?;
    let mut hero = settings.clone();
    hero.feedback = 0.7;
    hero.palette = crate::presets::VisualPalette::MoccaVanilla;
    renderer.history = None;
    let mut warm = Vec::new();
    for frame in 0..48 {
        let elapsed = frame as f32 / 60.0;
        warm = render_frame(
            &mut renderer,
            &device,
            &queue,
            ShaderUniforms::new(
                18.0 + elapsed,
                1.0 / 60.0,
                3,
                1,
                0.44,
                &hero,
                &profile.mappings,
                &music,
            )
            .with_math_blend(1, 3, 0.58, 12.0 + elapsed)
            .with_auto_mix(1.0, 2.0)
            .with_music_response([0.65, 0.45, 0.7, 0.5])
            .with_vocal_response([0.65, elapsed / 3.6, 0.5, elapsed * 0.35]),
            [1280, 800],
        )?;
    }
    write_png(
        &destination.join("mocca-growing-world.png"),
        &warm,
        [1280, 800],
    )?;
    let structural_looks = verify_structural_looks(
        &mut renderer,
        &device,
        &queue,
        &profile,
        &music,
        destination,
    )?;
    fs::write(
        destination.join("magic-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "status": "pass", "app_version": env!("CARGO_PKG_VERSION"),
            "adapter": info.name, "uniform_bytes": std::mem::size_of::<ShaderUniforms>(),
            "input": "synthetic features/controls only; no audio input opened",
            "scope": "short visual preview, exact math endpoints and all-world material response/disconnect checks; not an FPS benchmark or live capture proof",
            "material_responses": material_responses,
            "math_endpoints": "exact",
            "world_endpoints": world_endpoints,
            "warm_hero_frames": 48,
            "structural_looks": structural_looks
        }))?,
    )?;
    println!(
        "PASS growing worlds, living mathematics and all-world material response isolation on {}",
        info.name
    );
    Ok(())
}

/// Compare response states at fixed geometry/time. A new source epoch and the
/// same synthetic history sequence isolate each fixture, including landscape.
/// RGB comparisons verify that controls affect the image, not that every
/// intermediate shape is unchanged; inspect the shared material shader for that.
fn verify_material_responses(
    renderer: &mut VisualResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    audio: &AudioFeatures,
    destination: &Path,
) -> anyhow::Result<serde_json::Value> {
    const NO_MUSIC: [f32; 4] = [0.0; 4];
    const NO_VOICE: [f32; 4] = [0.0, 1.0, 0.0, 0.0];
    const MUSIC_A: [f32; 4] = [0.25, 0.2, 0.2, 0.15];
    const MUSIC_B: [f32; 4] = [0.9, 0.85, 0.8, 0.95];
    const VOICE: [f32; 4] = [0.8, 0.35, 0.95, 1.2];
    let mut settings = profile.visual.clone();
    settings.feedback = 0.0;
    let zero = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let mut epoch = 94_000_u64;
    let mut render = |scene: usize,
                      mappings: &AudioMappings,
                      music: [f32; 4],
                      voice: [f32; 4],
                      fresh: bool|
     -> anyhow::Result<Vec<u8>> {
        epoch += 1;
        renderer.history = None;
        let uniforms = |time: f32, dt: f32| {
            ShaderUniforms::new(time, dt, scene, scene, 1.0, &settings, mappings, audio)
                .with_math_blend(1, 3, 0.48, 12.0)
                .with_music_response(music)
                .with_vocal_response(voice)
                .with_audio_state(fresh, epoch)
        };
        if matches!(scene, 5..=7) && fresh {
            // Rebuild the exact same retained rows before every A/B image.
            // No readback during warmup, and no synthetic voice detector run.
            for frame in 0..64 {
                let mut encoder = device.create_command_encoder(&Default::default());
                renderer.render(
                    device,
                    queue,
                    &mut encoder,
                    uniforms(11.6 + frame as f32 * 0.1, 0.1),
                    SIZE,
                );
                queue.submit([encoder.finish()]);
            }
        }
        renderer.history = None;
        render_frame(renderer, device, queue, uniforms(18.0, 0.1), SIZE)
    };
    let mut sheet = Vec::new();
    let mut worlds = Vec::new();
    for (scene, name) in SCENES.iter().enumerate() {
        let baseline = render(scene, &profile.mappings, NO_MUSIC, NO_VOICE, true)?;
        let music_a = render(scene, &profile.mappings, MUSIC_A, NO_VOICE, true)?;
        let music_b = render(scene, &profile.mappings, MUSIC_B, NO_VOICE, true)?;
        let voice = render(scene, &profile.mappings, NO_MUSIC, VOICE, true)?;
        let change_a = difference(&baseline, &music_a);
        let change_b = difference(&baseline, &music_b);
        let change_ab = difference(&music_a, &music_b);
        let change_voice = difference(&baseline, &voice);
        // Write before asserting so a failed fixture still has reviewable art.
        let frames = [baseline, music_a, music_b, voice];
        write_contact_sheet(
            &destination.join(format!("material-response-world-{scene}.png")),
            &frames,
            4,
        )?;
        for (label, change) in [
            ("music A", change_a),
            ("music B", change_b),
            ("music A/B", change_ab),
            ("voice entry", change_voice),
        ] {
            anyhow::ensure!(
                change > 0.015,
                "{name}: {label} material response is invisible ({change})"
            );
        }
        let zero_base = render(scene, &zero, NO_MUSIC, NO_VOICE, true)?;
        let zero_driven = render(scene, &zero, MUSIC_B, VOICE, true)?;
        let zero_change = difference(&zero_base, &zero_driven);
        anyhow::ensure!(
            zero_change == 0.0,
            "{name}: zero mappings leak material response ({zero_change})"
        );
        let stale_base = render(scene, &profile.mappings, NO_MUSIC, NO_VOICE, false)?;
        let stale_driven = render(scene, &profile.mappings, MUSIC_B, VOICE, false)?;
        let stale_change = difference(&stale_base, &stale_driven);
        anyhow::ensure!(
            stale_change == 0.0,
            "{name}: stale source leaks material response ({stale_change})"
        );
        sheet.extend(frames);
        worlds.push(serde_json::json!({
            "scene":scene,"name":name,
            "music_a_rgb_difference":change_a,"music_b_rgb_difference":change_b,
            "music_a_vs_b_rgb_difference":change_ab,"voice_rgb_difference":change_voice,
            "all_zero_mappings_rgb_difference":zero_change,"stale_rgb_difference":stale_change
        }));
    }
    write_contact_sheet(&destination.join("material-responses.png"), &sheet, 4)?;
    Ok(serde_json::json!({
        "status":"pass","size":SIZE,"world_count":WORLD_COUNT,
        "music_a":MUSIC_A,"music_b":MUSIC_B,"voice_entry":VOICE,
        "sheet_columns":["response controls off","music A","music B","voice entry"],
        "scope":"fixed synthetic audio/phase, cold feedback and deterministic landscape history; RGB A/B and exact response disconnects only; image deltas alone do not prove unchanged geometry, recognition accuracy, aesthetics or FPS",
        "worlds":worlds
    }))
}

/// A focused contact-sheet study, not another broad aesthetic regression.
/// Readback is RGBA8: finite image statistics and nonblack output are checked;
/// these do not establish the absence of every intermediate shader NaN.
fn verify_structural_looks(
    renderer: &mut VisualResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    music: &AudioFeatures,
    destination: &Path,
) -> anyhow::Result<serde_json::Value> {
    // This historical 40-look study deliberately covers only the original four
    // procedural worlds. Mutable-history raster worlds have dedicated isolated
    // fixtures in preview_media rather than sharing cold endpoint assumptions.
    const LEGACY_WORLD_COUNT: usize = 4;
    let recipe_count: usize = LOOKS
        .iter()
        .take(LEGACY_WORLD_COUNT)
        .map(|looks| looks.len())
        .sum();
    let mut settings = profile.visual.clone();
    settings.feedback = 0.0;
    settings.complexity = 1.35;
    settings.palette = crate::presets::VisualPalette::Cosmic;
    let base = |scene, family, from, to, blend| {
        let mut looks = [[0.0, 0.0, 1.0, 18.0]; WORLD_COUNT];
        looks[scene] = [from as f32, to as f32, blend, 18.0];
        ShaderUniforms::new(
            18.0,
            1.0 / 60.0,
            scene,
            scene,
            1.0,
            &settings,
            &profile.mappings,
            music,
        )
        .with_math_blend(family, family, 1.0, 18.0)
        .with_looks(looks)
    };
    let inspect =
        |pixels: &[u8], size: [u32; 2], label: &str| -> anyhow::Result<serde_json::Value> {
            anyhow::ensure!(
                pixels.len() == (size[0] * size[1] * 4) as usize,
                "{label}: invalid readback dimensions"
            );
            let (mean, deviation) = stats(pixels);
            let spatial = luma_deviation(pixels);
            anyhow::ensure!(
                mean.is_finite() && deviation.is_finite() && spatial.is_finite(),
                "{label}: non-finite image statistics"
            );
            anyhow::ensure!(
                pixels
                    .chunks_exact(4)
                    .any(|pixel| pixel[..3].iter().any(|value| *value > 1)),
                "{label}: black image"
            );
            Ok(serde_json::json!({
                "mean_rgb": mean, "rgb_standard_deviation": deviation,
                "spatial_luma_standard_deviation": spatial,
                "nonblack": true, "finite_image_statistics": true
            }))
        };
    let mut worlds = Vec::new();
    for (scene, names) in LOOKS.iter().take(LEGACY_WORLD_COUNT).enumerate() {
        let mut frames = Vec::new();
        let mut recipes = Vec::new();
        let sheet = format!("structural-looks-world-{scene}.png");
        for (look, name) in names.iter().enumerate() {
            let family = if scene == 1 { look % 4 } else { 0 };
            renderer.history = None;
            let pixels = render_frame(
                renderer,
                device,
                queue,
                base(scene, family, look, look, 1.0),
                SIZE,
            )?;
            let measured = inspect(&pixels, SIZE, name)?;
            recipes.push(serde_json::json!({
                "look": look, "name": name, "math_family": (scene == 1).then_some(family),
                "sheet_column": look % 5, "sheet_row": look / 5, "image": measured
            }));
            frames.push(pixels);
        }
        write_contact_sheet(&destination.join(&sheet), &frames, 5)?;
        worlds.push(serde_json::json!({
            "world": SCENES[scene], "scene": scene, "sheet": sheet, "recipes": recipes
        }));
    }

    // Isolate coefficient morphs from family/world selection and feedback.
    // Vortex is fixed for the mathematical world in these endpoint probes.
    let mut morphs = Vec::new();
    let mut morph_frames = Vec::new();
    for (scene, _) in SCENES.iter().take(LEGACY_WORLD_COUNT).enumerate() {
        let mut endpoints = Vec::new();
        for look in [0, 7] {
            renderer.history = None;
            endpoints.push(render_frame(
                renderer,
                device,
                queue,
                base(scene, 3, look, look, 1.0),
                SIZE,
            )?);
        }
        let mut measured_endpoints = Vec::new();
        let mut middle_distances = Vec::new();
        for amount in [0.0, 0.5, 1.0] {
            renderer.history = None;
            let image = render_frame(renderer, device, queue, base(scene, 3, 0, 7, amount), SIZE)?;
            inspect(&image, SIZE, "structural look morph")?;
            if amount == 0.5 {
                middle_distances = endpoints
                    .iter()
                    .map(|frame| difference(frame, &image))
                    .collect();
                anyhow::ensure!(
                    middle_distances.iter().all(|change| *change > 0.005),
                    "{}: look midpoint collapsed to an endpoint",
                    SCENES[scene]
                );
            } else {
                let endpoint = usize::from(amount == 1.0);
                let change = difference(&endpoints[endpoint], &image);
                anyhow::ensure!(change == 0.0, "{}: look endpoint changed", SCENES[scene]);
                measured_endpoints.push(change);
            }
            morph_frames.push(image);
        }
        morphs.push(serde_json::json!({
            "scene": scene, "from_look": 0, "to_look": 7,
            "endpoint_rgb_differences": measured_endpoints,
            "midpoint_rgb_differences_from_endpoints": middle_distances
        }));
    }
    write_contact_sheet(
        &destination.join("structural-look-morphs.png"),
        &morph_frames,
        3,
    )?;

    let mut hero_settings = settings.clone();
    hero_settings.feedback = 0.72;
    hero_settings.palette = crate::presets::VisualPalette::MoccaVanilla;
    const HERO_SIZE: [u32; 2] = [1280, 800];
    const HERO_FRAMES: u32 = 24;
    let mut heroes = Vec::new();
    for (scene, look, family, combined) in [(1, 4, 0, false), (3, 6, 0, false), (3, 7, 3, true)] {
        renderer.history = None;
        let mut pixels = Vec::new();
        for frame in 0..HERO_FRAMES {
            let elapsed = frame as f32 / 60.0;
            let mut looks = [[0.0, 0.0, 1.0, 18.0 + elapsed]; WORLD_COUNT];
            looks[scene] = [look as f32, look as f32, 1.0, 18.0 + elapsed];
            if combined {
                looks[1] = [0.0, 7.0, 0.5, 18.0 + elapsed];
                looks[3] = [0.0, 7.0, 0.5, 18.0 + elapsed];
            }
            let uniforms = ShaderUniforms::new(
                18.0 + elapsed,
                1.0 / 60.0,
                scene,
                if combined { 1 } else { scene },
                if combined { 0.52 } else { 1.0 },
                &hero_settings,
                &profile.mappings,
                music,
            )
            .with_math_blend(
                if combined { 1 } else { family },
                family,
                if combined { 0.55 } else { 1.0 },
                18.0 + elapsed,
            )
            .with_auto_mix(f32::from(combined), 2.0)
            .with_looks(looks);
            pixels = render_frame(renderer, device, queue, uniforms, HERO_SIZE)?;
        }
        let file = if combined {
            "structural-looks-mocca-combined.png".to_owned()
        } else {
            format!("structural-looks-mocca-world-{scene}-look-{look}.png")
        };
        write_png(&destination.join(&file), &pixels, HERO_SIZE)?;
        let measured = inspect(&pixels, HERO_SIZE, "warm structural hero")?;
        heroes.push(serde_json::json!({
            "file": file, "scene": scene, "look": look, "combined_morph": combined,
            "frames": HERO_FRAMES, "size": HERO_SIZE, "image": measured
        }));
    }
    println!(
        "PASS {recipe_count} original-four-world structural previews and exact look-morph endpoints"
    );
    Ok(serde_json::json!({
        "status": "pass", "fixed_recipe_count": recipe_count, "world_count":LEGACY_WORLD_COUNT,
        "scene_indices":[0,1,2,3],"size": SIZE,
        "scope":"historical original four procedural worlds only; new geometry worlds are covered by preview_media",
        "sheet_layout": {"columns": 5, "rows": 2, "order": "look 0 through 9, row-major"},
        "input": "fixed synthetic audio; math families distributed by look modulo four in the original-world study",
        "guard_scope": "RGBA8 output nonblack and finite image statistics; no aesthetic score, FPS claim, or intermediate floating-point proof",
        "worlds": worlds, "look_morphs": morphs, "warm_heroes": heroes
    }))
}

fn verify_canvas_centre(
    renderer: &mut VisualResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    music: &AudioFeatures,
    destination: &Path,
) -> anyhow::Result<serde_json::Value> {
    let mut settings = profile.visual.clone();
    settings.scene = 1;
    settings.feedback = 0.55;
    const SHIFT_PIXELS: usize = 16;
    let shift = 2.0 * SHIFT_PIXELS as f32 / SIZE[1] as f32;
    let mut cold = Vec::new();
    let mut warm = Vec::new();
    for offset in [0.0, shift] {
        renderer.history = None;
        for step in 0..=1 {
            let time = 14.0 + step as f32 / 60.0;
            let pixels = render_frame(
                renderer,
                device,
                queue,
                ShaderUniforms::new(
                    time,
                    1.0 / 60.0,
                    1,
                    1,
                    1.0,
                    &settings,
                    &profile.mappings,
                    music,
                )
                .with_math_blend(2, 2, 1.0, time * 0.13)
                .with_canvas_centre(offset),
                SIZE,
            )?;
            if step == 0 {
                cold.push(pixels);
            } else {
                warm.push(pixels);
            }
        }
    }
    let registered_difference = |first: &[u8], translated: &[u8]| {
        let width = SIZE[0] as usize;
        let height = SIZE[1] as usize;
        let mut sum = 0.0;
        let mut values = 0;
        // Ignore clipped borders; compare the same interior world coordinates.
        for y in 32..height - 32 {
            for x in 48..width - 48 {
                let a = (y * width + x) * 4;
                let b = (y * width + x + SHIFT_PIXELS) * 4;
                for channel in 0..3 {
                    sum +=
                        (f64::from(first[a + channel]) - f64::from(translated[b + channel])).abs();
                    values += 1;
                }
            }
        }
        sum / f64::from(values)
    };
    let cold_difference = registered_difference(&cold[0], &cold[1]);
    let warm_difference = registered_difference(&warm[0], &warm[1]);
    anyhow::ensure!(
        cold_difference < 1.0 && warm_difference < 1.0,
        "canvas-centre translation changes registered geometry/history (cold {cold_difference:.4}, warm {warm_difference:.4})"
    );
    let preview = [
        cold[0].clone(),
        cold[1].clone(),
        warm[0].clone(),
        warm[1].clone(),
    ];
    write_contact_sheet(&destination.join("canvas-centre.png"), &preview, 2)?;
    println!("PASS canvas-centre translation and feedback coordinate registration");
    Ok(
        serde_json::json!({"shift_pixels": SHIFT_PIXELS, "uniform_shift": shift,
        "cold_registered_rgb_difference": cold_difference,
        "two_frame_feedback_registered_rgb_difference": warm_difference,
        "scope": "Synthetic Harmonic Sculpture; interior registration sanity, not full UI layout proof"}),
    )
}

fn verify_math_families(
    renderer: &mut VisualResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    audio: (&AudioFeatures, &AudioFeatures),
    destination: &Path,
) -> anyhow::Result<serde_json::Value> {
    use crate::presets::MathForm;
    let forms = [
        MathForm::Julia,
        MathForm::InversionLace,
        MathForm::HarmonicSculpture,
        MathForm::VortexWeave,
    ];
    let (quiet, music) = audio;
    let mut settings = profile.visual.clone();
    settings.scene = 1;
    settings.complexity = 1.35;
    settings.feedback = 0.0;
    let disconnected = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let mut no_spectrum = music.clone();
    no_spectrum.spectrum.fill(0.0);
    // Isolate each mathematical family from journey scheduling, outer-world
    // transitions and framebuffer history. Continuous phase advances with the
    // sample clock used by these synthetic cases, not an audio scalar.
    let mut render = |from: usize,
                      to: usize,
                      blend: f32,
                      time: f32,
                      palette: f32,
                      mappings: &AudioMappings,
                      features: &AudioFeatures| {
        renderer.history = None;
        render_frame(
            renderer,
            device,
            queue,
            ShaderUniforms::new(time, 1.0 / 60.0, 1, 1, 1.0, &settings, mappings, features)
                .with_palette_mix(palette)
                .with_math_blend(from, to, blend, time * 0.13),
            SIZE,
        )
    };
    let mut family_previews = Vec::new();
    let mut time_previews = Vec::new();
    let mut morph_previews = Vec::new();
    let mut cosmic_quiet = Vec::new();
    let mut cosmic_music = Vec::new();
    let mut mocca_quiet = Vec::new();
    let mut mocca_music = Vec::new();
    let mut families = Vec::new();
    for form in forms {
        let family = form.family().expect("fixed mathematical family");
        let name = form.label();
        let cq = render(family, family, 1.0, 12.0, 0.0, &profile.mappings, quiet)?;
        let cm = render(family, family, 1.0, 12.0, 0.0, &profile.mappings, music)?;
        let mq = render(family, family, 1.0, 12.0, 1.0, &profile.mappings, quiet)?;
        let mm = render(family, family, 1.0, 12.0, 1.0, &profile.mappings, music)?;
        let audio_difference = difference(&cq, &cm);
        let mocca_audio_difference = difference(&mq, &mm);
        let palette_difference = difference(&cm, &mm);
        for (label, pixels) in [
            ("Cosmic quiet", &cq),
            ("Cosmic music", &cm),
            ("Mocca quiet", &mq),
            ("Mocca music", &mm),
        ] {
            let (mean, variation) = stats(pixels);
            anyhow::ensure!(
                mean > 1.5 && mean < 235.0 && variation > 6.0 && luma_deviation(pixels) > 3.0,
                "{name}: {label} loses exposure or spatial structure (mean {mean:.3})"
            );
        }
        anyhow::ensure!(
            audio_difference > 0.35 && mocca_audio_difference > 0.35,
            "{name}: quiet/music no longer changes both palette variants"
        );
        anyhow::ensure!(
            palette_difference > 1.0,
            "{name}: Cosmic and Mocca palettes are effectively identical"
        );
        let mut zero_mapping_differences = Vec::new();
        for palette in [0.0, 1.0] {
            let off_quiet = render(family, family, 1.0, 12.0, palette, &disconnected, quiet)?;
            let off_music = render(family, family, 1.0, 12.0, palette, &disconnected, music)?;
            let change = difference(&off_quiet, &off_music);
            anyhow::ensure!(
                change < 0.02,
                "{name}: disconnected mappings leak audio at palette {palette}"
            );
            zero_mapping_differences.push(change);
        }
        let mut routes = Vec::new();
        for (band, start, end) in [("bass", 0, 8), ("mids", 8, 24), ("treble", 24, 48)] {
            let mut isolated = no_spectrum.clone();
            isolated.spectrum[start..end].copy_from_slice(&music.spectrum[start..end]);
            let enabled = AudioMappings {
                bass_to_pulse: f32::from(start == 0),
                mids_to_geometry: f32::from(start == 8),
                treble_to_sparkle: f32::from(start == 24),
                beat_to_flash: 0.0,
            };
            let disabled = AudioMappings {
                bass_to_pulse: 1.0 - enabled.bass_to_pulse,
                mids_to_geometry: 1.0 - enabled.mids_to_geometry,
                treble_to_sparkle: 1.0 - enabled.treble_to_sparkle,
                beat_to_flash: 1.0,
            };
            let baseline = render(family, family, 1.0, 12.0, 0.0, &enabled, &no_spectrum)?;
            let routed = render(family, family, 1.0, 12.0, 0.0, &enabled, &isolated)?;
            let enabled_difference = difference(&baseline, &routed);
            let baseline = render(family, family, 1.0, 12.0, 0.0, &disabled, &no_spectrum)?;
            let off = render(family, family, 1.0, 12.0, 0.0, &disabled, &isolated)?;
            let disabled_difference = difference(&baseline, &off);
            anyhow::ensure!(
                enabled_difference > 0.02 && disabled_difference < 0.02,
                "{name}: {band} FFT routing missing or leaks to another mapping (on {enabled_difference:.4}, off {disabled_difference:.4})"
            );
            routes.push(serde_json::json!({"band": band,
                "enabled_difference": enabled_difference, "disabled_difference": disabled_difference}));
        }
        let mut continuity = Vec::new();
        for time in [12.0, 32.0] {
            let first = if time == 12.0 {
                cm.clone()
            } else {
                render(family, family, 1.0, time, 0.0, &profile.mappings, music)?
            };
            let mut previous = first.clone();
            let mut maximum_step = 0.0_f64;
            for step in 1..=2 {
                let next = render(
                    family,
                    family,
                    1.0,
                    time + step as f32 / 60.0,
                    0.0,
                    &profile.mappings,
                    music,
                )?;
                maximum_step = maximum_step.max(difference(&previous, &next));
                previous = next;
            }
            let motion = difference(&first, &previous);
            anyhow::ensure!(
                maximum_step < MAX_TEMPORAL_STEP && motion > 0.004,
                "{name}: discontinuous or static small time steps at {time}s (step {maximum_step:.3}, motion {motion:.3})"
            );
            continuity.push(serde_json::json!({"time": time,
                "maximum_60hz_step_rgb_difference": maximum_step, "two_step_rgb_difference": motion}));
            time_previews.extend([first, previous]);
        }
        family_previews.extend([cq.clone(), cm.clone(), mq.clone(), mm.clone()]);
        write_png(
            &destination.join(format!("math-family-{family}.png")),
            &cm,
            SIZE,
        )?;
        families.push(serde_json::json!({"family": family, "name": name,
            "audio_difference": audio_difference, "mocca_audio_difference": mocca_audio_difference,
            "palette_difference": palette_difference, "zero_mapping_differences": zero_mapping_differences,
            "music_spatial_luma_deviation": luma_deviation(&cm),
            "spectrum_routing": routes, "temporal_continuity": continuity}));
        cosmic_quiet.push(cq);
        cosmic_music.push(cm);
        mocca_quiet.push(mq);
        mocca_music.push(mm);
        println!(
            "PASS {name}: isolated mathematics, both palettes, measured spectrum routes and small time steps"
        );
    }
    let mut pairs = Vec::new();
    for first in 0..forms.len() {
        for second in first + 1..forms.len() {
            let cq = difference(&cosmic_quiet[first], &cosmic_quiet[second]);
            let cm = difference(&cosmic_music[first], &cosmic_music[second]);
            let mq = difference(&mocca_quiet[first], &mocca_quiet[second]);
            let mm = difference(&mocca_music[first], &mocca_music[second]);
            anyhow::ensure!(
                cq > 2.0 && cm > 3.0 && mq > 1.0 && mm > 2.0,
                "{} and {} have converged in one or more quiet/music palette variants",
                forms[first].label(),
                forms[second].label()
            );
            pairs.push(
                serde_json::json!({"from": forms[first].label(), "to": forms[second].label(),
                "cosmic_quiet_difference": cq, "cosmic_music_difference": cm,
                "mocca_quiet_difference": mq, "mocca_music_difference": mm}),
            );
        }
    }
    let mut morphs = Vec::new();
    for from in 0..forms.len() {
        let to = (from + 1) % forms.len();
        let start = render(from, to, 0.0, 12.0, 0.0, &profile.mappings, music)?;
        let midpoint = render(from, to, 0.5, 12.0, 0.0, &profile.mappings, music)?;
        let end = render(from, to, 1.0, 12.0, 0.0, &profile.mappings, music)?;
        let start_difference = difference(&start, &cosmic_music[from]);
        let end_difference = difference(&end, &cosmic_music[to]);
        let middle_from = difference(&midpoint, &start);
        let middle_to = difference(&midpoint, &end);
        anyhow::ensure!(
            start_difference < 0.02 && end_difference < 0.02,
            "{} to {} family morph has discontinuous endpoints",
            forms[from].label(),
            forms[to].label()
        );
        anyhow::ensure!(
            middle_from > 0.3 && middle_to > 0.3,
            "{} to {} family morph midpoint is not a distinct blend",
            forms[from].label(),
            forms[to].label()
        );
        morphs.push(serde_json::json!({"from": forms[from].label(), "to": forms[to].label(),
            "start_endpoint_difference": start_difference, "end_endpoint_difference": end_difference,
            "midpoint_source_difference": middle_from, "midpoint_destination_difference": middle_to}));
        morph_previews.extend([start, midpoint, end]);
    }
    write_contact_sheet(&destination.join("math-families.png"), &family_previews, 4)?;
    write_contact_sheet(
        &destination.join("math-family-times.png"),
        &time_previews,
        4,
    )?;
    write_contact_sheet(
        &destination.join("math-family-morphs.png"),
        &morph_previews,
        3,
    )?;
    Ok(
        serde_json::json!({"outer_world": SCENES[1], "uniform": "u.j = from, to, blend, continuous phase",
        "input": "synthetic features only; cold history; family state supplied directly, not journey scheduling",
        "size": SIZE, "families": families, "pair_differences": pairs, "morphs": morphs,
        "limits": "Pixel regressions and inspectable previews, not subjective beauty or display FPS. Journey timing is verified by its CPU tests."}),
    )
}

fn verify_palette_study(
    renderer: &mut VisualResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    audio: (&AudioFeatures, &AudioFeatures),
    cosmic_previews: &[Vec<u8>],
    destination: &Path,
) -> anyhow::Result<serde_json::Value> {
    let (quiet, music) = audio;
    let mut settings = profile.visual.clone();
    settings.feedback = 0.0;
    let disconnected = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let mut pairs = Vec::new();
    let mut quiet_music = Vec::new();
    let mut warm_worlds = Vec::new();
    let mut report = Vec::new();
    for (scene, name) in SCENES.iter().enumerate() {
        let (cosmic, mocca_quiet, mocca_music, zero_quiet, zero_music) = {
            let mut render = |features: &AudioFeatures, mappings: &AudioMappings, palette: f32| {
                let mut uniforms = ShaderUniforms::new(
                    12.0,
                    1.0 / 60.0,
                    scene,
                    scene,
                    1.0,
                    &settings,
                    mappings,
                    features,
                );
                uniforms.values[3][3] = palette;
                renderer.history = None;
                render_frame(renderer, device, queue, uniforms, SIZE)
            };
            (
                render(music, &profile.mappings, 0.0)?,
                render(quiet, &profile.mappings, 1.0)?,
                render(music, &profile.mappings, 1.0)?,
                render(quiet, &disconnected, 1.0)?,
                render(music, &disconnected, 1.0)?,
            )
        };
        let zero_palette_difference = difference(&cosmic, &cosmic_previews[scene]);
        let zero_mapping_difference = difference(&zero_quiet, &zero_music);
        let audio_difference = difference(&mocca_quiet, &mocca_music);
        let aligned_edges = edge_alignment(&cosmic, &mocca_music);
        anyhow::ensure!(
            zero_palette_difference == 0.0,
            "{name}: Cosmic palette zero differs from the normal Cosmic render"
        );
        anyhow::ensure!(
            zero_mapping_difference == 0.0,
            "{name}: Mocca palette bypasses zero audio mappings"
        );
        anyhow::ensure!(
            audio_difference > 0.6 && aligned_edges > 0.65,
            "{name}: Mocca loses audio response or registered material structure"
        );
        write_png(
            &destination.join(format!("mocca-world-{scene}-quiet.png")),
            &mocca_quiet,
            SIZE,
        )?;
        write_png(
            &destination.join(format!("mocca-world-{scene}-music.png")),
            &mocca_music,
            SIZE,
        )?;
        let mut warm_settings = settings.clone();
        warm_settings.feedback = 0.86;
        renderer.history = None;
        let mut warm = Vec::new();
        for frame in 0..120 {
            let time = 12.0 - (119 - frame) as f32 / 60.0 * warm_settings.motion;
            let mut uniforms = ShaderUniforms::new(
                time,
                1.0 / 60.0,
                scene,
                scene,
                1.0,
                &warm_settings,
                &profile.mappings,
                music,
            );
            uniforms.values[3][3] = 1.0;
            warm = render_frame(renderer, device, queue, uniforms, SIZE)?;
        }
        let (mean, variation) = stats(&warm);
        let spatial = luma_deviation(&warm);
        anyhow::ensure!(
            mean > 2.0 && mean < 225.0 && variation > 8.0 && spatial > MIN_SPATIAL_LUMA_DEVIATION,
            "{name}: warmed Mocca loses exposure or spatial structure"
        );
        write_png(
            &destination.join(format!("mocca-world-{scene}-warm.png")),
            &warm,
            SIZE,
        )?;
        report.push(serde_json::json!({
            "scene": name,
            "cosmic_zero_difference": zero_palette_difference,
            "mocca_zero_mapping_difference": zero_mapping_difference,
            "mocca_audio_difference": audio_difference,
            "cosmic_mocca_registered_edge_alignment": aligned_edges,
            "warm_120_frames": {
                "feedback": warm_settings.feedback,
                "mean_rgb": mean,
                "standard_deviation": variation,
                "spatial_luma_standard_deviation": spatial
            }
        }));
        pairs.extend([cosmic, mocca_music.clone()]);
        quiet_music.extend([mocca_quiet, mocca_music]);
        warm_worlds.push(warm);
        println!(
            "PASS {name}: Cosmic zero, Mocca quiet/music/mapping isolation, registered edges and warm history"
        );
    }
    write_contact_sheet(&destination.join("palettes.png"), &pairs, 2)?;
    write_contact_sheet(&destination.join("mocca-quiet-music.png"), &quiet_music, 2)?;
    write_contact_sheet(&destination.join("mocca-warm.png"), &warm_worlds, 2)?;
    Ok(serde_json::json!({
        "uniform": "u.d.w, continuous 0 Cosmic to 1 Mocca & Vanilla",
        "scope": "post-world colour only; edge alignment is a registration regression proxy, not a geometry proof",
        "minimum_registered_edge_alignment": 0.65,
        "worlds": report
    }))
}

fn edge_alignment(first: &[u8], second: &[u8]) -> f64 {
    let width = SIZE[0] as usize;
    let height = SIZE[1] as usize;
    let mut product = 0.0;
    let mut first_energy = 0.0;
    let mut second_energy = 0.0;
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let gradient = |pixels: &[u8]| {
                let value = |x, y| {
                    let offset = (y * width + x) * 4;
                    display_luma(&pixels[offset..offset + 4])
                };
                [
                    value(x + 1, y) - value(x - 1, y),
                    value(x, y + 1) - value(x, y - 1),
                ]
            };
            let a = gradient(first);
            let b = gradient(second);
            product += a[0] * b[0] + a[1] * b[1];
            first_energy += a[0] * a[0] + a[1] * a[1];
            second_energy += b[0] * b[0] + b[1] * b[1];
        }
    }
    product / (first_energy * second_energy).sqrt().max(0.000001)
}

fn verify_fractal_study(
    renderer: &mut VisualResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &crate::presets::Profile,
    music: &AudioFeatures,
    destination: &Path,
) -> anyhow::Result<serde_json::Value> {
    let mut settings = profile.visual.clone();
    settings.scene = 1;
    settings.complexity = 1.2;
    settings.hue = 0.78;
    settings.motion = 0.62;
    settings.feedback = 0.86;
    let mut cold_settings = settings.clone();
    cold_settings.feedback = 0.0;
    let mut frames = Vec::new();
    let mut report = Vec::new();
    for time in [2.0_f32, 12.0, 32.0] {
        renderer.history = None;
        let cold = render_frame(
            renderer,
            device,
            queue,
            ShaderUniforms::new(
                time,
                1.0 / 60.0,
                1,
                1,
                1.0,
                &cold_settings,
                &profile.mappings,
                music,
            ),
            SIZE,
        )?;
        write_png(
            &destination.join(format!("fractal-cold-{}.png", time as u32)),
            &cold,
            SIZE,
        )?;
        renderer.history = None;
        let mut warm = Vec::new();
        for frame in 0..120 {
            let render_time = time - (119 - frame) as f32 / 60.0 * settings.motion;
            warm = render_frame(
                renderer,
                device,
                queue,
                ShaderUniforms::new(
                    render_time,
                    1.0 / 60.0,
                    1,
                    1,
                    1.0,
                    &settings,
                    &profile.mappings,
                    music,
                ),
                SIZE,
            )?;
        }
        write_png(
            &destination.join(format!("fractal-warm-{}.png", time as u32)),
            &warm,
            SIZE,
        )?;
        let (cold_mean, _) = stats(&cold);
        let (warm_mean, _) = stats(&warm);
        let warm_spatial = luma_deviation(&warm);
        anyhow::ensure!(
            cold_mean > 2.0
                && cold_mean < 225.0
                && warm_mean > 2.0
                && warm_mean < 225.0
                && warm_spatial > MIN_SPATIAL_LUMA_DEVIATION,
            "fractal study at {time}s loses exposure or spatial detail"
        );
        report.push(serde_json::json!({
            "render_time": time,
            "cold_mean_rgb": cold_mean,
            "warm_mean_rgb": warm_mean,
            "warm_spatial_luma_standard_deviation": warm_spatial,
            "cold_warm_difference": difference(&cold, &warm),
            "cold_dark_fraction": dark_fraction(&cold),
            "warm_dark_fraction": dark_fraction(&warm)
        }));
        frames.push(cold);
        frames.push(warm);
    }
    write_contact_sheet(&destination.join("fractal-study.png"), &frames, 2)?;

    // Hold the camera, all audio and the phrase-light sum fixed. Only the
    // slow drive/lift split changes, so this pair exercises Julia morphology
    // rather than a rigid camera rotation or a uniform brightness change.
    let mut morphology = Vec::new();
    for (drive, lift) in [(0.2, 0.7), (0.8, 0.1)] {
        renderer.history = None;
        morphology.push(render_frame(
            renderer,
            device,
            queue,
            ShaderUniforms::new(
                12.0,
                1.0 / 60.0,
                1,
                1,
                1.0,
                &cold_settings,
                &profile.mappings,
                music,
            )
            .with_choreography(Choreography {
                drive,
                lift,
                openness: 0.75,
                accent: 0.0,
                phase: 12.0,
                travel_speed: 1.0,
            }),
            SIZE,
        )?);
    }
    let phrase_morph_difference = difference(&morphology[0], &morphology[1]);
    anyhow::ensure!(
        phrase_morph_difference > 1.0,
        "Julia phrase controls change neither geometry nor orbit-trap structure"
    );
    let five_second_time = 12.0 + 5.0 * settings.motion;
    renderer.history = None;
    let after_five_seconds = render_frame(
        renderer,
        device,
        queue,
        ShaderUniforms::new(
            five_second_time,
            1.0 / 60.0,
            1,
            1,
            1.0,
            &cold_settings,
            &profile.mappings,
            music,
        ),
        SIZE,
    )?;
    let five_second_difference = difference(&frames[2], &after_five_seconds);
    anyhow::ensure!(
        five_second_difference > 2.0,
        "Julia remains effectively static over five seconds at Motion 0.62"
    );
    morphology.push(frames[2].clone());
    morphology.push(after_five_seconds);
    write_contact_sheet(&destination.join("fractal-morph-proof.png"), &morphology, 2)?;
    renderer.history = None;
    let native_size = [1600, 1000];
    let native = render_frame(
        renderer,
        device,
        queue,
        ShaderUniforms::new(
            12.0,
            1.0 / 60.0,
            1,
            1,
            1.0,
            &cold_settings,
            &profile.mappings,
            music,
        ),
        native_size,
    )?;
    write_png(
        &destination.join("fractal-study-native.png"),
        &native,
        native_size,
    )?;
    println!("PASS recursive Julia: three cold/warm studies and 1600x1000 readback");
    Ok(serde_json::json!({
        "math": "quadratic complex Julia orbit, analytic derivative relief, distance and orbit-trap contours",
        "maximum_iterations": 36,
        "detail": settings.complexity,
        "hue": settings.hue,
        "motion": settings.motion,
        "feedback": settings.feedback,
        "warm_frames_per_time": 120,
        "frame_delta_seconds": 1.0 / 60.0,
        "times": report,
        "phrase_morphology": {
            "same_render_time": 12.0,
            "fixed_camera_audio_and_drive_plus_lift": true,
            "drive_lift_pairs": [[0.2, 0.7], [0.8, 0.1]],
            "rgb_difference": phrase_morph_difference
        },
        "five_seconds_at_motion_062": {
            "start_render_time": 12.0,
            "end_render_time": five_second_time,
            "travel_speed": 1.0,
            "rgb_difference": five_second_difference,
            "scope": "fixed synthetic features; domain and Julia parameters both evolve"
        },
        "high_resolution_size": native_size,
        "scope": "synthetic fixed features; GPU image checks, not display frame rate or subjective beauty"
    }))
}

fn build_phrase_sequence(
    mappings: &AudioMappings,
    visual: &VisualSettings,
    quiet: &AudioFeatures,
    music: &AudioFeatures,
) -> anyhow::Result<PhraseSequence> {
    let dt = 1.0 / PHRASE_HZ as f32;
    let mut controller = Choreographer::new();
    let mut reset_probe = Choreographer::new();
    let mut phrase = controller.update_mapped(quiet, mappings, 0.0);
    let mut render_time = 0.0;
    let mut sequence = PhraseSequence {
        snapshots: vec![PhraseSnapshot {
            seconds: 0,
            stage: "initial quiet",
            audio: quiet.clone(),
            phrase,
            paused_phrase: phrase,
            render_time,
        }],
        reset_before: phrase,
        reset_after: phrase,
        max_phase_step: 0.0,
        max_render_step: 0.0,
    };
    for frame in 1..=28 * PHRASE_HZ {
        let audio = if frame > 8 * PHRASE_HZ && frame <= 16 * PHRASE_HZ {
            music
        } else {
            quiet
        };
        let previous = phrase;
        let previous_time = render_time;
        phrase = controller.update_mapped(audio, mappings, dt);
        // This integrates the same render clock as app.rs; no mapped-energy
        // formula is duplicated here because update_mapped owns that boundary.
        render_time += dt * motion_rate(visual.motion) * phrase.travel_speed;
        let phase_step = phrase.phase - previous.phase;
        let render_step = render_time - previous_time;
        anyhow::ensure!(
            [
                phrase.drive,
                phrase.openness,
                phrase.lift,
                phrase.accent,
                phrase.travel_speed,
                phrase.phase,
                render_time,
            ]
            .iter()
            .all(|value| value.is_finite())
                && phase_step > 0.0
                && phase_step <= dt * 1.8 + 0.000_01
                && render_step >= 0.0
                && render_step <= dt * motion_rate(visual.motion) * 1.8 + 0.000_01,
            "phrase sequence contains a non-finite value or a travel discontinuity at frame {frame}"
        );
        sequence.max_phase_step = sequence.max_phase_step.max(phase_step);
        sequence.max_render_step = sequence.max_render_step.max(render_step);
        if frame <= 16 * PHRASE_HZ {
            let probe = reset_probe.update_mapped(audio, mappings, dt);
            anyhow::ensure!(probe == phrase, "phrase sequence is not deterministic");
            if frame == 16 * PHRASE_HZ {
                sequence.reset_before = probe;
                reset_probe.reset();
                let after = reset_probe.update_mapped(quiet, mappings, 0.0);
                anyhow::ensure!(
                    after.phase == probe.phase && after.travel_speed == probe.travel_speed,
                    "source reset changes integrated phase or speed"
                );
                sequence.reset_after = after;
            }
        }
        let stage = match frame {
            frame if frame == 8 * PHRASE_HZ => "after 8 seconds quiet",
            frame if frame == 16 * PHRASE_HZ => "after 8 seconds loud",
            frame if frame == 28 * PHRASE_HZ => "after 12 seconds silence",
            _ => continue,
        };
        let paused_phrase = controller.update_mapped(audio, mappings, 0.0);
        let paused_time = render_time + 0.0 * visual.motion * paused_phrase.travel_speed;
        anyhow::ensure!(
            paused_phrase == phrase && paused_time == render_time,
            "zero-delta update changes phrase or integrated render time"
        );
        sequence.snapshots.push(PhraseSnapshot {
            seconds: frame / PHRASE_HZ,
            stage,
            audio: audio.clone(),
            phrase,
            paused_phrase,
            render_time,
        });
    }
    Ok(sequence)
}

fn phrase_json(phrase: Choreography) -> serde_json::Value {
    serde_json::json!({
        "drive": phrase.drive,
        "openness": phrase.openness,
        "lift": phrase.lift,
        "accent": phrase.accent,
        "travel_speed": phrase.travel_speed,
        "integrated_phase": phrase.phase
    })
}

pub(super) fn write_contact_sheet(
    path: &Path,
    frames: &[Vec<u8>],
    columns: usize,
) -> anyhow::Result<()> {
    anyhow::ensure!(!frames.is_empty() && columns > 0, "empty contact sheet");
    let rows = frames.len().div_ceil(columns);
    let width = SIZE[0] as usize * columns;
    let height = SIZE[1] as usize * rows;
    let mut sheet = vec![0_u8; width * height * 4];
    for (index, pixels) in frames.iter().enumerate() {
        anyhow::ensure!(
            pixels.len() == (SIZE[0] * SIZE[1] * 4) as usize,
            "contact sheet frame has unexpected dimensions"
        );
        for row in 0..SIZE[1] as usize {
            let source = row * SIZE[0] as usize * 4;
            let target = ((index / columns * SIZE[1] as usize + row) * width
                + index % columns * SIZE[0] as usize)
                * 4;
            sheet[target..target + SIZE[0] as usize * 4]
                .copy_from_slice(&pixels[source..source + SIZE[0] as usize * 4]);
        }
    }
    write_png(path, &sheet, [width as u32, height as u32])
}

fn dark_fraction(pixels: &[u8]) -> f64 {
    let dark = pixels
        .chunks_exact(4)
        .filter(|pixel| {
            // Display-value luma is a contrast proxy, not a radiometric claim.
            display_luma(pixel) < DARK_LUMA
        })
        .count();
    dark as f64 / (pixels.len() / 4).max(1) as f64
}

fn display_luma(pixel: &[u8]) -> f64 {
    0.2126 * pixel[0] as f64 + 0.7152 * pixel[1] as f64 + 0.0722 * pixel[2] as f64
}

fn luma_deviation(pixels: &[u8]) -> f64 {
    // Variation across pixels, unlike a flattened RGB deviation that a
    // completely uniform but strongly colored image could satisfy.
    let (sum, squares, count) =
        pixels
            .chunks_exact(4)
            .fold((0.0, 0.0, 0.0_f64), |(sum, squares, count), pixel| {
                let value = display_luma(pixel);
                (sum + value, squares + value * value, count + 1.0)
            });
    let count = count.max(1.0);
    (squares / count - (sum / count).powi(2)).max(0.0).sqrt()
}

pub(super) fn render_frame(
    renderer: &mut VisualResources,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    uniforms: ShaderUniforms,
    size: [u32; 2],
) -> anyhow::Result<Vec<u8>> {
    let mut encoder = device.create_command_encoder(&Default::default());
    renderer.render(device, queue, &mut encoder, uniforms, size);
    queue.submit([encoder.finish()]);
    let history = renderer.history.as_ref().expect("rendered history");
    read_texture(device, queue, &history._textures[history.current], size)
}

fn read_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    size: [u32; 2],
) -> anyhow::Result<Vec<u8>> {
    let row_bytes = (size[0] * 4).div_ceil(256) * 256;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("verification readback"),
        size: row_bytes as u64 * size[1] as u64,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_bytes),
                rows_per_image: Some(size[1]),
            },
        },
        wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let (tx, rx) = mpsc::sync_channel(1);
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(Duration::from_secs(20)),
    })?;
    rx.recv_timeout(Duration::from_secs(2))??;
    let mapped = buffer.slice(..).get_mapped_range()?;
    let mut pixels = Vec::with_capacity((size[0] * size[1] * 4) as usize);
    for row in mapped.chunks_exact(row_bytes as usize) {
        pixels.extend_from_slice(&row[..size[0] as usize * 4]);
    }
    drop(mapped);
    buffer.unmap();
    Ok(pixels)
}

pub(super) fn write_png(path: &Path, pixels: &[u8], size: [u32; 2]) -> anyhow::Result<()> {
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(fs::File::create(path)?),
        size[0],
        size[1],
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(pixels)?;
    Ok(())
}

fn stats(pixels: &[u8]) -> (f64, f64) {
    let values: Vec<f64> = pixels
        .chunks_exact(4)
        .flat_map(|pixel| pixel[..3].iter().map(|value| *value as f64))
        .collect();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64;
    (mean, variance.sqrt())
}

pub(super) fn difference(a: &[u8], b: &[u8]) -> f64 {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .flat_map(|(a, b)| (0..3).map(move |index| (a[index] as f64 - b[index] as f64).abs()))
        .sum::<f64>()
        / (a.len() / 4 * 3) as f64
}

#[cfg(test)]
mod probe_tests {
    use super::*;

    #[test]
    fn landscape_probe_validates_and_cannot_read_audio_or_background() {
        let module = wgpu::naga::front::wgsl::parse_str(LANDSCAPE_PROBE_SHADER)
            .unwrap_or_else(|error| panic!("{}", error.emit_to_string(LANDSCAPE_PROBE_SHADER)));
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(LANDSCAPE_PROBE_SHADER)));
        // A single texture resource: no audio uniforms, previous-frame input,
        // sampler or external background can influence this diagnostic.
        assert_eq!(module.global_variables.len(), 1);
        let (_, geometry) = module.global_variables.iter().next().unwrap();
        let binding = geometry.binding.as_ref().unwrap();
        assert_eq!((binding.group, binding.binding), (0, 3));
        assert!(LANDSCAPE_PROBE_SHADER.contains("position.xy), 2, 0)"));
    }
}
