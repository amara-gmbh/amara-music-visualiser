//! Small production-GPU gallery and musical-routing check for Northern Lights.
//! Synthetic features only: no capture permissions or personal audio involved.
use super::{
    AudioFeatures, HISTORY_FORMAT, LOOKS, ShaderUniforms, VisualResources, WORLD_COUNT,
    verification::{difference, render_frame, write_contact_sheet, write_png},
    wgpu,
};
use crate::presets::AudioMappings;
use anyhow::Context;
use std::{fs, path::Path};

const SIZE: [u32; 2] = [640, 400];
const AURORA: usize = 9;

pub fn preview_aurora(destination: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
    let report = destination.join("aurora-report.json");
    fs::write(&report, br#"{"status":"incomplete","input":"synthetic"}"#)?;
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
    .context("no GPU adapter available for Northern Lights preview")?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("amara aurora preview"),
        ..Default::default()
    }))?;
    let mut renderer = VisualResources::new(&device, HISTORY_FORMAT);
    renderer.specialize(&device, &[1 << AURORA]);
    let mut profile = crate::presets::built_in_profiles()
        .into_iter()
        .find(|profile| profile.visual.scene == AURORA)
        .context("Northern Lights preset missing")?;
    profile.visual.feedback = 0.0;
    let music = AudioFeatures {
        bass: 0.68,
        mids: 0.57,
        treble: 0.48,
        energy: 0.60,
        spectrum: (0..48)
            .map(|bin| 0.12 + 0.55 * (bin as f32 * 0.37 + 0.21).sin().powi(2))
            .collect(),
        ..Default::default()
    };
    let uniforms = |from: usize,
                    to: usize,
                    blend: f32,
                    time: f32,
                    audio: &AudioFeatures,
                    mappings: &AudioMappings| {
        let mut looks = [[0.0, 0.0, 1.0, 0.0]; WORLD_COUNT];
        looks[AURORA] = [from as f32, to as f32, blend, time];
        ShaderUniforms::new(
            time,
            1.0 / 60.0,
            AURORA,
            AURORA,
            1.0,
            &profile.visual,
            mappings,
            audio,
        )
        .with_looks(looks)
        .with_audio_state(true, 1)
    };
    let mut draw = |input, size| {
        renderer.history = None;
        render_frame(&mut renderer, &device, &queue, input, size)
    };
    let mut looks = Vec::new();
    let mut look_reports = Vec::new();
    for (look, name) in LOOKS[AURORA].iter().enumerate() {
        let frame = draw(
            uniforms(look, look, 1.0, 12.0, &music, &profile.mappings),
            SIZE,
        )?;
        let quiet = draw(
            uniforms(
                look,
                look,
                1.0,
                12.0,
                &AudioFeatures::default(),
                &profile.mappings,
            ),
            SIZE,
        )?;
        let response = difference(&quiet, &frame);
        anyhow::ensure!(
            response > 0.5,
            "{name}: music does not visibly reveal curtains"
        );
        write_png(
            &destination.join(format!("aurora-look-{look}.png")),
            &frame,
            SIZE,
        )?;
        look_reports.push(serde_json::json!({"look":look,"name":name,"music_difference":response}));
        looks.push(frame);
    }
    write_contact_sheet(&destination.join("aurora-ten-looks.png"), &looks, 5)?;
    // The two wings must be judged beyond a single favourable clock position.
    // These are composition evidence, not a demand that a natural curtain keep
    // an invariant brightness centroid while travelling through the sky.
    let mut glacial_times = Vec::new();
    let mut glacial_frames = Vec::new();
    for time in [0.0, 8.0, 16.0, 24.0, 32.0, 40.0, 60.0, 90.0, 120.0, 180.0] {
        let frame = draw(uniforms(6, 6, 1.0, time, &music, &profile.mappings), SIZE)?;
        write_png(
            &destination.join(format!("aurora-glacial-time-{time:03.0}.png")),
            &frame,
            SIZE,
        )?;
        glacial_times.push(time);
        glacial_frames.push(frame);
    }
    write_contact_sheet(
        &destination.join("aurora-glacial-time-series.png"),
        &glacial_frames,
        5,
    )?;
    let hero = draw(
        uniforms(0, 0, 1.0, 12.0, &music, &profile.mappings),
        [1200, 750],
    )?;
    write_png(&destination.join("aurora-hero.png"), &hero, [1200, 750])?;
    let quiet = AudioFeatures::default();
    let zero = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let off_quiet = draw(uniforms(0, 0, 1.0, 12.0, &quiet, &zero), SIZE)?;
    let off_music = draw(uniforms(0, 0, 1.0, 12.0, &music, &zero), SIZE)?;
    anyhow::ensure!(
        difference(&off_quiet, &off_music) == 0.0,
        "zero audio mappings leak into aurora"
    );
    let baseline = draw(uniforms(0, 0, 1.0, 12.0, &quiet, &profile.mappings), SIZE)?;
    let mut responses = vec![baseline.clone()];
    let mut bands = Vec::new();
    for (label, start, end) in [("bass", 0, 8), ("mids", 8, 24), ("highs", 24, 48)] {
        let mut isolated = AudioFeatures::default();
        for bin in start..end {
            isolated.spectrum[bin] = 0.22 + 0.48 * (bin as f32 * 0.51).sin().powi(2);
        }
        let frame = draw(
            uniforms(0, 0, 1.0, 12.0, &isolated, &profile.mappings),
            SIZE,
        )?;
        let response = difference(&baseline, &frame);
        anyhow::ensure!(response > 0.08, "aurora ignores isolated {label} FFT rows");
        let mut disabled = profile.mappings.clone();
        match start {
            0 => disabled.bass_to_pulse = 0.0,
            8 => disabled.mids_to_geometry = 0.0,
            _ => disabled.treble_to_sparkle = 0.0,
        }
        let disabled_baseline = draw(uniforms(0, 0, 1.0, 12.0, &quiet, &disabled), SIZE)?;
        let disabled_music = draw(uniforms(0, 0, 1.0, 12.0, &isolated, &disabled), SIZE)?;
        let disconnected = difference(&disabled_baseline, &disabled_music);
        anyhow::ensure!(
            disconnected == 0.0,
            "aurora {label} leaks through another mapping"
        );
        bands.push(
            serde_json::json!({"band":label,"fft_only_difference":response,
            "disabled_difference":disconnected}),
        );
        responses.push(frame);
    }
    write_contact_sheet(
        &destination.join("aurora-frequency-response.png"),
        &responses,
        4,
    )?;
    let next = draw(
        uniforms(0, 0, 1.0, 12.0 + 1.0 / 60.0, &music, &profile.mappings),
        SIZE,
    )?;
    let later = draw(uniforms(0, 0, 1.0, 12.5, &music, &profile.mappings), SIZE)?;
    let frame_step = difference(&looks[0], &next);
    let motion = difference(&looks[0], &later);
    anyhow::ensure!(
        frame_step < 6.0 && motion > 0.1,
        "aurora motion is static or jumps"
    );
    let mut morph = Vec::new();
    for blend in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let frame = draw(uniforms(0, 8, blend, 12.0, &music, &profile.mappings), SIZE)?;
        if blend == 0.0 {
            anyhow::ensure!(frame == looks[0], "aurora source look endpoint changed");
        }
        if blend == 1.0 {
            anyhow::ensure!(frame == looks[8], "aurora target look endpoint changed");
        }
        morph.push(frame);
    }
    write_contact_sheet(&destination.join("aurora-look-morph.png"), &morph, 5)?;
    let mut long_session_steps = Vec::new();
    for time in [600.0, 1800.0, 7200.0] {
        let before = draw(uniforms(0, 8, 0.5, time, &music, &profile.mappings), SIZE)?;
        let after = draw(
            uniforms(
                0,
                8,
                0.5 + 1.0 / 360.0,
                time + 1.0 / 60.0,
                &music,
                &profile.mappings,
            ),
            SIZE,
        )?;
        let step = difference(&before, &after);
        anyhow::ensure!(
            step < 3.0,
            "aurora look morph jumps after {time}s ({step:.3})"
        );
        long_session_steps.push(serde_json::json!({"time":time,"one_frame_morph_step":step}));
    }
    fs::write(
        &report,
        serde_json::to_vec_pretty(&serde_json::json!({
            "status":"pass", "adapter":info.name, "world":AURORA,
            "looks":look_reports, "bands":bands, "zero_mappings_difference":0.0,
            "one_frame_mean_rgb_step":frame_step, "half_second_mean_rgb_motion":motion,
            "look_endpoints":"pixel exact",
            "long_session_look_morphs":long_session_steps,
            "glacial_wings_composition_times":glacial_times,
            "scope":"Synthetic production-GPU frames, ten looks, FFT routes, mapping disconnection, small time step and look endpoints. Not live input, display FPS or continuous runtime acceptance."
        }))?,
    )?;
    println!("Northern Lights previews passed on {}", info.name);
    Ok(())
}
