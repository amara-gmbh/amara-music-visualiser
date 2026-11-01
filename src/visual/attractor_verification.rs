//! Measured-history Attractor fixtures through the real production render path.
use super::{
    AudioFeatures, HISTORY_FORMAT, LOOKS, ShaderUniforms, VisualResources, WORLD_COUNT,
    verification::{difference, render_frame, write_contact_sheet, write_png},
    wgpu,
};
use crate::presets::AudioMappings;
use anyhow::Context;
use std::{fs, path::Path};

const SIZE: [u32; 2] = [640, 400];
const WORLD: usize = 5;

fn support(pixels: &[u8]) -> Vec<bool> {
    pixels
        .chunks_exact(4)
        .map(|p| p[0].max(p[1]).max(p[2]) > 12)
        .collect()
}

fn displaced_support(before: &[u8], after: &[u8]) -> usize {
    let old = support(before);
    let new = support(after);
    let width = SIZE[0] as i32;
    let height = SIZE[1] as i32;
    new.iter()
        .enumerate()
        .filter(|(i, occupied)| {
            if !**occupied {
                return false;
            }
            let x = *i as i32 % width;
            let y = *i as i32 / width;
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let px = x + dx;
                    let py = y + dy;
                    if px >= 0
                        && px < width
                        && py >= 0
                        && py < height
                        && old[(py * width + px) as usize]
                    {
                        return false;
                    }
                }
            }
            true
        })
        .count()
}

