//! Focused production-GPU previews of Water Garden. Only synthetic features
//! are used; this is not a capture-permission, frame-rate or listening test.
use super::{
    AudioFeatures, HISTORY_FORMAT, LOOKS, ShaderUniforms, VisualResources, WORLD_COUNT,
    verification::{difference, render_frame, write_contact_sheet, write_png},
    wgpu,
};
use crate::{
    presets::{AudioMappings, VisualPalette, VisualSettings},
    ripples::RIPPLE_COUNT,
};
use anyhow::Context;
use std::{fs, path::Path};

const SIZE: [u32; 2] = [640, 400];
const HERO_SIZE: [u32; 2] = [1000, 700];
const WATER: usize = 8;
const PRIMARY_EVENTS: usize = 12;

fn music() -> AudioFeatures {
    AudioFeatures {
        bass: 0.78,
        mids: 0.58,
        treble: 0.46,
        energy: 0.64,
        beat: 0.45,
        transient: 0.55,
        spectrum: (0..48)
            .map(|i| {
                let x = i as f32 / 47.0;
                (0.06
                    + 0.65 * (-((x - 0.08) / 0.09).powi(2)).exp()
                    + 0.40 * (-((x - 0.36) / 0.12).powi(2)).exp()
                    + 0.23 * (i as f32 * 1.41).sin().powi(2))
                .clamp(0.0, 1.0)
            })
            .collect(),
        ..Default::default()
    }
}

fn events() -> [[f32; 4]; RIPPLE_COUNT] {
    let primary = [
        [-0.27, -0.08, 0.75, 0.95],
        [0.18, 0.14, 1.35, 0.80],
        [0.06, -0.17, 2.10, 0.60],
        [-0.15, 0.18, 3.05, 0.45],
        [0.37, -0.31, 0.43, 0.68],
        [-0.42, 0.28, 1.10, 0.55],
        [0.26, 0.36, 1.90, 0.45],
        [-0.32, -0.20, 2.55, 0.35],
        [0.60, 0.20, 0.32, 0.60],
        [-0.55, -0.35, 0.80, 0.50],
        [0.35, -0.48, 1.35, 0.40],
        [-0.20, 0.45, 2.25, 0.30],
    ];
    let mut events = [[0.0; 4]; RIPPLE_COUNT];
    events[..PRIMARY_EVENTS].copy_from_slice(&primary);
    events
}

fn uniforms(
    look: usize,
    visual: &VisualSettings,
    mappings: &AudioMappings,
    audio: &AudioFeatures,
    ripples: [[f32; 4]; RIPPLE_COUNT],
) -> ShaderUniforms {
    let mut looks = [[0.0, 0.0, 1.0, 0.0]; WORLD_COUNT];
    looks[WATER] = [look as f32, look as f32, 1.0, 0.0];
    ShaderUniforms::new(12.0, 1.0 / 60.0, WATER, WATER, 1.0, visual, mappings, audio)
        .with_looks(looks)
        .with_audio_state(true, 1)
        .with_ripples(ripples)
}

fn frame_metrics(pixels: &[u8]) -> (f64, f64) {
    let mut sum = 0.0;
    let mut squares = 0.0;
    let count = (pixels.len() / 4).max(1) as f64;
    for p in pixels.chunks_exact(4) {
        let luma = 0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2]);
        sum += luma;
        squares += luma * luma;
    }
    let mean = sum / count;
    (mean, (squares / count - mean * mean).max(0.0).sqrt())
}

fn changed_pixel_fraction(a: &[u8], b: &[u8]) -> f64 {
    let changed = a
        .chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(a, b)| (0..3).any(|channel| a[channel].abs_diff(b[channel]) >= 2))
        .count();
    changed as f64 / (a.len() / 4).max(1) as f64
}

fn low_level_audio() -> AudioFeatures {
    // Keep non-spectrum scalars identical across the band probes, including
    // audible energy. A scalar bass/mid/air fallback must not pass an FFT test.
    AudioFeatures {
        energy: 0.08,
        ..AudioFeatures::default()
    }
}

fn low_level_spectrum(start: usize, end: usize, peak: f32) -> AudioFeatures {
    let mut audio = low_level_audio();
    for bin in start..end {
        let shape = 0.35 + 0.65 * (bin as f32 * 1.13 + 0.20).sin().abs();
        audio.spectrum[bin] = peak * shape;
    }
    audio
}

