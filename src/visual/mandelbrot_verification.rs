//! Actual production-GPU Mandelbrot gallery and bounded camera/routing checks.
use super::{
    AudioFeatures, HISTORY_FORMAT, LOOKS, ShaderUniforms, VisualResources, WORLD_COUNT,
    verification::{difference, render_frame, write_contact_sheet, write_png},
    wgpu,
};
use crate::presets::AudioMappings;
use anyhow::Context;
use std::{fs, path::Path};

const SIZE: [u32; 2] = [640, 400];
const WORLD: usize = 10;
const COMPLETE_TIME: f32 = 40.0;

/// Follow the production Auto/camera/region clocks; never invent formation ages.
pub fn preview_mandelbrot_auto(destination: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
    let mut auto = crate::auto_journey::AutoJourney::new_with_seed(WORLD, 81);
    let mut scene = crate::scene::SceneTransition::new(0);
    scene.request(WORLD);
    let mut morph = auto.morph_seconds();
    let mut visit = crate::app::MandelbrotVisit::default();
    let mut looks = crate::look_journey::LookJourney::seeded(931);
    let mut motion = crate::motion::RenderMotion::new();
    let mut pace = crate::motion::MotionPace::new(0.8);
    let mut choreography = crate::choreography::Choreographer::new();
    let mut musical = crate::music_response::MusicResponse::new();
    let mut profile = crate::presets::built_in_profiles()
        .into_iter()
        .find(|profile| profile.visual.scene == WORLD)
        .context("Mandelbrot preset missing")?;
    profile.visual.motion = 0.8;
    let audio = AudioFeatures {
        bass: 0.62,
        mids: 0.56,
        treble: 0.48,
        energy: 0.57,
        spectrum: (0..48)
            .map(|i| 0.14 + 0.50 * (i as f32 * 0.37 + 0.2).sin().powi(2))
            .collect(),
        ..Default::default()
    };
    let mut states = Vec::new();
    let mut events = Vec::new();
    let mut visual_time = 0.0_f64;
    let mut previous_look = looks.current(WORLD);
    let mut departure = None;
    let mut masks = std::collections::BTreeSet::new();
    for frame in 0..=70 * 60 {
        let dt = if frame == 0 { 0.0 } else { 1.0 / 60.0 };
        let physical = frame as f32 / 60.0;
        let step = pace.advance(0.8, dt);
        if let Some(next) = auto.update(&audio, &profile.mappings, step.world_delta) {
            if scene.amount == 1.0 {
                morph = auto.morph_seconds();
            }
            scene.request(next);
            departure = Some(physical);
        }
        scene.advance_over(dt, morph);
        let (age, _) = visit.update_for_mode(&scene, dt, true, true, 0, &mut looks);
        let rendered = motion.update(&audio, dt);
        looks.set_visible_pair(scene.from, scene.to, scene.blend());
        let lanes = looks.update(
            &rendered,
            &profile.mappings,
            true,
            scene.to,
            0,
            step.evolution_delta,
        );
        let current = looks.current(WORLD);
        if current != previous_look {
            events.push(serde_json::json!({"physical_seconds":physical,"age":age,"region":current,"name":LOOKS[WORLD][current]}));
            previous_look = current;
        }
        let mut phrase = choreography.update_mapped(&rendered, &profile.mappings, dt);
        visual_time += f64::from(step.travel_delta) * f64::from(phrase.travel_speed);
        phrase.phase = visual_time as f32;
        let response = musical.update(&audio, &profile.mappings, true, 500, dt);
        let u = ShaderUniforms::new(
            visual_time as f32,
            dt,
            scene.to,
            scene.from,
            scene.blend(),
            &profile.visual,
            &profile.mappings,
            &rendered,
        )
        .with_choreography(phrase)
        .with_auto_mix(1.0, auto.blend_seed())
        .with_looks(lanes)
        .with_mandelbrot_age(age)
        .with_mandelbrot_motion(visit.manual_motion())
        .with_music_response(response)
        .with_audio_state(true, 500);
        masks.insert(super::pipelines::frame_mask(
            scene.from as f32,
            scene.to as f32,
            scene.blend(),
        ));
        states.push((u,serde_json::json!({"physical_seconds":physical,"camera_age":age,"region":current,"name":LOOKS[WORLD][current],"region_blend":lanes[WORLD][2],"from":scene.from,"to":scene.to,"world_blend":scene.blend(),"countdown":auto.next_world_progress().1})));
    }
    anyhow::ensure!(
        events.iter().any(
            |event| event["physical_seconds"].as_f64().unwrap_or(0.0) > 30.0
                && event["age"].as_f64().unwrap_or(1.0) < 0.1
        ),
        "Auto never began another broad formation"
    );
    anyhow::ensure!(
        departure.is_some_and(|t| (64.0..=70.1).contains(&t)),
        "Auto departure did not use longer dwell"
    );
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: if cfg!(target_os = "macos") {
            wgpu::Backends::METAL
        } else {
            wgpu::Backends::all()
        },
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
    let mut renderer = VisualResources::new(&device, HISTORY_FORMAT);
    renderer.specialize(&device, &masks.into_iter().collect::<Vec<_>>());
    let mut frames = Vec::new();
    let mut samples = Vec::new();
    let mut max_step = 0.0_f64;
    for second in [
        0, 4, 8, 12, 18, 24, 30, 36, 40, 44, 48, 52, 56, 60, 64, 68, 70,
    ] {
        let index = second * 60;
        renderer.history = None;
        let image = render_frame(&mut renderer, &device, &queue, states[index].0, SIZE)?;
        if index + 1 < states.len() {
            renderer.history = None;
            let next = render_frame(&mut renderer, &device, &queue, states[index + 1].0, SIZE)?;
            max_step = max_step.max(difference(&image, &next));
        }
        write_png(
            &destination.join(format!("auto-{second:02}s.png")),
            &image,
            SIZE,
        )?;
        frames.push(image);
        samples.push(states[index].1.clone());
    }
    write_contact_sheet(&destination.join("mandelbrot-auto-70s.png"), &frames, 4)?;
    fs::write(
        destination.join("mandelbrot-auto-report.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"status":"pass","departure_seconds":departure,"region_events":events,"samples":samples,"maximum_sampled_adjacent_rgb_codes":max_step,"scope":"70physical seconds simulated at60Hz through actual AutoJourney, SceneTransition, MandelbrotVisit, LookJourney, RenderMotion, MusicResponse, MotionPace0.8 and Choreographer. Actual prepared renderer at listed states. Fixed material palette; synthetic audio, no live capture/FPS claim."}),
        )?,
    )?;
    Ok(())
}