pub fn preview_attractor(destination: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
    let report = destination.join("attractor-report.json");
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
    .context("no GPU adapter for Attractor preview")?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("amara Attractor measured-history preview"),
        ..Default::default()
    }))?;
    let mut production = VisualResources::new(&device, HISTORY_FORMAT);
    let mut isolated = VisualResources::new(&device, HISTORY_FORMAT);
    // Expose the foreground already produced by the real raster renderer.
    // Its FFT buffers are populated only by VisualResources::render/prepare.
    let output = "return vec4<f32>(clamp(colour, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);";
    anyhow::ensure!(
        super::WORLD_SHADER_SOURCE.contains(output),
        "Attractor foreground probe hook changed"
    );
    let source = super::WORLD_SHADER_SOURCE.replace(
        output,
        "return vec4<f32>(vec3<f32>(1.0) - exp(-radiance.marks), 1.0);",
    );
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("actual Attractor foreground probe"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Attractor foreground probe layout"),
        bind_group_layouts: &[Some(&isolated.bind_group_layout)],
        immediate_size: 0,
    });
    isolated.scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Attractor foreground probe compositor"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
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
    let profile = crate::presets::Profile::astral_bloom();
    let mut visual = profile.visual.clone();
    visual.scene = WORLD;
    visual.feedback = 0.0;
    visual.complexity = 1.1;
    let mut epoch = 0;
    let mut draw = |renderer: &mut VisualResources,
                    look: usize,
                    audio: &AudioFeatures,
                    mappings: &AudioMappings|
     -> anyhow::Result<Vec<u8>> {
        epoch += 1;
        renderer.history = None;
        for frame in 0..=64 {
            let time = 8.0 + frame as f32 * 0.1;
            let mut looks = [[0.0, 0.0, 1.0, time]; WORLD_COUNT];
            looks[WORLD] = [look as f32, look as f32, 1.0, time];
            let input = ShaderUniforms::new(time, 0.1, WORLD, WORLD, 1.0, &visual, mappings, audio)
                .with_looks(looks)
                .with_audio_state(true, epoch);
            if frame == 64 {
                return render_frame(renderer, &device, &queue, input, SIZE);
            }
            let mut encoder = device.create_command_encoder(&Default::default());
            renderer.render(&device, &queue, &mut encoder, input, SIZE);
            queue.submit([encoder.finish()]);
        }
        unreachable!()
    };
    let quiet = AudioFeatures::default();
    let mut route_reports = Vec::new();
    let mut failures = Vec::new();
    let mut production_sheet = Vec::new();
    let mut isolated_sheet = Vec::new();
    for look in [1, 2, 3, 8] {
        let base = draw(&mut isolated, look, &quiet, &profile.mappings)?;
        production_sheet.push(draw(&mut production, look, &quiet, &profile.mappings)?);
        isolated_sheet.push(base.clone());
        for (label, start, end) in [("bass", 0, 8), ("mids", 8, 24), ("highs", 24, 48)] {
            let mut band = AudioFeatures::default();
            for bin in start..end {
                band.spectrum[bin] = 0.28 + 0.46 * (bin as f32 * 0.37).sin().powi(2);
            }
            let frame = draw(&mut isolated, look, &band, &profile.mappings)?;
            let colour = draw(&mut production, look, &band, &profile.mappings)?;
            let rgb = difference(&base, &frame);
            let new_support = displaced_support(&base, &frame);
            write_png(
                &destination.join(format!("attractor-{look}-{label}-foreground.png")),
                &frame,
                SIZE,
            )?;
            write_png(
                &destination.join(format!("attractor-{look}-{label}.png")),
                &colour,
                SIZE,
            )?;
            // Bass and mids flex the body; highs illuminate travelling fine
            // filaments. Do not force the air control to deform a liked form.
            let geometry_role = label != "highs";
            if rgb <= 0.10 || (geometry_role && new_support < 32) {
                failures.push(format!("Attractor {} {label} lacks displaced foreground (RGB {rgb:.3}, support {new_support})", LOOKS[WORLD][look]));
            }
            let mut disabled = profile.mappings.clone();
            match start {
                0 => disabled.bass_to_pulse = 0.0,
                8 => disabled.mids_to_geometry = 0.0,
                _ => disabled.treble_to_sparkle = 0.0,
            }
            let disconnected = draw(&mut isolated, look, &band, &disabled)?;
            let disabled_difference = difference(&base, &disconnected);
            if base != disconnected {
                failures.push(format!(
                    "Attractor look {look} {label} leaks across mapping"
                ));
            }
            route_reports.push(serde_json::json!({"look":look,"name":LOOKS[WORLD][look],"band":label,
                "foreground_rgb_difference":rgb,"new_foreground_pixels_beyond_two_pixel_baseline":new_support,
                "role":if geometry_role {"body flex"} else {"travelling filament light"},"disabled_difference":disabled_difference}));
            production_sheet.push(colour);
            isolated_sheet.push(frame);
        }
    }
    write_contact_sheet(
        &destination.join("attractor-frequency-response.png"),
        &production_sheet,
        4,
    )?;
    write_contact_sheet(
        &destination.join("attractor-foreground-response.png"),
        &isolated_sheet,
        4,
    )?;
    let loud = AudioFeatures {
        spectrum: vec![0.85; 48],
        ..Default::default()
    };
    let maximum = AudioMappings {
        bass_to_pulse: 2.0,
        mids_to_geometry: 2.0,
        treble_to_sparkle: 2.0,
        beat_to_flash: 2.0,
    };
    let mut fit = Vec::new();
    let mut gallery = Vec::new();
    for (look, name) in LOOKS[WORLD].iter().enumerate() {
        let raw = draw(&mut isolated, look, &loud, &maximum)?;
        let frame = draw(&mut production, look, &loud, &maximum)?;
        let mask = support(&raw);
        let mut bound = [SIZE[0], SIZE[1], 0, 0];
        let mut occupied = 0;
        let mut whites = 0;
        for (index, on) in mask.iter().enumerate() {
            if *on {
                let x = index as u32 % SIZE[0];
                let y = index as u32 / SIZE[0];
                bound = [
                    bound[0].min(x),
                    bound[1].min(y),
                    bound[2].max(x),
                    bound[3].max(y),
                ];
                occupied += 1;
            }
            if raw[index * 4..index * 4 + 3]
                .iter()
                .all(|channel| *channel > 245)
            {
                whites += 1;
            }
        }
        if occupied <= 100
            || bound[0] <= 1
            || bound[1] <= 1
            || bound[2] + 2 >= SIZE[0]
            || bound[3] + 2 >= SIZE[1]
        {
            failures.push(format!(
                "Attractor {name} clips or loses its body at max mappings: {bound:?}"
            ));
        }
        write_png(
            &destination.join(format!("attractor-look-{look}-maximum.png")),
            &frame,
            SIZE,
        )?;
        fit.push(serde_json::json!({"look":look,"name":name,"foreground_bounds":bound,"occupied_pixels":occupied,"near_white_foreground_pixels":whites}));
        gallery.push(frame);
    }
    write_contact_sheet(
        &destination.join("attractor-ten-looks-maximum.png"),
        &gallery,
        5,
    )?;
    fs::write(
        &report,
        serde_json::to_vec_pretty(
            &serde_json::json!({"status":if failures.is_empty() {"pass"} else {"failed"},"failures":failures,"adapter":info.name,
        "routes":route_reports,"maximum_mapping_fit":fit,"size":SIZE,"history":"64 actual production warm renders per fixture, fresh source epoch; no direct storage population",
        "scope":"Synthetic actual renderer and isolated foreground; FFT-driven spatial support and mapping isolation, 10 max-gain bounds. Support is a thresholded pixel witness, not a complete vertex displacement measurement or live-audio/FPS proof."}),
        )?,
    )?;
    anyhow::ensure!(
        failures.is_empty(),
        "Attractor checks failed: {}",
        failures.join("; ")
    );
    println!(
        "Attractor foreground and measured-history previews passed on {}",
        info.name
    );
    Ok(())
}