fn verify_low_level_bands(
    destination: &Path,
    visual: &VisualSettings,
    mappings: &AudioMappings,
    draw: &mut impl FnMut(ShaderUniforms, [u32; 2]) -> anyhow::Result<Vec<u8>>,
) -> anyhow::Result<serde_json::Value> {
    let no_events = [[0.0; 4]; RIPPLE_COUNT];
    let baseline = low_level_audio();
    let mut reports = Vec::new();
    let mut failures = Vec::new();
    let mut scalar_frames = Vec::new();
    for (look, label) in [(7, "mercury"), (6, "cross-current")] {
        let quiet_uniforms = uniforms(look, visual, mappings, &baseline, no_events);
        let quiet_pixels = draw(quiet_uniforms, SIZE)?;
        let mut later = quiet_uniforms;
        later.values[0][2] = 35.0;
        let later_quiet = draw(later, SIZE)?;
        if quiet_pixels != later_quiet {
            failures.push(format!(
                "{label}: zero-spectrum/no-impact water moves in silence"
            ));
        }
        let mut rows = Vec::new();
        let mut levels = Vec::new();
        for peak in [0.05_f32, 0.10, 0.20] {
            rows.push(quiet_pixels.clone());
            for (band, name, start, end) in
                [(0, "bass", 0, 8), (1, "mids", 8, 24), (2, "treble", 24, 48)]
            {
                let audio = low_level_spectrum(start, end, peak);
                let pixels = draw(uniforms(look, visual, mappings, &audio, no_events), SIZE)?;
                let mean_change = difference(&pixels, &quiet_pixels);
                let changed_fraction = changed_pixel_fraction(&pixels, &quiet_pixels);
                // Require distributed visible variation, not one changed LSB.
                // This is a renderer regression floor, not a perception score.
                if mean_change <= 0.15 || changed_fraction <= 0.02 {
                    failures.push(format!("{label} {name} FFT peak {peak:.2}: mean delta {mean_change:.5}, changed fraction {changed_fraction:.5}"));
                }
                let mut gated = mappings.clone();
                match band {
                    0 => gated.bass_to_pulse = 0.0,
                    1 => gated.mids_to_geometry = 0.0,
                    _ => gated.treble_to_sparkle = 0.0,
                }
                let gate_quiet = draw(uniforms(look, visual, &gated, &baseline, no_events), SIZE)?;
                let gate_signal = draw(uniforms(look, visual, &gated, &audio, no_events), SIZE)?;
                let gate_exact = gate_quiet == gate_signal;
                if !gate_exact {
                    failures.push(format!(
                        "{label} {name} FFT peak {peak:.2}: zero mapping leaks signal"
                    ));
                }
                levels.push(serde_json::json!({"band": name, "peak_limit": peak,
                    "mean_rgb_difference": mean_change, "fraction_pixels_changed_at_least_two_codes": changed_fraction,
                    "zero_mapping_pixel_exact": gate_exact}));
                rows.push(pixels);
            }
        }
        write_contact_sheet(
            &destination.join(format!("water-low-level-{label}.png")),
            &rows,
            4,
        )?;

        // A full-band test alone can miss sampling blind spots. Each individual
        // FFT bin must reach the production image, including all 40 upper bins.
        // Nonzero here proves coverage only; legibility is checked above.
        let mut bin_reports = Vec::new();
        let mut bin_edges = Vec::new();
        for bin in 0..48 {
            let mut audio = baseline.clone();
            audio.spectrum[bin] = 0.20;
            let pixels = draw(uniforms(look, visual, mappings, &audio, no_events), SIZE)?;
            let changed = pixels != quiet_pixels;
            if !changed {
                failures.push(format!("{label}: isolated FFT bin {bin} is invisible"));
            }
            bin_reports.push(serde_json::json!({"bin": bin, "input": 0.20,
                "mean_rgb_difference": difference(&pixels, &quiet_pixels),
                "fraction_pixels_changed_at_least_two_codes": changed_pixel_fraction(&pixels, &quiet_pixels),
                "production_pixels_changed": changed}));
            if [0, 7, 8, 12, 13, 17, 18, 23, 24, 29, 30, 35, 36, 41, 42, 47].contains(&bin) {
                bin_edges.push(pixels);
            }
        }
        write_contact_sheet(
            &destination.join(format!("water-bin-edges-{label}.png")),
            &bin_edges,
            4,
        )?;

        scalar_frames.push(quiet_pixels.clone());
        let mut scalar_reports = Vec::new();
        for (name, channel) in [("mids", 1), ("treble", 2)] {
            let mut audio = baseline.clone();
            if channel == 1 {
                audio.mids = 0.10;
            } else {
                audio.treble = 0.10;
            }
            let pixels = draw(uniforms(look, visual, mappings, &audio, no_events), SIZE)?;
            let mean_change = difference(&pixels, &quiet_pixels);
            let changed_fraction = changed_pixel_fraction(&pixels, &quiet_pixels);
            if mean_change <= 0.15 || changed_fraction <= 0.02 {
                failures.push(format!("{label}: scalar-only {name} .10 is too weak ({mean_change:.5}, {changed_fraction:.5})"));
            }
            let mut gated = mappings.clone();
            if channel == 1 {
                gated.mids_to_geometry = 0.0;
            } else {
                gated.treble_to_sparkle = 0.0;
            }
            let gate_quiet = draw(uniforms(look, visual, &gated, &baseline, no_events), SIZE)?;
            let gate_signal = draw(uniforms(look, visual, &gated, &audio, no_events), SIZE)?;
            let gate_exact = gate_quiet == gate_signal;
            if !gate_exact {
                failures.push(format!(
                    "{label}: scalar-only {name} leaks through zero mapping"
                ));
            }
            scalar_reports.push(serde_json::json!({"band": name, "scalar_input": 0.10, "spectrum": "all zero",
                "mean_rgb_difference": mean_change, "fraction_pixels_changed_at_least_two_codes": changed_fraction,
                "zero_mapping_pixel_exact": gate_exact}));
            scalar_frames.push(pixels);
        }
        reports.push(serde_json::json!({"look": look, "name": LOOKS[WATER][look],
            "quiet_time_pixel_exact": quiet_pixels == later_quiet,
            "levels": levels, "single_bin_coverage": bin_reports, "scalar_only": scalar_reports}));
    }
    write_contact_sheet(
        &destination.join("water-scalar-upper-bands.png"),
        &scalar_frames,
        3,
    )?;
    let report = serde_json::json!({
        "status": if failures.is_empty() { "pass" } else { "fail" },
        "looks": reports, "failures": failures,
        "sheet_layout": "Each look: rows .05/.10/.20 peak-limited uneven FFT; columns quiet/bass/mids/treble. Scalar sheet: Mercury then Cross Current rows; columns quiet/mids-only/treble-only. Bin-edge sheets follow 0,7,8,12,13,17,18,23,24,29,30,35,36,41,42,47.",
        "scope": "Synthetic low-amplitude normalized feature coverage. Low-band/scalar probes require mean RGB delta >.15 and >2% of pixels changing by >=2 display codes; single-bin probes establish only nonzero coverage. No claim of real-track recognition, native latency, runtime frame cadence or perceived smoothness."
    });
    // Preserve every diagnostic row and measurement if this regression fails.
    fs::write(
        destination.join("water-low-level-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    anyhow::ensure!(
        failures.is_empty(),
        "low-level water regressions: {}",
        failures.join("; ")
    );
    Ok(report)
}

pub fn preview_water(destination: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
    let report_path = destination.join("water-report.json");
    fs::write(
        &report_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "status": "incomplete", "input": "synthetic audio features and impact records"
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
    .context("no GPU adapter available for the Water Garden preview")?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("amara water verification"),
        ..Default::default()
    }))?;
    let mut renderer = VisualResources::new(&device, HISTORY_FORMAT);
    let profile = crate::presets::built_in_profiles()
        .into_iter()
        .find(|profile| profile.visual.scene == WATER)
        .context("Water Garden preset missing")?;
    let mut visual = profile.visual;
    visual.feedback = 0.0;
    let audio = music();
    let ripples = events();
    anyhow::ensure!(
        ripples.iter().flatten().all(|value| value.is_finite())
            && audio.spectrum.iter().all(|value| value.is_finite()),
        "synthetic water input contains a non-finite value"
    );
    let mut draw = |uniforms: ShaderUniforms, size| {
        renderer.history = None;
        render_frame(&mut renderer, &device, &queue, uniforms, size)
    };
    let mut frames = Vec::new();
    let mut looks_report = Vec::new();
    for (look, name) in LOOKS[WATER].iter().enumerate() {
        let pixels = draw(
            uniforms(look, &visual, &profile.mappings, &audio, ripples),
            SIZE,
        )?;
        write_png(
            &destination.join(format!("water-look-{look}.png")),
            &pixels,
            SIZE,
        )?;
        let (mean, spatial) = frame_metrics(&pixels);
        anyhow::ensure!(
            mean.is_finite() && spatial.is_finite(),
            "water look {look}: invalid image statistics"
        );
        anyhow::ensure!(
            mean > 2.0 && mean < 245.0 && spatial > 2.0,
            "water look {look}: black, clipped or spatially flat output ({mean:.3}, {spatial:.3})"
        );
        looks_report.push(serde_json::json!({
            "look": look, "name": name,
            "mean_display_luma": mean, "spatial_luma_deviation": spatial
        }));
        frames.push(pixels);
    }
    write_contact_sheet(&destination.join("water-ten-looks.png"), &frames, 5)?;
    // The material must remain colourful to the screen edge: no aperture,
    // oval pool or dark outer frame. Sustained musical detail should leave
    // genuinely calmer coloured areas between active currents. Compare music
    // with silence at the SAME time, without impact packets, so the activity
    // measure describes sustained texture rather than randomly placed splashes.
    let mut spatial_report = Vec::new();
    let no_impacts = [[0.0; 4]; RIPPLE_COUNT];
    for (label, size, offset) in [
        ("wide", [640, 400], 0.0),
        ("sidebar", [640, 400], -0.42),
        ("portrait", [400, 640], 0.0),
    ] {
        for look in 0..LOOKS[WATER].len() {
            let input = uniforms(look, &visual, &profile.mappings, &audio, no_impacts)
                .with_canvas_centre(offset);
            let early = draw(input, size)?;
            let quiet = draw(
                uniforms(
                    look,
                    &visual,
                    &profile.mappings,
                    &AudioFeatures::default(),
                    no_impacts,
                )
                .with_canvas_centre(offset),
                size,
            )?;
            let mut later_input = input;
            later_input.values[0][2] += 0.75;
            let later = draw(later_input, size)?;
            let margin_x = size[0] / 16;
            let margin_y = size[1] / 16;
            let mut perimeter_count = 0_u32;
            let mut quiet_perimeter_light = 0.0_f64;
            let mut musical_perimeter_light = 0.0_f64;
            let mut perimeter_chroma = 0.0_f64;
            let mut calm_pixels = 0_u32;
            let mut active_pixels = 0_u32;
            for y in 0..size[1] {
                for x in 0..size[0] {
                    let pixel = ((y * size[0] + x) * 4) as usize;
                    let change = (0..3)
                        .map(|channel| early[pixel + channel].abs_diff(quiet[pixel + channel]))
                        .max()
                        .unwrap();
                    calm_pixels += u32::from(change <= 6);
                    active_pixels += u32::from(change >= 12);
                    if x < margin_x
                        || x >= size[0] - margin_x
                        || y < margin_y
                        || y >= size[1] - margin_y
                    {
                        perimeter_count += 1;
                        let luma = |frame: &[u8]| {
                            0.2126 * f64::from(frame[pixel])
                                + 0.7152 * f64::from(frame[pixel + 1])
                                + 0.0722 * f64::from(frame[pixel + 2])
                        };
                        quiet_perimeter_light += luma(&quiet);
                        musical_perimeter_light += luma(&early);
                        let channels = &quiet[pixel..pixel + 3];
                        perimeter_chroma += f64::from(
                            channels.iter().max().unwrap() - channels.iter().min().unwrap(),
                        );
                    }
                }
            }
            let perimeter = f64::from(perimeter_count.max(1));
            quiet_perimeter_light /= perimeter;
            musical_perimeter_light /= perimeter;
            perimeter_chroma /= perimeter;
            let count = f64::from(size[0] * size[1]);
            let calm_fraction = f64::from(calm_pixels) / count;
            let active_fraction = f64::from(active_pixels) / count;
            let movement = difference(&early, &later);
            anyhow::ensure!(
                quiet_perimeter_light > 24.0
                    && perimeter_chroma > 14.0
                    && musical_perimeter_light > quiet_perimeter_light * 0.60,
                "water look {look} {label}: colour lost at the perimeter (quiet {quiet_perimeter_light:.3}, music {musical_perimeter_light:.3}, chroma {perimeter_chroma:.3})"
            );
            anyhow::ensure!(
                calm_fraction > 0.10 && active_fraction > 0.02 && active_fraction < 0.88,
                "water look {look} {label}: sustained ripples leave insufficient calm colour or no active detail (calm {calm_fraction:.3}, active {active_fraction:.3})"
            );
            anyhow::ensure!(
                movement > 0.1,
                "water look {look} {label}: sparse currents stopped moving"
            );
            if look == 6 || look == 7 {
                write_png(
                    &destination.join(format!("water-currents-{label}-{look}.png")),
                    &early,
                    size,
                )?;
            }
            spatial_report.push(serde_json::json!({"look":look,"layout":label,
                "quiet_perimeter_mean_luma":quiet_perimeter_light,
                "musical_perimeter_mean_luma":musical_perimeter_light,
                "quiet_perimeter_mean_chroma":perimeter_chroma,
                "calm_coloured_fraction":calm_fraction,"active_detail_fraction":active_fraction,
                "current_motion_mean_delta":movement}));
        }
    }
    fs::write(
        destination.join("water-spatial-activity-report.json"),
        serde_json::to_vec_pretty(&spatial_report)?,
    )?;
    let mut background_frames = Vec::new();
    let mut background_reports = Vec::new();
    for look in [0, 7, 6] {
        let input = uniforms(look, &visual, &profile.mappings, &audio, ripples);
        let original = draw(input.with_background([0.0; 4]), SIZE)?;
        anyhow::ensure!(
            original == frames[look],
            "water look {look}: explicit World Colours changed the default image"
        );
        let green = draw(input.with_background([0.04, 0.30, 0.12, 0.45]), SIZE)?;
        let purple = draw(input.with_background([0.35, 0.03, 0.15, 0.45]), SIZE)?;
        let green_change = difference(&original, &green);
        let purple_change = difference(&original, &purple);
        let colour_change = difference(&green, &purple);
        anyhow::ensure!(
            green_change > 0.1 && purple_change > 0.1 && colour_change > 0.1,
            "water look {look}: manual background colours do not change the image"
        );
        write_png(
            &destination.join(format!("water-look-{look}-background-green.png")),
            &green,
            SIZE,
        )?;
        write_png(
            &destination.join(format!("water-look-{look}-background-purple.png")),
            &purple,
            SIZE,
        )?;
        background_frames.extend([original, green, purple]);
        background_reports.push(serde_json::json!({"look": look, "name": LOOKS[WATER][look],
            "world_colours_matches_default": "pixel-exact",
            "green_vs_default_mean_rgb_difference": green_change,
            "purple_vs_default_mean_rgb_difference": purple_change,
            "green_vs_purple_mean_rgb_difference": colour_change}));
    }
    write_contact_sheet(
        &destination.join("water-background-options.png"),
        &background_frames,
        3,
    )?;
    let hero = draw(
        uniforms(0, &visual, &profile.mappings, &audio, ripples),
        HERO_SIZE,
    )?;
    write_png(&destination.join("water-hero.png"), &hero, HERO_SIZE)?;
    let mut mocca = visual.clone();
    mocca.palette = VisualPalette::MoccaVanilla;
    let mocca_pixels = draw(
        uniforms(0, &mocca, &profile.mappings, &audio, ripples),
        SIZE,
    )?;
    write_png(&destination.join("water-mocca.png"), &mocca_pixels, SIZE)?;

    let quiet = AudioFeatures::default();
    let mut one_impact = [[0.0; 4]; RIPPLE_COUNT];
    one_impact[0] = [-0.12, 0.08, 0.55, 1.0];
    let early = draw(
        uniforms(0, &visual, &profile.mappings, &quiet, one_impact),
        SIZE,
    )?;
    one_impact[0][2] = 1.15;
    let late = draw(
        uniforms(0, &visual, &profile.mappings, &quiet, one_impact),
        SIZE,
    )?;
    write_png(&destination.join("water-impulse-early.png"), &early, SIZE)?;
    write_png(&destination.join("water-impulse-late.png"), &late, SIZE)?;
    let age_change = difference(&early, &late);
    anyhow::ensure!(
        age_change > 0.1,
        "aging the same water impact did not move the rendered surface"
    );

    let off = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let off_quiet = draw(
        uniforms(0, &visual, &off, &quiet, [[0.0; 4]; RIPPLE_COUNT]),
        SIZE,
    )?;
    let mut with_tails = ripples;
    with_tails[PRIMARY_EVENTS..].copy_from_slice(&ripples[..PRIMARY_EVENTS]);
    let off_music = draw(uniforms(0, &visual, &off, &audio, with_tails), SIZE)?;
    let disconnected_change = difference(&off_quiet, &off_music);
    anyhow::ensure!(
        off_quiet == off_music,
        "zero mappings still allow water audio or stored impacts through ({disconnected_change:.6})"
    );
    write_png(
        &destination.join("water-disconnected.png"),
        &off_quiet,
        SIZE,
    )?;

    let no_events = [[0.0; 4]; RIPPLE_COUNT];
    let spectrum_only = draw(
        uniforms(0, &visual, &profile.mappings, &audio, no_events),
        SIZE,
    )?;
    let mut no_spectrum = audio.clone();
    no_spectrum.spectrum.fill(0.0);
    let flat = draw(
        uniforms(0, &visual, &profile.mappings, &no_spectrum, no_events),
        SIZE,
    )?;
    let spectrum_change = difference(&spectrum_only, &flat);
    anyhow::ensure!(
        spectrum_change > 0.1,
        "water ignores the measured FFT contour"
    );
    write_png(
        &destination.join("water-spectrum-only.png"),
        &spectrum_only,
        SIZE,
    )?;
    // All scalar features, time, palette and look remain fixed. Only one
    // measured FFT region changes, so unrelated bass/beat shading cannot pass.
    let mut band_reports = Vec::new();
    let mut band_frames = vec![flat.clone()];
    for (name, start, end) in [("bass", 0, 8), ("mids", 8, 24), ("treble", 24, 48)] {
        let mut isolated = no_spectrum.clone();
        isolated.spectrum[start..end].copy_from_slice(&audio.spectrum[start..end]);
        let pixels = draw(
            uniforms(0, &visual, &profile.mappings, &isolated, no_events),
            SIZE,
        )?;
        let change = difference(&pixels, &flat);
        anyhow::ensure!(
            change > 0.1,
            "water ignores isolated {name} FFT bins ({change:.6})"
        );
        write_png(
            &destination.join(format!("water-fft-{name}.png")),
            &pixels,
            SIZE,
        )?;
        band_frames.push(pixels);
        band_reports.push(serde_json::json!({"band": name, "first_bin": start,
            "end_exclusive": end, "mean_rgb_difference_from_flat": change}));
    }
    write_contact_sheet(
        &destination.join("water-frequency-regions.png"),
        &band_frames,
        4,
    )?;
    let low_level_report =
        verify_low_level_bands(destination, &visual, &profile.mappings, &mut draw)?;

    // Retiring events must retain exactly the primary slot's frequency class.
    // Test each individual mapping, not just an all-zero escape condition.
    let mut tail_gate_reports = Vec::new();
    for (band, name) in ["bass", "mids", "treble"].into_iter().enumerate() {
        let mut gated = profile.mappings.clone();
        match band {
            0 => gated.bass_to_pulse = 0.0,
            1 => gated.mids_to_geometry = 0.0,
            _ => gated.treble_to_sparkle = 0.0,
        }
        let mut isolated = no_events;
        isolated[band * 4] = [-0.18, 0.12, 0.75, 1.0];
        isolated[PRIMARY_EVENTS + band * 4] = [0.28, -0.16, 1.20, 0.65];
        let reference = draw(uniforms(6, &visual, &gated, &quiet, no_events), SIZE)?;
        let with_packets = draw(uniforms(6, &visual, &gated, &quiet, isolated), SIZE)?;
        anyhow::ensure!(
            reference == with_packets,
            "water {name} mapping fails to disconnect primary and retiring packets"
        );
        tail_gate_reports.push(serde_json::json!({"band": name,
            "primary_and_tail_disconnect": "pixel-exact"}));
    }

    let mut recycling_reports = Vec::new();
    let mut terminal_reports = Vec::new();
    for (look, label) in [(7, "mercury"), (6, "cross-current")] {
        let old = [-0.18, 0.12, 0.75, 1.0];
        let mut before = no_events;
        before[0] = old;
        let before_pixels = draw(
            uniforms(look, &visual, &profile.mappings, &quiet, before),
            SIZE,
        )?;
        let mut recycled = no_events;
        recycled[PRIMARY_EVENTS] = old;
        recycled[0] = [0.32, -0.22, 0.0, 1.0];
        let recycled_pixels = draw(
            uniforms(look, &visual, &profile.mappings, &quiet, recycled),
            SIZE,
        )?;
        let recycling_change = difference(&before_pixels, &recycled_pixels);
        anyhow::ensure!(
            before_pixels == recycled_pixels,
            "water {label}: slot recycling changes pixels at zero-age birth ({recycling_change:.6})"
        );

        let cutoff = draw(
            uniforms(look, &visual, &profile.mappings, &quiet, no_events),
            SIZE,
        )?;
        // Both the primary packet and its optional delayed secondary have
        // reached exact zero by 6.12. Test every band and both storage pools,
        // at maximum strength, with no FFT/scalar evidence to mask leakage.
        let mut terminal = events();
        terminal[PRIMARY_EVENTS..].copy_from_slice(&ripples[..PRIMARY_EVENTS]);
        for event in &mut terminal {
            event[2] = 6.12;
            event[3] = 1.5;
        }
        let terminal_pixels = draw(
            uniforms(look, &visual, &profile.mappings, &quiet, terminal),
            SIZE,
        )?;
        let terminal_change = difference(&terminal_pixels, &cutoff);
        anyhow::ensure!(
            terminal_pixels == cutoff,
            "water {label}: expired primary/secondary/tail packets leak at age 6.12 ({terminal_change:.6})"
        );
        terminal_reports.push(serde_json::json!({"look": look, "name": LOOKS[WATER][look],
            "event_age": 6.12, "event_strength": 1.5, "active_fixture_slots": RIPPLE_COUNT,
            "mean_rgb_difference_from_quiet": terminal_change,
            "terminal_endpoint": "pixel-exact quiet",
            "scope": "Stored packet endpoint only; ongoing 6.00-to-6.06 secondary age advancement is covered by packing/controller unit tests, not inferred from potentially off-canvas GPU wavefronts."}));
        let old_cutoff_change = difference(&before_pixels, &cutoff);
        anyhow::ensure!(
            old_cutoff_change > 0.1,
            "water {label}: recycling fixture's old wave is not visibly present"
        );
        // Before plus seven 50ms checkpoints. This fixture follows the CPU's
        // 300ms C1 retirement envelope, at the user-reported slow Pace of .25.
        // It demonstrates shader slot continuity, not runtime frame cadence.
        let mut retirement_frames = vec![before_pixels];
        for seconds in [0.0_f32, 0.05, 0.10, 0.15, 0.20, 0.25, 0.30] {
            let t = seconds / 0.30;
            let mut aged = recycled;
            aged[0][2] = seconds * 0.25;
            aged[PRIMARY_EVENTS][2] = old[2] + seconds * 0.25;
            aged[PRIMARY_EVENTS][3] = old[3] * (1.0 - t * t * (3.0 - 2.0 * t));
            retirement_frames.push(draw(
                uniforms(look, &visual, &profile.mappings, &quiet, aged),
                SIZE,
            )?);
        }
        let steps: Vec<_> = retirement_frames
            .windows(2)
            .map(|pair| difference(&pair[0], &pair[1]))
            .collect();
        let midpoint_vs_cutoff = difference(&retirement_frames[4], &cutoff);
        anyhow::ensure!(
            midpoint_vs_cutoff > 0.1,
            "water {label}: retiring-wave midpoint lost all visible detail"
        );
        write_contact_sheet(
            &destination.join(format!("water-retirement-{label}.png")),
            &retirement_frames,
            4,
        )?;
        recycling_reports.push(serde_json::json!({"look": look, "name": LOOKS[WATER][look],
            "zero_age_recycling_mean_rgb_difference": recycling_change,
            "immediate_old_cutoff_mean_rgb_difference": old_cutoff_change,
            "retirement_midpoint_vs_empty_mean_rgb_difference": midpoint_vs_cutoff,
            "retirement_checkpoint_seconds": ["before", "0", ".05", ".10", ".15", ".20", ".25", ".30"],
            "adjacent_checkpoint_mean_rgb_differences": steps, "fixture_pace": 0.25}));
    }
    let mut low_detail = visual.clone();
    low_detail.complexity = 0.25;
    let lower = draw(
        uniforms(0, &low_detail, &profile.mappings, &audio, ripples),
        SIZE,
    )?;
    let detail_change = difference(&lower, &frames[0]);
    anyhow::ensure!(
        detail_change > 0.05,
        "water detail control does not affect the image"
    );
    write_contact_sheet(
        &destination.join("water-response.png"),
        &[early, late, off_quiet, spectrum_only, mocca_pixels],
        5,
    )?;
    let mut transitions = Vec::new();
    for (from, to) in [(2, 8), (8, 2), (3, 8), (8, 3)] {
        for progress in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let mut input = uniforms(0, &visual, &profile.mappings, &audio, ripples);
            input.values[0][3] = to as f32;
            input.values[5][2] = from as f32;
            input.values[5][3] = progress;
            let image = draw(input, SIZE)?;
            if progress == 0.0 || progress == 1.0 {
                let scene = if progress == 0.0 { from } else { to };
                input.values[0][3] = scene as f32;
                input.values[5][2] = scene as f32;
                input.values[5][3] = 1.0;
                let endpoint = draw(input, SIZE)?;
                anyhow::ensure!(
                    image == endpoint,
                    "water bridge endpoint changed {from}->{to} at {progress}"
                );
            }
            transitions.push(image);
        }
    }
    write_contact_sheet(&destination.join("water-field-morphs.png"), &transitions, 5)?;
    fs::write(
        &report_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "status": "pass", "adapter": info.name, "backend": format!("{:?}", info.backend),
            "input": "synthetic feature snapshots and recorded-impact fixtures; no personal audio",
            "looks": looks_report,
            "impact_age_mean_rgb_difference": age_change,
            "zero_mapping_mean_rgb_difference": disconnected_change,
            "fft_only_mean_rgb_difference": spectrum_change,
            "isolated_fft_regions": band_reports,
            "low_level_band_coverage": low_level_report,
            "retiring_packet_mapping_gates": tail_gate_reports,
            "recycling_continuity": recycling_reports,
            "terminal_packet_endpoints": terminal_reports,
            "background_options": background_reports,
            "full_colour_and_sparse_currents": spatial_report,
            "detail_mean_rgb_difference": detail_change,
            "field_bridge_endpoints": "exact, Water/Glass Tide and Water/Feedback in both directions",
            "scope": "Production Water Garden renderer with feedback reset per frame. Finite synthetic inputs and RGBA8 image statistics; ten looks nonblack and spatially varying; same-time impact aging, each low/mid/high FFT region, exact primary/tail mapping disconnect, pixel-exact birth/recycling in Mercury and Cross Current. Retirement contact sheets are synthetic 50ms checkpoints at Pace .25, not runtime frame pacing. Not a proof of every intermediate float, event detection, live audio permissions, frame rate, or perceived smoothness."
        }))?,
    )?;
    println!("Water Garden previews passed on {}", info.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_saturated_colour_cannot_satisfy_spatial_detail_metric() {
        let pixels = [255, 0, 0, 255].repeat(64);
        let (mean, spatial) = frame_metrics(&pixels);
        assert!(mean > 2.0);
        assert!(spatial < 0.0001);
        let varying = [[0, 0, 0, 255], [255, 255, 255, 255]].concat();
        assert!(frame_metrics(&varying).1 > 100.0);
    }

    #[test]
    fn water_preview_fixture_has_finite_distinct_events_in_each_frequency_pool() {
        let audio = music();
        assert_eq!(audio.spectrum.len(), 48);
        assert!(
            audio
                .spectrum
                .iter()
                .all(|x| x.is_finite() && *x >= 0.0 && *x <= 1.0)
        );
        let fixture = events();
        assert_eq!(RIPPLE_COUNT, PRIMARY_EVENTS * 2);
        assert!(
            fixture[PRIMARY_EVENTS..]
                .iter()
                .all(|event| *event == [0.0; 4])
        );
        for pool in fixture[..PRIMARY_EVENTS].chunks_exact(4) {
            assert!(pool.iter().flatten().all(|x| x.is_finite()));
            assert!(pool.iter().all(|event| event[2] > 0.0 && event[3] > 0.0));
            assert!(pool.windows(2).all(|pair| pair[0][2] != pair[1][2]));
        }
    }

    #[test]
    fn low_level_fft_fixture_is_bounded_and_has_no_scalar_band_or_impulse_evidence() {
        for (start, end) in [(0, 8), (8, 24), (24, 48)] {
            for peak in [0.05, 0.10, 0.20] {
                let audio = low_level_spectrum(start, end, peak);
                assert_eq!(
                    [
                        audio.bass,
                        audio.mids,
                        audio.treble,
                        audio.beat,
                        audio.transient
                    ],
                    [0.0; 5]
                );
                assert_eq!(audio.energy, 0.08);
                for (bin, value) in audio.spectrum.iter().enumerate() {
                    if (start..end).contains(&bin) {
                        assert!(*value > 0.0 && *value <= peak);
                    } else {
                        assert_eq!(*value, 0.0);
                    }
                }
            }
        }
    }

    #[test]
    fn changed_pixel_coverage_ignores_alpha_and_single_code_noise() {
        let a = [10, 20, 30, 0, 40, 50, 60, 255];
        assert_eq!(
            changed_pixel_fraction(&a, &[11, 20, 30, 255, 40, 50, 60, 0]),
            0.0
        );
        assert_eq!(
            changed_pixel_fraction(&a, &[12, 20, 30, 0, 40, 50, 60, 255]),
            0.5
        );
    }
}