pub fn preview_mandelbrot(destination: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
    let report = destination.join("mandelbrot-report.json");
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
    .context("no GPU adapter for Mandelbrot preview")?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("amara Mandelbrot preview"),
        ..Default::default()
    }))?;
    let mut renderer = VisualResources::new(&device, HISTORY_FORMAT);
    let profile = crate::presets::built_in_profiles()
        .into_iter()
        .find(|profile| profile.visual.scene == WORLD)
        .context("Mandelbrot preset missing")?;
    let music = AudioFeatures {
        bass: 0.68,
        mids: 0.57,
        treble: 0.48,
        spectrum: (0..48)
            .map(|bin| 0.12 + 0.55 * (bin as f32 * 0.37 + 0.21).sin().powi(2))
            .collect(),
        ..Default::default()
    };
    let quiet = AudioFeatures::default();
    let uniforms = |from: usize,
                    to: usize,
                    blend: f32,
                    time: f32,
                    audio: &AudioFeatures,
                    mappings: &AudioMappings| {
        let mut looks = [[0.0, 0.0, 1.0, 0.0]; WORLD_COUNT];
        looks[WORLD] = [from as f32, to as f32, blend, time];
        ShaderUniforms::new(
            time,
            1.0 / 60.0,
            WORLD,
            WORLD,
            1.0,
            &profile.visual,
            mappings,
            audio,
        )
        .with_looks(looks)
        .with_mandelbrot_age(COMPLETE_TIME)
        .with_audio_state(true, 1)
    };
    let mut draw = |input, size| {
        renderer.history = None;
        render_frame(&mut renderer, &device, &queue, input, size)
    };
    let mut gallery = Vec::new();
    let mut recipes = Vec::new();
    for (look, name) in LOOKS[WORLD].iter().enumerate() {
        let frame = draw(
            uniforms(look, look, 1.0, COMPLETE_TIME, &music, &profile.mappings),
            SIZE,
        )?;
        write_png(
            &destination.join(format!("mandelbrot-look-{look}.png")),
            &frame,
            SIZE,
        )?;
        let silent = draw(
            uniforms(look, look, 1.0, COMPLETE_TIME, &quiet, &profile.mappings),
            SIZE,
        )?;
        let response = difference(&silent, &frame);
        let next = draw(
            uniforms(
                look,
                look,
                1.0,
                COMPLETE_TIME + 1.0 / 60.0,
                &music,
                &profile.mappings,
            ),
            SIZE,
        )?;
        let step = difference(&frame, &next);
        let later = draw(
            uniforms(
                look,
                look,
                1.0,
                COMPLETE_TIME + 0.5,
                &music,
                &profile.mappings,
            ),
            SIZE,
        )?;
        let motion = difference(&frame, &later);
        anyhow::ensure!(response > 0.5, "{name}: missing musical relief");
        anyhow::ensure!(
            step < 12.0 && motion > 0.1,
            "{name}: static or discontinuous camera ({step:.3}, {motion:.3})"
        );
        recipes.push(
            serde_json::json!({"look":look,"name":name,"music_difference":response,
            "one_frame_rgb_step":step,"half_second_motion":motion}),
        );
        gallery.push(frame);
    }
    write_contact_sheet(&destination.join("mandelbrot-ten-looks.png"), &gallery, 5)?;
    let mut formation_frames = Vec::new();
    let mut formation_samples = Vec::new();
    // A late app clock deliberately proves creation is the current visit,
    // rather than a global cycle that Auto can enter halfway through.
    const VISIT_START: f32 = 1200.0;
    for age in [
        0.0, 0.5, 1.0, 2.0, 3.0, 4.0, 6.0, 8.0, 10.0, 12.0, 16.0, 24.0,
    ] {
        let frame = draw(
            uniforms(0, 0, 1.0, VISIT_START + age, &music, &profile.mappings)
                .with_mandelbrot_age(age),
            SIZE,
        )?;
        let adjacent = draw(
            uniforms(
                0,
                0,
                1.0,
                VISIT_START + age + 1.0 / 60.0,
                &music,
                &profile.mappings,
            )
            .with_mandelbrot_age(age + 1.0 / 60.0),
            SIZE,
        )?;
        let step = difference(&frame, &adjacent);
        anyhow::ensure!(
            step < 12.0,
            "Mandelbrot formation jumps at visit age {age}s: {step:.3}"
        );
        write_png(
            &destination.join(format!("mandelbrot-formation-{age:05.1}.png")),
            &frame,
            SIZE,
        )?;
        formation_frames.push(frame);
        formation_samples.push(serde_json::json!({"visible_age":age,"app_time":VISIT_START+age,"one_frame_rgb_step":step}));
    }
    write_contact_sheet(
        &destination.join("mandelbrot-formation-sequence.png"),
        &formation_frames,
        4,
    )?;
    // Exercise the actual app controller and real look lane, rather than a
    // second shader-only approximation of the manual journey schedule.
    let scene = crate::scene::SceneTransition::new(WORLD);
    let mut visit = crate::app::MandelbrotVisit::default();
    let mut journey = crate::look_journey::LookJourney::seeded(931);
    let mut selected = journey.current(WORLD);
    let mut manual_states = Vec::new();
    let mut region_changes = Vec::new();
    let mut seam_frames = Vec::new();
    for frame in 0..=60 * 150 {
        let dt = if frame == 0 { 0.0 } else { 1.0 / 60.0 };
        let time = frame as f32 / 60.0;
        let age = visit.update(&scene, dt, true, Some(selected));
        if visit.take_next_region()
            && let Some(next) = journey.next_manual_mandelbrot_region(selected)
        {
            selected = next;
            visit.selected_region(next);
            region_changes.push(
                serde_json::json!({"time":time,"target":next,"name":LOOKS[WORLD][next],"age":age}),
            );
            seam_frames.extend([frame - 1, frame, frame + 1, frame + 30, frame + 420]);
        }
        let looks = journey.update(&music, &profile.mappings, false, WORLD, selected, dt);
        manual_states.push(
            uniforms(
                selected,
                selected,
                1.0,
                VISIT_START + time,
                &music,
                &profile.mappings,
            )
            .with_mandelbrot_age(age)
            .with_mandelbrot_motion(visit.manual_motion())
            .with_looks(looks),
        );
    }
    anyhow::ensure!(
        region_changes.len() >= 3,
        "manual journey did not reach new regions"
    );
    let gallery_seconds = [
        0, 4, 10, 18, 24, 30, 36, 40, 44, 50, 60, 76, 80, 90, 100, 112,
    ];
    let mut sample_frames: Vec<_> = gallery_seconds.iter().map(|second| second * 60).collect();
    sample_frames.extend(seam_frames);
    sample_frames.sort_unstable();
    sample_frames.dedup();
    let mut manual_frames = Vec::new();
    let mut manual_samples = Vec::new();
    for frame in sample_frames
        .into_iter()
        .filter(|frame| *frame + 30 < manual_states.len())
    {
        let input = manual_states[frame];
        let pixels = draw(input, SIZE)?;
        let adjacent = draw(manual_states[frame + 1], SIZE)?;
        let later = draw(manual_states[frame + 30], SIZE)?;
        let step = difference(&pixels, &adjacent);
        let motion = difference(&pixels, &later);
        anyhow::ensure!(
            step < 12.0,
            "manual Mandelbrot discontinuity at frame {frame}: {step:.3}"
        );
        anyhow::ensure!(
            motion > 0.1,
            "manual Mandelbrot held still at frame {frame}: {motion:.3}"
        );
        manual_samples.push(serde_json::json!({"time":frame as f32/60.0,"visible_age":input.material[1],"look":input.looks[WORLD],"one_frame_rgb_step":step,"half_second_motion":motion}));
        if frame % 60 == 0 && gallery_seconds.contains(&(frame / 60)) {
            write_png(
                &destination.join(format!("mandelbrot-manual-{:03}.png", frame / 60)),
                &pixels,
                SIZE,
            )?;
            manual_frames.push(pixels);
        }
    }
    write_contact_sheet(
        &destination.join("mandelbrot-manual-journey.png"),
        &manual_frames,
        4,
    )?;
    let hero = draw(
        uniforms(0, 0, 1.0, COMPLETE_TIME, &music, &profile.mappings),
        [1200, 750],
    )?;
    write_png(&destination.join("mandelbrot-hero.png"), &hero, [1200, 750])?;
    let mut morph = Vec::new();
    for blend in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let frame = draw(
            uniforms(0, 8, blend, COMPLETE_TIME, &music, &profile.mappings),
            SIZE,
        )?;
        if blend == 0.0 {
            anyhow::ensure!(frame == gallery[0], "Mandelbrot source endpoint changed");
        }
        if blend == 1.0 {
            anyhow::ensure!(frame == gallery[8], "Mandelbrot target endpoint changed");
        }
        morph.push(frame);
    }
    write_contact_sheet(
        &destination.join("mandelbrot-camera-passage.png"),
        &morph,
        5,
    )?;
    let zero = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let off_quiet = draw(uniforms(0, 0, 1.0, COMPLETE_TIME, &quiet, &zero), SIZE)?;
    let off_music = draw(uniforms(0, 0, 1.0, COMPLETE_TIME, &music, &zero), SIZE)?;
    anyhow::ensure!(
        off_quiet == off_music,
        "Mandelbrot mapping disconnection leaked"
    );
    let baseline = draw(
        uniforms(0, 0, 1.0, COMPLETE_TIME, &quiet, &profile.mappings),
        SIZE,
    )?;
    let mut envelopes_only = music.clone();
    envelopes_only.spectrum.fill(0.0);
    let scalar_frame = draw(
        uniforms(0, 0, 1.0, COMPLETE_TIME, &envelopes_only, &profile.mappings),
        SIZE,
    )?;
    let spectrum_over_envelopes = difference(&scalar_frame, &gallery[0]);
    anyhow::ensure!(
        spectrum_over_envelopes > 0.1,
        "Mandelbrot scalar envelopes hide FFT material detail"
    );
    let mut bands = Vec::new();
    let mut responses = vec![baseline.clone()];
    for (label, start, end) in [("bass", 0, 8), ("mids", 8, 24), ("highs", 24, 48)] {
        let mut isolated = AudioFeatures::default();
        for bin in start..end {
            isolated.spectrum[bin] = 0.55;
        }
        let frame = draw(
            uniforms(0, 0, 1.0, COMPLETE_TIME, &isolated, &profile.mappings),
            SIZE,
        )?;
        let response = difference(&baseline, &frame);
        anyhow::ensure!(response > 0.1, "Mandelbrot ignores {label}");
        let mut disabled = profile.mappings.clone();
        match start {
            0 => disabled.bass_to_pulse = 0.0,
            8 => disabled.mids_to_geometry = 0.0,
            _ => disabled.treble_to_sparkle = 0.0,
        }
        let disabled_frame = draw(
            uniforms(0, 0, 1.0, COMPLETE_TIME, &isolated, &disabled),
            SIZE,
        )?;
        anyhow::ensure!(
            disabled_frame == baseline,
            "Mandelbrot {label} mapping leaks"
        );
        bands.push(serde_json::json!({"band":label,"fft_only_difference":response,"disabled_difference":0.0}));
        responses.push(frame);
    }
    write_contact_sheet(
        &destination.join("mandelbrot-frequency-response.png"),
        &responses,
        4,
    )?;
    let mut time_steps = Vec::new();
    let mut complete_views = Vec::new();
    // Covers held detail and long-running clocks independently of visit age.
    for time in [0.0, 28.0, 40.0, 96.0, 600.0, 1800.0, 7200.0] {
        for look in 0..10 {
            let before = draw(
                uniforms(look, look, 1.0, time, &music, &profile.mappings),
                SIZE,
            )?;
            let after = draw(
                uniforms(
                    look,
                    look,
                    1.0,
                    time + 1.0 / 60.0,
                    &music,
                    &profile.mappings,
                ),
                SIZE,
            )?;
            let step = difference(&before, &after);
            let lit = before
                .chunks_exact(4)
                .filter(|pixel| pixel[0].max(pixel[1]).max(pixel[2]) > 16)
                .count() as f64
                / f64::from(SIZE[0] * SIZE[1]);
            anyhow::ensure!(lit > 0.02, "Mandelbrot look {look} goes empty at {time}s");
            if time == COMPLETE_TIME {
                write_png(
                    &destination.join(format!("mandelbrot-look-{look}-complete.png")),
                    &before,
                    SIZE,
                )?;
                complete_views.push(before.clone());
            }
            anyhow::ensure!(
                step < 12.0,
                "Mandelbrot camera jumps at {time}s, look {look}: {step:.3}"
            );
            time_steps.push(serde_json::json!({"time":time,"look":look,"one_frame_rgb_step":step,"lit_fraction":lit}));
        }
    }
    write_contact_sheet(
        &destination.join("mandelbrot-complete-views.png"),
        &complete_views,
        5,
    )?;
    // Selecting a world can carry another preset's Echo value. A true zero
    // memory world must bypass the shared minimum retention lifetime too.
    renderer.history = None;
    let mut warm = uniforms(0, 0, 1.0, COMPLETE_TIME, &music, &profile.mappings);
    warm.values[6][0] = 1.0;
    render_frame(&mut renderer, &device, &queue, warm, SIZE)?;
    let mut echo = uniforms(0, 0, 1.0, COMPLETE_TIME + 0.5, &music, &profile.mappings);
    echo.values[6][0] = 1.0;
    let warm_pixels = render_frame(&mut renderer, &device, &queue, echo, SIZE)?;
    renderer.history = None;
    echo.values[6][0] = 0.0;
    let cold_pixels = render_frame(&mut renderer, &device, &queue, echo, SIZE)?;
    anyhow::ensure!(
        warm_pixels == cold_pixels,
        "Mandelbrot inherited another world's image history"
    );
    let entry = uniforms(0, 0, 1.0, 7200.0, &music, &profile.mappings).with_mandelbrot_age(0.0);
    let first_entry = render_frame(&mut renderer, &device, &queue, entry, SIZE)?;
    render_frame(
        &mut renderer,
        &device,
        &queue,
        entry.with_mandelbrot_age(16.0),
        SIZE,
    )?;
    let reentry = render_frame(&mut renderer, &device, &queue, entry, SIZE)?;
    anyhow::ensure!(
        first_entry == reentry,
        "Mandelbrot reentry retains the previous visit's detail"
    );
    fs::write(
        &report,
        serde_json::to_vec_pretty(&serde_json::json!({
            "status":"pass","adapter":info.name,"world":WORLD,"looks":recipes,"bands":bands,
            "zero_mappings_difference":0.0,"look_endpoints":"pixel exact","camera_samples":time_steps,
            "spectrum_over_fixed_envelopes_difference":spectrum_over_envelopes,
            "carried_echo_history_difference":0.0,
            "formation_samples":formation_samples,
            "manual_journey":{"region_changes":region_changes,"samples":manual_samples,"controller":"production MandelbrotVisit and LookJourney, 150 physical seconds simulated at 60Hz; continuous age legs and real named-region transitions"},
            "entry_reentry":"pixel exact at zero visit age after drawing a formed visit at the same late app clock; CPU checks separately verify exposure and hidden-world resets",
            "formation":"Successive finite-iteration radius-two escape sets with a fractionally interpolated first-crossing frontier. Auto retains once-per-visit visible-exposure formation. Manual hold keeps moving through eased depth journeys and fresh real regions, renewing formation only at broad framing without a modulo reset.",
            "precision":"f32, minimum complex-plane half-height 0.0008, 160–352 iterations; bounded repeating zoom excursions",
            "scope":"Synthetic production-GPU frames; ten view recipes, separate FFT mapping checks, camera endpoints and bounded time-step samples. Not live audio, display FPS or uninterrupted runtime acceptance."
        }))?,
    )?;
    println!("Mandelbrot previews passed on {}", info.name);
    Ok(())
}
