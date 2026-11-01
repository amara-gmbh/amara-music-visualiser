//! Bounded production-GPU evidence for appended worlds and music response. Synthetic
//! audio only; these image comparisons do not claim live sync or display FPS.
use super::{
    AudioFeatures, HISTORY_FORMAT, LOOKS, SCENES, ShaderUniforms, VisualResources, WORLD_COUNT,
    verification::{difference, render_frame, write_contact_sheet, write_png},
    wgpu,
};
use crate::presets::{AudioMappings, VisualSettings};
use anyhow::Context;
use std::{fs, path::Path};

const SIZE: [u32; 2] = [640, 400];
const DT: f32 = 1.0 / 60.0;

struct Probe {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: VisualResources,
}

impl Probe {
    fn isolate_silk(&mut self) {
        self.fragment_probe(
            "fs_silk_probe",
            r#"
@fragment fn fs_silk_probe(in: VertexOut) -> @location(0) vec4<f32> {
    let raw = textureSampleLevel(raster_worlds, frame_sampler, in.uv, 4, 0.0).rgb;
    return vec4<f32>(1.0 - exp(-raw), 1.0);
}"#,
        );
    }

    fn fragment_probe(&mut self, entry: &str, fragment: &str) {
        self.renderer.prepared_pipelines.clear();
        let source = format!("{}\n{fragment}", super::WORLD_SHADER_SOURCE);
        let module = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("diagnostic production-function probe"),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        let layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[Some(&self.renderer.bind_group_layout)],
                immediate_size: 0,
            });
        self.renderer.scene_pipeline =
            self.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("diagnostic fragment probe"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("vs_main"),
                        buffers: &[],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some(entry),
                        targets: &[Some(HISTORY_FORMAT.into())],
                        compilation_options: Default::default(),
                    }),
                    primitive: Default::default(),
                    depth_stencil: None,
                    multisample: Default::default(),
                    multiview_mask: None,
                    cache: None,
                });
    }
    fn draw(&mut self, input: ShaderUniforms, size: [u32; 2]) -> anyhow::Result<Vec<u8>> {
        render_frame(&mut self.renderer, &self.device, &self.queue, input, size)
    }

    fn cold(&mut self, input: ShaderUniforms, size: [u32; 2]) -> anyhow::Result<Vec<u8>> {
        self.renderer.history = None;
        self.draw(input, size)
    }

    fn advance(&mut self, mut input: ShaderUniforms, frames: usize) {
        for frame in 0..frames {
            input.values[0][2] += DT;
            input.values[8][0] += DT;
            let mut encoder = self.device.create_command_encoder(&Default::default());
            self.renderer
                .render(&self.device, &self.queue, &mut encoder, input, SIZE);
            self.queue.submit([encoder.finish()]);
            if frame % 60 == 59 {
                // Keep the diagnostic bounded rather than queueing minutes of work.
                self.device
                    .poll(wgpu::PollType::Wait {
                        submission_index: None,
                        timeout: Some(std::time::Duration::from_secs(20)),
                    })
                    .expect("GPU warm frames");
            }
        }
    }

    fn fixture(&mut self, mut input: ShaderUniforms) -> anyhow::Result<Vec<u8>> {
        self.renderer.history = None;
        if input.values[0][3] == 13.0 && input.values[6][0] > 0.0 {
            input.values[0][2] -= 1.5;
            input.values[8][0] -= 1.5;
            self.advance(input, 90);
            input.values[0][2] += 1.5;
            input.values[8][0] += 1.5;
        }
        self.draw(input, SIZE)
    }
}

fn input(
    world: usize,
    look: usize,
    time: f32,
    audio: &AudioFeatures,
    mappings: &AudioMappings,
) -> ShaderUniforms {
    let settings = VisualSettings {
        scene: world,
        look,
        complexity: 1.1,
        feedback: if world == 13 { 0.38 } else { 0.0 },
        ..Default::default()
    };
    let mut looks = [[0.0, 0.0, 1.0, time]; WORLD_COUNT];
    looks[world] = [look as f32, look as f32, 1.0, time];
    ShaderUniforms::new(time, DT, world, world, 1.0, &settings, mappings, audio)
        .with_looks(looks)
        .with_audio_state(true, 1)
}

fn music() -> AudioFeatures {
    AudioFeatures {
        bass: 0.62,
        mids: 0.56,
        treble: 0.48,
        beat: 0.28,
        energy: 0.57,
        spectrum: (0..48)
            .map(|bin| 0.14 + 0.50 * (bin as f32 * 0.37 + 0.2).sin().powi(2))
            .collect(),
        ..Default::default()
    }
}

/// Strict same-device A/B through real geometry, history and presentation.
pub fn verify_specialization(destination: &Path) -> anyhow::Result<()> {
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
    let mut general = VisualResources::new(&device, HISTORY_FORMAT);
    let mut specialized = VisualResources::new(&device, HISTORY_FORMAT);
    let start = std::time::Instant::now();
    specialized.specialize(&device, &super::pipelines::masks(0));
    let build_ms = start.elapsed().as_secs_f64() * 1000.0;
    let music = music();
    let mappings = AudioMappings::default();
    let mut cases = Vec::new();
    let mut maximum_difference: f64 = 0.0;
    let mut maximum_changed_fraction: f64 = 0.0;
    let mut maximum_channel_difference = 0;
    let mut worst_case = serde_json::Value::Null;
    for world in 0..WORLD_COUNT {
        for size in [SIZE, [360, 640]] {
            for centre in [0.0, -0.4] {
                for palette in [0.0, 1.0] {
                    general.history = None;
                    specialized.history = None;
                    let mut difference_at_end = 0.0;
                    let mut maximum_channel_delta = 0;
                    let mut changed_fraction: f64 = 0.0;
                    let mut changed_samples = Vec::new();
                    for frame in 0..8 {
                        let mut u = input(world, 0, 12.0 + frame as f32 * DT, &music, &mappings)
                            .with_canvas_centre(centre)
                            .with_palette_mix(palette);
                        u.values[6][0] = 0.38;
                        if palette > 0.0 {
                            u.values[4] = [0.0; 4];
                            u.spectrum = [[0.0; 4]; 12];
                        }
                        let reference = render_frame(&mut general, &device, &queue, u, size)?;
                        let actual = render_frame(&mut specialized, &device, &queue, u, size)?;
                        difference_at_end = difference(&reference, &actual);
                        maximum_channel_delta = reference
                            .iter()
                            .zip(&actual)
                            .map(|(a, b)| a.abs_diff(*b))
                            .max()
                            .unwrap_or(0);
                        maximum_channel_difference =
                            maximum_channel_difference.max(maximum_channel_delta);
                        let changed = reference
                            .chunks_exact(4)
                            .zip(actual.chunks_exact(4))
                            .enumerate()
                            .filter(|(_, (a, b))| a != b)
                            .collect::<Vec<_>>();
                        changed_fraction = changed.len() as f64 / (size[0] * size[1]) as f64;
                        maximum_changed_fraction = maximum_changed_fraction.max(changed_fraction);
                        changed_samples = changed.iter().take(16).map(|(index,(a,b))| serde_json::json!({
                            "x":*index as u32%size[0],"y":*index as u32/size[0],"general":a,"specialized":b
                        })).collect::<Vec<_>>();
                        if difference_at_end > maximum_difference {
                            maximum_difference = difference_at_end;
                            worst_case = serde_json::json!({"world":world,"frame":frame,"size":size,"centre":centre,"palette":palette,"rgb_difference":difference_at_end,"changed_fraction":changed_fraction,"maximum_channel_delta":maximum_channel_delta});
                            write_png(&destination.join("worst-reference.png"), &reference, size)?;
                            write_png(&destination.join("worst-actual.png"), &actual, size)?;
                        }
                        if frame == 7 && size == SIZE && centre == 0.0 && palette == 0.0 {
                            write_contact_sheet(
                                &destination
                                    .join(format!("specialization-{world}-reference-actual.png")),
                                &[reference, actual],
                                2,
                            )?;
                        }
                    }
                    cases.push(serde_json::json!({"world":world,"size":size,"centre":centre,
                        "palette":palette,"final_rgb_difference":difference_at_end,"final_maximum_channel_delta":maximum_channel_delta,
                        "final_changed_fraction":changed_fraction,"final_changed_samples":changed_samples}));
                }
            }
        }
    }
    for (from, to) in [
        (5, 11),
        (11, 5),
        (5, 7),
        (11, 12),
        (12, 13),
        (11, 13),
        (0, 11),
        (9, 13),
        (2, 9),
        (2, 3),
        (3, 2),
        (2, 8),
        (8, 2),
        (3, 8),
        (8, 3),
    ] {
        for handover in [false, true] {
            general.history = None;
            specialized.history = None;
            for (frame, amount) in [0.5, 0.8, 0.95, 0.99, 1.0, 1.0, 1.0, 1.0]
                .into_iter()
                .enumerate()
            {
                let mut u = input(to, 0, 18.0 + frame as f32 * DT, &music, &mappings)
                    .with_auto_mix(0.75, 1.7);
                u.values[5][2] = from as f32;
                u.values[5][3] = if handover { amount } else { 1.0 };
                let reference = render_frame(&mut general, &device, &queue, u, SIZE)?;
                let actual = render_frame(&mut specialized, &device, &queue, u, SIZE)?;
                let delta = difference(&reference, &actual);
                let maximum = reference
                    .iter()
                    .zip(&actual)
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap_or(0);
                let changed = reference
                    .chunks_exact(4)
                    .zip(actual.chunks_exact(4))
                    .filter(|(a, b)| a != b)
                    .count() as f64
                    / (SIZE[0] * SIZE[1]) as f64;
                maximum_difference = maximum_difference.max(delta);
                maximum_changed_fraction = maximum_changed_fraction.max(changed);
                maximum_channel_difference = maximum_channel_difference.max(maximum);
                cases.push(serde_json::json!({"source":from,"world":to,"handover":handover,"frame":frame,
                    "blend":u.values[5][3],"rgb_difference":delta,"changed_fraction":changed,"maximum_channel_delta":maximum}));
            }
        }
    }
    // Simulate arrival of a completed worker result after three fallback frames.
    // History stays alive while the selected pipeline changes underneath it.
    for world in [0, 10, 11, 13] {
        let mut prepared = Some(std::mem::take(&mut specialized.prepared_pipelines));
        general.history = None;
        specialized.history = None;
        for frame in 0..8 {
            if frame == 3 {
                specialized.prepared_pipelines = prepared.take().expect("prepared cache");
            }
            let u = input(world, 0, 18.0 + frame as f32 * DT, &music, &mappings);
            let reference = render_frame(&mut general, &device, &queue, u, SIZE)?;
            let actual = render_frame(&mut specialized, &device, &queue, u, SIZE)?;
            let delta = difference(&reference, &actual);
            let maximum = reference
                .iter()
                .zip(&actual)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap_or(0);
            let changed = reference
                .chunks_exact(4)
                .zip(actual.chunks_exact(4))
                .filter(|(a, b)| a != b)
                .count() as f64
                / (SIZE[0] * SIZE[1]) as f64;
            maximum_difference = maximum_difference.max(delta);
            maximum_changed_fraction = maximum_changed_fraction.max(changed);
            maximum_channel_difference = maximum_channel_difference.max(maximum);
            cases.push(serde_json::json!({"world":world,"frame":frame,"delayed_cache_arrival":true,"prepared":frame>=3,"rgb_difference":delta,"changed_fraction":changed,"maximum_channel_delta":maximum}));
        }
    }
    // Expanded to all14worlds: reviewed Mandelbrot boundary rounding has a
    // wider pixel support than Glass, but remains below one display code on
    // average by a wide margin. Keep support and peak bounds independent.
    let visually_equivalent = maximum_difference <= 0.02
        && maximum_changed_fraction <= 0.006
        && maximum_channel_difference <= 32;
    let report = serde_json::json!({"status":if visually_equivalent {"pass"} else {"fail"},
        "pixel_identical":maximum_difference==0.0,
        "acceptance":"Reviewed all-world visual equivalence: mean≤0.02 display codes, changed pixels≤0.6%, maximum channel≤32. Bounds include sensitive Mandelbrot boundary rounding; never a claim of exact identity.",
        "pipeline_build_ms":build_ms,"worst_case":worst_case,"maximum_frame_rgb_difference":maximum_difference,
        "maximum_changed_pixel_fraction":maximum_changed_fraction,"maximum_channel_difference":maximum_channel_difference,"cases":cases,
        "scope":"Production general/prepared pixel A/B for all14worlds, two canvas sizes, sidebar centres, palettes, mapped/disconnected audio, eight-frame real history, pair→single and delayed fallback→prepared handovers. No timing inference."});
    fs::write(
        destination.join("specialization-parity.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    anyhow::ensure!(
        visually_equivalent,
        "specialized pixels differ: {maximum_difference}"
    );
    println!(
        "Specialized Attractor/Glass visual equivalence passes (mean delta {maximum_difference})"
    );
    Ok(())
}

pub fn preview_new_worlds(destination: &Path) -> anyhow::Result<()> {
    preview_filtered(destination, false)
}

pub fn preview_disco_silk(
    destination: &Path,
    glass_reference: Option<&Path>,
) -> anyhow::Result<()> {
    preview_silk_journey(destination, glass_reference, false)
}

pub fn verify_glass(destination: &Path, reference: &Path) -> anyhow::Result<()> {
    preview_silk_journey(destination, Some(reference), true)
}

/// Bounded evidence for the original coastal flight, using the same prepared
/// world/transition paths as the app. No window or audio source is changed.
pub fn preview_neon_coast(destination: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
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
    renderer.specialize(
        &device,
        &[
            1 << 14,
            1 << 11,
            1 << 9,
            0,
            (1 << 14) | (1 << 11),
            (1 << 14) | (1 << 9),
        ],
    );
    let mut probe = Probe {
        device,
        queue,
        renderer,
    };
    let mappings = AudioMappings {
        bass_to_pulse: 1.5,
        mids_to_geometry: 1.5,
        treble_to_sparkle: 1.5,
        beat_to_flash: 1.5,
    };
    let zero = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let music = music();
    let quiet = AudioFeatures::default();
    let mut failures = Vec::new();
    let mut gallery = Vec::new();
    let mut bands = Vec::new();
    let mut edges = Vec::new();
    for look in 0..10 {
        let u = input(14, look, 18.0, &music, &mappings);
        let pixels = probe.cold(u, SIZE)?;
        gallery.push(pixels);
        if [0, 1, 2, 3, 5, 8].contains(&look) {
            let hero = probe.cold(u, [1600, 1000])?;
            write_png(
                &destination.join(format!("coast-look-{look}-1600.png")),
                &hero,
                [1600, 1000],
            )?;
        }
        let off = probe.cold(input(14, look, 18.0, &music, &zero), SIZE)?;
        let silent = probe.cold(input(14, look, 18.0, &quiet, &zero), SIZE)?;
        let stale = probe.cold(u.with_audio_state(false, 1), SIZE)?;
        let quiet_frame = probe.cold(input(14, look, 18.0, &quiet, &mappings), SIZE)?;
        if off != silent || stale != quiet_frame {
            failures.push(format!("look{look}: gain/stale gate"));
        }
        for band in 0..3 {
            let a = probe.cold(
                input(14, look, 18.0, &band_fixture(band, 0.2), &mappings),
                SIZE,
            )?;
            let b = probe.cold(
                input(14, look, 18.0, &band_fixture(band, 0.8), &mappings),
                SIZE,
            )?;
            let delta = difference(&a, &b);
            let mut disabled = mappings.clone();
            match band {
                0 => disabled.bass_to_pulse = 0.0,
                1 => disabled.mids_to_geometry = 0.0,
                _ => disabled.treble_to_sparkle = 0.0,
            }
            let disconnected = probe.cold(
                input(14, look, 18.0, &band_fixture(band, 0.8), &disabled),
                SIZE,
            )?;
            let silence = probe.cold(input(14, look, 18.0, &quiet, &disabled), SIZE)?;
            let mapping_exact = disconnected == silence;
            if !mapping_exact {
                failures.push(format!("look{look}/band{band}: gain gate"));
            }
            bands.push(
                serde_json::json!({"look":look,"band":band,"ordinary_level_rgb_change":delta,"mapping_zero_exact":mapping_exact}),
            );
            if delta < 0.03 {
                failures.push(format!("look{look}/band{band}: weak response{delta}"));
            }
        }
        // Recipe endpoints must be identical to the corresponding held recipe.
        for (blend, held) in [(0.0, look), (1.0, (look + 1) % 10)] {
            let mut u = input(14, look, 18.0, &music, &mappings);
            u.looks[14] = [look as f32, ((look + 1) % 10) as f32, blend, 18.0];
            let actual = probe.cold(u, SIZE)?;
            let expected = probe.cold(input(14, held, 18.0, &music, &mappings), SIZE)?;
            if actual != expected {
                failures.push(format!("look{look}: recipe endpoint{blend}"));
            }
        }
    }
    write_contact_sheet(&destination.join("coast-ten-looks.png"), &gallery, 5)?;
    let mapping_proof = coast_mapping_proof(&mut probe, destination, &mappings, &mut failures)?;
    let mut journey = Vec::new();
    for look in [0, 5] {
        for time in [18.0, 34.0, 58.0, 82.0] {
            journey.push(probe.cold(input(14, look, time, &music, &mappings), SIZE)?);
        }
    }
    write_contact_sheet(
        &destination.join("coast-day-night-journey.png"),
        &journey,
        4,
    )?;
    for size in [[1280, 800], [900, 1200], [1600, 720]] {
        let image = probe.cold(
            input(14, 1, 18.0, &music, &mappings).with_canvas_centre(-0.32),
            size,
        )?;
        write_png(
            &destination.join(format!("coast-sidebar-{}x{}.png", size[0], size[1])),
            &image,
            size,
        )?;
    }
    for (from, to) in [(11, 14), (14, 9), (12, 14)] {
        let mut strip = Vec::new();
        for blend in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let mut u = input(to, 0, 18.0, &music, &mappings);
            u.values[5][2] = from as f32;
            u.values[5][3] = blend;
            let image = probe.cold(u, SIZE)?;
            if blend == 0.0 || blend == 1.0 {
                let held = if blend == 0.0 { from } else { to };
                let expected = probe.cold(input(held, 0, 18.0, &music, &mappings), SIZE)?;
                let delta = difference(&image, &expected);
                if delta > 0.01 {
                    failures.push(format!("outer{from}→{to}/{blend}: endpoint{delta}"));
                }
            }
            strip.push(image);
        }
        write_contact_sheet(
            &destination.join(format!("coast-transition-{from}-{to}.png")),
            &strip,
            5,
        )?;
    }
    // Crossing persistent building/palm-cell boundaries must remain continuous.
    for spacing in [11.0, 18.0, 104.0, 127.0, 136.0, 231.0] {
        let time = spacing * 9.0 / 5.4;
        let a = probe.cold(input(14, 1, time - 0.0001, &music, &mappings), SIZE)?;
        let b = probe.cold(input(14, 1, time + 0.0001, &music, &mappings), SIZE)?;
        let delta = difference(&a, &b);
        edges.push(serde_json::json!({"spacing":spacing,"time":time,"rgb_difference":delta}));
        if delta > 0.10 {
            failures.push(format!("cell{spacing}: boundary jump{delta}"));
        }
    }
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
        .arg(destination.join("coast-flight-12s.mp4"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok();
    let mut timeline = Vec::new();
    let mut previous = Vec::new();
    let mut maximum_step = 0.0_f64;
    let mut maximum_white = 0.0_f64;
    let mut motion = crate::motion::RenderMotion::new();
    let mut response = crate::music_response::MusicResponse::new();
    let mut musical_contrast = Vec::new();
    for frame in 0..360 {
        let time = 18.0 + frame as f32 / 30.0;
        let band = (frame / 120) as usize;
        let envelope = 0.2
            + 0.6
                * (std::f32::consts::PI * (frame % 120) as f32 / 120.0)
                    .sin()
                    .powi(2);
        let raw = band_fixture(band, envelope);
        let rendered = motion.update(&raw, 1.0 / 30.0);
        let musical = response.update(&raw, &mappings, true, 700, 1.0 / 30.0);
        let mut u = input(14, 1, time, &rendered, &mappings)
            .with_music_response(musical)
            .with_audio_state(true, 700);
        u.values[6][1] = 1.0 / 30.0;
        u.values[8][0] = frame as f32 / 30.0;
        let pixels = probe.draw(u, SIZE)?;
        if !previous.is_empty() {
            maximum_step = maximum_step.max(difference(&previous, &pixels));
        }
        maximum_white = maximum_white.max(
            pixels
                .chunks_exact(4)
                .filter(|p| p[0] > 245 && p[1] > 245 && p[2] > 245)
                .count() as f64
                / 256000.0,
        );
        if frame % 30 == 0 {
            timeline.push(pixels.clone());
            let control = probe.cold(input(14, 1, time, &quiet, &mappings), SIZE)?;
            musical_contrast.push(serde_json::json!({"time":time,"band":band,"input_level":envelope,"same_time_quiet_rgb_difference":difference(&control,&pixels)}));
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
    let encoded = video.as_mut().is_some_and(|encoder| {
        encoder.stdin.take();
        encoder.wait().is_ok_and(|s| s.success())
    });
    write_contact_sheet(&destination.join("coast-flight-times.png"), &timeline, 3)?;
    if maximum_step > 9.0 || maximum_white > 0.02 {
        failures.push(format!("temporal{maximum_step}/white{maximum_white}"));
    }
    fs::write(
        destination.join("coast-report.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"status":if failures.is_empty(){"pass"}else{"fail"},"gpu":adapter.get_info().name,"bands":bands,"object_mapping":mapping_proof,"cell_boundaries":edges,"music_timeline":musical_contrast,"maximum_adjacent_rgb_difference":maximum_step,"maximum_white_fraction":maximum_white,"clip_encoded":encoded,"failures":failures,"scope":"Actual prepared renderer, synthetic music; ten looks, recipe/outer endpoints, persistent cell/curved route boundaries, aspect/sidebar samples and 12s RenderMotion/MusicResponse-smoothed flight. Timeline bands are bass0–4s, mids4–8s, highs8–12s; control frames have identical camera time. Journey rows are looks0/5, columns18/34/58/82s. Not live music or display FPS."}),
        )?,
    )?;
    anyhow::ensure!(
        failures.is_empty(),
        "Neon Coast findings: {}",
        failures.join("; ")
    );
    Ok(())
}

/// Masks use the real Coast geometry, while comparisons use the COMPLETE
/// production compositor. This catches generic material/phrase contamination.
fn coast_mapping_proof(
    production: &mut Probe,
    destination: &Path,
    mappings: &AudioMappings,
    failures: &mut Vec<String>,
) -> anyhow::Result<serde_json::Value> {
    let mut mask = Probe {
        device: production.device.clone(),
        queue: production.queue.clone(),
        renderer: VisualResources::new(&production.device, HISTORY_FORMAT),
    };
    mask.fragment_probe("fs_coast_mask", r#"
@fragment fn fs_coast_mask(in: VertexOut) -> @location(0) vec4<f32> {
    let uv = (in.uv * 2.0 - 1.0) * vec2<f32>(u.a.x/u.a.y,1.0) - vec2<f32>(u.g.w,0.0);
    let look = coast_look(); let audio = coast_audio(); let time = u.a.z; let travel = time*5.4;
    let hue = look.colour.x + sin(time*0.011)*0.075 + (coast_landscape(travel/167.0,313.0)-0.5)*0.10;
    let sky = coast_sky(uv,look,audio,hue,time);
    let ray = vec3<f32>(uv.x/1.2+coast_heading(travel),(-0.11-uv.y)/1.2,1.0);
    let city = coast_buildings(sky.colour,sky.ambient,ray,vec3<f32>(0.0,2.7,0.0),travel,look,audio,hue);
    let palms = coast_palms(city,uv,travel,look,audio,hue);
    let clear = all(palms == city.colour);
    return vec4<f32>(select(0.0,1.0,city.coverage==1.0 && clear),
        select(0.0,1.0,city.coverage==0.0 && clear && uv.y < -0.12),
        select(0.0,1.0,city.coverage==0.0 && clear && uv.y > 0.10),1.0);
}"#);
    let mut results = Vec::new();
    let mut paired = Vec::new();
    for look in [0, 5] {
        for band in 0..3 {
            let a = warm_music_input(
                production,
                14,
                look,
                &band_fixture(band, 0.2),
                mappings,
                800 + look as u64 * 10 + band as u64,
            );
            let b = warm_music_input(
                production,
                14,
                look,
                &band_fixture(band, 0.8),
                mappings,
                900 + look as u64 * 10 + band as u64,
            );
            let low = production.cold(a, SIZE)?;
            let high = production.cold(b, SIZE)?;
            let mask_low = mask.cold(a, SIZE)?;
            let mask_high = mask.cold(b, SIZE)?;
            for (channel, name, intended) in [
                (0, "opaque_buildings", band == 1),
                (1, "clear_sky", band == 0),
                (2, "road_water", true),
            ] {
                let mut count = 0;
                let mut sum = 0_u64;
                let mut max = 0_u8;
                for ((x, y), (mx, my)) in low
                    .chunks_exact(4)
                    .zip(high.chunks_exact(4))
                    .zip(mask_low.chunks_exact(4).zip(mask_high.chunks_exact(4)))
                {
                    if mx[channel] > 100 && my[channel] > 100 {
                        count += 1;
                        for c in 0..3 {
                            let d = x[c].abs_diff(y[c]);
                            sum += d as u64;
                            max = max.max(d);
                        }
                    }
                }
                let delta = sum as f64 / (count.max(1) * 3) as f64;
                if count < 100 || (intended && delta < 0.015) || (!intended && max > 0) {
                    failures.push(format!(
                        "Coast look{look}/{name}/band{band}: count{count}, delta{delta}, max{max}"
                    ));
                }
                results.push(serde_json::json!({"look":look,"band":band,"region":name,"intended":intended,"pixels":count,"mean_rgb_codes":delta,"max_code":max}));
            }
            paired.push(low);
            paired.push(high);
        }
    }
    write_contact_sheet(&destination.join("coast-object-bands.png"), &paired, 2)?;
    let voice_base = input(14, 5, 18.0, &music(), mappings);
    let voice_on = voice_base.with_vocal_response([0.8, 1.0, 0.7, 0.0]);
    let voice_delta = difference(
        &production.cold(voice_base, SIZE)?,
        &production.cold(voice_on, SIZE)?,
    );
    let mut no_mids = mappings.clone();
    no_mids.mids_to_geometry = 0.0;
    let disconnected = input(14, 5, 18.0, &music(), &no_mids);
    let voice_gate = production.cold(disconnected, SIZE)?
        == production.cold(disconnected.with_vocal_response([0.8, 1.0, 0.7, 0.0]), SIZE)?;
    let stale_voice = production.cold(voice_on.with_audio_state(false, 2), SIZE)?
        == production.cold(voice_base.with_audio_state(false, 2), SIZE)?;
    if voice_delta < 0.01 || !voice_gate || !stale_voice {
        failures.push(format!(
            "Coast facade voice: response{voice_delta}, gate{voice_gate}, stale{stale_voice}"
        ));
    }
    results.push(serde_json::json!({"region":"facade_vocal_entry","mean_rgb_codes":voice_delta,"mids_gain_zero_exact":voice_gate,"stale_exact":stale_voice}));
    mask.fragment_probe("fs_coast_palm",r#"
@fragment fn fs_coast_palm(in: VertexOut) -> @location(0) vec4<f32> {
    let uv=(in.uv*2.0-1.0)*vec2<f32>(u.a.x/u.a.y,1.0);
    let look=coast_look(); let time=u.a.z; let travel=time*5.4;
    let hue=look.colour.x+sin(time*0.011)*0.075+(coast_landscape(travel/167.0,313.0)-0.5)*0.10;
    return vec4<f32>(coast_palms(CoastSurface(vec3<f32>(0.6),10000.0,0.0),uv,travel,look,coast_audio(),hue),1.0);
}"#);
    let mut palm_pairs = Vec::new();
    for band in 0..3 {
        let a = warm_music_input(
            production,
            14,
            1,
            &band_fixture(band, 0.2),
            mappings,
            1000 + band as u64,
        );
        let b = warm_music_input(
            production,
            14,
            1,
            &band_fixture(band, 0.8),
            mappings,
            1010 + band as u64,
        );
        let low = mask.cold(a, SIZE)?;
        let high = mask.cold(b, SIZE)?;
        let delta = difference(&low, &high);
        if (band < 2 && low != high) || (band == 2 && delta < 0.03) {
            failures.push(format!("Coast palm isolation band{band}:{delta}"));
        }
        results.push(serde_json::json!({"band":band,"region":"isolated_real_palm_geometry_and_material","mean_rgb_codes":delta,"exact_equal":low==high}));
        if band == 2 {
            palm_pairs.push(low);
            palm_pairs.push(high);
        }
    }
    write_contact_sheet(
        &destination.join("coast-highs-palm-detail.png"),
        &palm_pairs,
        2,
    )?;
    mask.fragment_probe("fs_coast_occlusion",r#"
@fragment fn fs_coast_occlusion(in: VertexOut) -> @location(0) vec4<f32> {
    let uv=(in.uv*2.0-1.0)*vec2<f32>(u.a.x/u.a.y,1.0);
    let look=coast_look();let time=u.a.z;let travel=time*5.4;let audio=coast_audio();
    let hue=look.colour.x+sin(time*0.011)*0.075+(coast_landscape(travel/167.0,313.0)-0.5)*0.10;
    let sky=coast_sky(uv,look,audio,hue,time);
    let ray=vec3<f32>(uv.x/1.2+coast_heading(travel),(-0.11-uv.y)/1.2,1.0);
    let a=coast_buildings(vec3<f32>(0.0),sky.ambient,ray,vec3<f32>(0.0,2.7,0.0),travel,look,audio,hue);
    let b=coast_buildings(vec3<f32>(2.0,1.0,0.5),sky.ambient,ray,vec3<f32>(0.0,2.7,0.0),travel,look,audio,hue);
    let eligible=a.coverage==1.0 && distance(uv,coast_orb_position(time))<0.075;
    return vec4<f32>(select(0.0,length(a.colour-b.colour)*1000.0,eligible),select(0.0,1.0,eligible),0.0,1.0);
}"#);
    let mut occultation = Vec::new();
    for (look, time) in [(0, 73.75), (5, 8.25), (3, 24.5)] {
        let u = input(14, look, time, &music(), mappings);
        let check = mask.cold(u, SIZE)?;
        let count = check.chunks_exact(4).filter(|p| p[1] > 100).count();
        let leak = check.chunks_exact(4).map(|p| p[0]).max().unwrap_or(0);
        if count < 30 || leak > 0 {
            failures.push(format!(
                "Coast orb occlusion look{look}/t{time}: pixels{count}/leak{leak}"
            ));
        }
        results.push(serde_json::json!({"look":look,"time":time,"region":"opaque_city_inside_orb","pixels":count,"base_light_leak_max_code":leak}));
        occultation.push(production.cold(u, SIZE)?);
        write_png(
            &destination.join(format!("coast-orb-occluded-{look}.png")),
            &production.cold(u, [1600, 1000])?,
            [1600, 1000],
        )?;
    }
    write_contact_sheet(
        &destination.join("coast-orb-occlusion.png"),
        &occultation,
        3,
    )?;
    Ok(serde_json::json!(results))
}

fn band_fixture(band: usize, level: f32) -> AudioFeatures {
    let mut audio = AudioFeatures {
        energy: level.max(0.12),
        ..Default::default()
    };
    match band {
        0 => audio.bass = level,
        1 => audio.mids = level,
        _ => audio.treble = level,
    }
    let (start, end) = [(0, 8), (8, 24), (24, 48)][band];
    for row in start..end {
        audio.spectrum[row] = level * (0.85 + 0.15 * (row as f32 * 0.37).sin().powi(2));
    }
    audio
}

/// Settle the real RenderMotion and raster filters without moving the camera.
/// The raster's distinct physical snapshot stamp must advance even here.
fn warm_music_input(
    probe: &mut Probe,
    world: usize,
    look: usize,
    audio: &AudioFeatures,
    mappings: &AudioMappings,
    epoch: u64,
) -> ShaderUniforms {
    let mut motion = crate::motion::RenderMotion::new();
    let mut response = crate::music_response::MusicResponse::new();
    let mut final_input = input(world, look, 18.0, audio, mappings);
    for frame in 0..60 {
        let rendered = motion.update(audio, DT);
        let envelope = response.update(audio, mappings, true, epoch, DT);
        final_input = input(world, look, 18.0, &rendered, mappings)
            .with_music_response(envelope)
            .with_audio_state(true, epoch);
        final_input.values[8][0] = frame as f32 * DT;
        probe
            .renderer
            .raster
            .prepare(&probe.queue, &mut final_input);
    }
    // No additional filter step when drawing the already-prepared snapshot.
    final_input
}

/// Music contrast at ordinary levels and real temporal smoothing. This is
/// deliberately scoped to Silk/Aurora; no native capture or extra runtime work.
pub fn preview_music_response(destination: &Path) -> anyhow::Result<()> {
    preview_music_worlds(destination, &[12, 9])
}

pub fn preview_aurora_response(destination: &Path) -> anyhow::Result<()> {
    preview_music_worlds(destination, &[9])
}

fn preview_music_worlds(destination: &Path, worlds: &[usize]) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
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
    let make_probe = || {
        let mut renderer = VisualResources::new(&device, HISTORY_FORMAT);
        renderer.specialize(&device, &[0, 1 << 9]);
        Probe {
            device: device.clone(),
            queue: queue.clone(),
            renderer,
        }
    };
    let mappings = AudioMappings {
        bass_to_pulse: 1.5,
        mids_to_geometry: 1.5,
        treble_to_sparkle: 1.5,
        beat_to_flash: 1.5,
    };
    let zero = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let quiet = AudioFeatures::default();
    let mut epoch = 20;
    let mut reports = Vec::new();
    let mut failures = Vec::new();
    for &world in worlds {
        let mut production = make_probe();
        let mut colour = make_probe();
        let mut geometry = make_probe();
        if world == 12 {
            colour.isolate_silk();
            geometry.renderer.raster.probe_silk_normals(&device);
            geometry.isolate_silk();
        } else {
            geometry.fragment_probe(
                "fs_aurora_geometry",
                r#"
@fragment fn fs_aurora_geometry(in: VertexOut) -> @location(0) vec4<f32> {
    let audio = aurora_audio();
    let phase = u.a.z * 0.13 + sin(u.a.z * 0.047) * 0.36;
    let lane = select(0.0, 1.0, in.uv.y > 0.5);
    let point = aurora_edge(in.uv.x * 2.0 - 1.0, lane, phase, aurora_look(), audio.x, audio.y);
    return vec4<f32>(vec3<f32>(0.5) + point * 0.025, 1.0);
}"#,
            );
        }
        let mut static_cases = Vec::new();
        let mut looks = Vec::new();
        for look in 0..10 {
            let mut mixed = band_fixture(0, 0.5);
            mixed.mids = 0.5;
            mixed.treble = 0.5;
            mixed.spectrum.fill(0.5);
            epoch += 1;
            let u = warm_music_input(&mut production, world, look, &mixed, &mappings, epoch);
            looks.push(production.cold(u, SIZE)?);
            let mut band_sheet = Vec::new();
            let mut geometry_sheet = Vec::new();
            for band in 0..3 {
                let mut colours = Vec::new();
                let mut shapes = Vec::new();
                for level in [0.2, 0.5, 0.8] {
                    let audio = band_fixture(band, level);
                    epoch += 1;
                    let u = warm_music_input(&mut colour, world, look, &audio, &mappings, epoch);
                    colours.push(colour.cold(u, SIZE)?);
                    let u = warm_music_input(&mut geometry, world, look, &audio, &mappings, epoch);
                    shapes.push(geometry.cold(u, SIZE)?);
                }
                let contrasts = [
                    difference(&colours[0], &colours[1]),
                    difference(&colours[1], &colours[2]),
                ];
                let shape_changes = [
                    difference(&shapes[0], &shapes[1]),
                    difference(&shapes[1], &shapes[2]),
                ];
                if contrasts.iter().any(|v| *v < 0.08) {
                    failures.push(format!(
                        "world{world}/look{look}/band{band}: normal-level contrast {contrasts:?}"
                    ));
                }
                if band < 2 && shape_changes.iter().any(|v| *v < 0.03) {
                    failures.push(format!(
                        "world{world}/look{look}/band{band}: geometry contrast {shape_changes:?}"
                    ));
                }
                // A disconnected band must not appear via another channel.
                let mut disconnected = mappings.clone();
                match band {
                    0 => disconnected.bass_to_pulse = 0.0,
                    1 => disconnected.mids_to_geometry = 0.0,
                    _ => disconnected.treble_to_sparkle = 0.0,
                }
                epoch += 1;
                let u = warm_music_input(
                    &mut colour,
                    world,
                    look,
                    &band_fixture(band, 0.8),
                    &disconnected,
                    epoch,
                );
                let off = colour.cold(u, SIZE)?;
                epoch += 1;
                let u = warm_music_input(&mut colour, world, look, &quiet, &disconnected, epoch);
                let silent = colour.cold(u, SIZE)?;
                if off != silent {
                    failures.push(format!(
                        "world{world}/look{look}/band{band}: disconnected route leaks"
                    ));
                }
                static_cases.push(serde_json::json!({"look":look,"band":(["bass","mids","highs"][band]),
                    "raw_levels":[0.2,0.5,0.8],"gain":1.5,"rgb_steps":contrasts,"geometry_steps":shape_changes,"mapping_zero_exact":off==silent}));
                band_sheet.extend(colours);
                geometry_sheet.extend(shapes);
            }
            write_contact_sheet(
                &destination.join(format!("world-{world}-look-{look}-bands.png")),
                &band_sheet,
                3,
            )?;
            if look == 0 {
                write_contact_sheet(
                    &destination.join(format!("world-{world}-geometry.png")),
                    &geometry_sheet,
                    3,
                )?;
            }
            epoch += 1;
            let u = warm_music_input(&mut colour, world, look, &mixed, &zero, epoch);
            let disconnected = colour.cold(u, SIZE)?;
            let u = warm_music_input(&mut colour, world, look, &quiet, &zero, epoch + 1);
            let silent = colour.cold(u, SIZE)?;
            epoch += 2;
            if disconnected != silent {
                failures.push(format!("world{world}/look{look}: all-zero leak"));
            }
            let u = warm_music_input(&mut colour, world, look, &mixed, &mappings, epoch);
            let stale = colour.cold(u.with_audio_state(false, epoch), SIZE)?;
            let u = warm_music_input(&mut colour, world, look, &quiet, &mappings, epoch + 1);
            let silent = colour.cold(u, SIZE)?;
            epoch += 2;
            if stale != silent {
                failures.push(format!("world{world}/look{look}: stale leak"));
            }
        }
        write_contact_sheet(
            &destination.join(format!("world-{world}-ten-looks.png")),
            &looks,
            5,
        )?;
        println!(
            "Music response world{world}: ten looks and ordinary-level geometry/bands rendered"
        );
        let mut control = make_probe();
        let mut motion = crate::motion::RenderMotion::new();
        let mut response = crate::music_response::MusicResponse::new();
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
                "640x200",
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
            .arg(destination.join(format!("world-{world}-music-12s.mp4")))
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok();
        let smooth = |x: f32| {
            let v = x.clamp(0.0, 1.0);
            v * v * (3.0 - 2.0 * v)
        };
        let mut previous = Vec::new();
        let mut maximum_step = 0.0_f64;
        let mut maximum_white = 0.0_f64;
        let mut points = Vec::new();
        let mut timeline = Vec::new();
        for frame in 0..360 {
            let elapsed = frame as f32 / 30.0;
            let band = (frame / 120).min(2) as usize;
            let t = elapsed % 4.0;
            let level = 0.2 + 0.3 * smooth((t - 0.5) / 0.75) + 0.3 * smooth((t - 2.0) / 0.75)
                - 0.6 * smooth((t - 3.25) / 0.75);
            let raw = band_fixture(band, level);
            let rendered = motion.update(&raw, 1.0 / 30.0);
            let material = response.update(&raw, &mappings, true, epoch, 1.0 / 30.0);
            let time = 18.0 + elapsed * 0.8;
            let mut u = input(world, 0, time, &rendered, &mappings)
                .with_music_response(material)
                .with_audio_state(true, epoch);
            u.values[6][1] = 1.0 / 30.0;
            u.values[8][0] = elapsed;
            let actual = production.draw(u, [320, 200])?;
            let mut u = input(world, 0, time, &quiet, &mappings).with_audio_state(true, epoch);
            u.values[6][1] = 1.0 / 30.0;
            u.values[8][0] = elapsed;
            let reference = control.draw(u, [320, 200])?;
            if !previous.is_empty() {
                maximum_step = maximum_step.max(difference(&previous, &actual));
            }
            let white = actual
                .chunks_exact(4)
                .filter(|p| p[0] > 245 && p[1] > 245 && p[2] > 245)
                .count() as f64
                / 64_000.0;
            maximum_white = maximum_white.max(white);
            let mut pair = Vec::with_capacity(640 * 200 * 4);
            for y in 0..200 {
                let start = y * 320 * 4;
                pair.extend_from_slice(&reference[start..start + 320 * 4]);
                pair.extend_from_slice(&actual[start..start + 320 * 4]);
            }
            if frame % 30 == 0 {
                points.push(serde_json::json!({"seconds":elapsed,"band":(["bass","mids","highs"][band]),"raw_level":level,"music_rgb_difference":difference(&reference,&actual)}));
                timeline.push(pair.clone());
            }
            if let Some(encoder) = &mut video
                && let Some(stdin) = &mut encoder.stdin
            {
                use std::io::Write as _;
                if stdin.write_all(&pair).is_err() {
                    encoder.stdin.take();
                }
            }
            previous = actual;
        }
        let encoded = video.as_mut().is_some_and(|encoder| {
            encoder.stdin.take();
            encoder.wait().is_ok_and(|s| s.success())
        });
        // The common contact writer assumes640×400. Stack each pair over a
        // neutral second half so the existing output convention stays honest.
        for image in &mut timeline {
            image.resize((640 * 400 * 4) as usize, 0);
            for pixel in image[640 * 200 * 4..].chunks_exact_mut(4) {
                pixel[3] = 255;
            }
        }
        write_contact_sheet(
            &destination.join(format!("world-{world}-temporal.png")),
            &timeline,
            3,
        )?;
        if maximum_step > 9.0 || maximum_white > 0.02 {
            failures.push(format!(
                "world{world}: temporal step{maximum_step}/white{maximum_white}"
            ));
        }
        reports.push(serde_json::json!({"world":world,"static":static_cases,"clip":{"seconds":12,"frames":360,"size":[640,200],"encoded":encoded,
            "columns":["quiet control","changing music"],"bands":"bass0–4s, mids4–8s, highs8–12s","fixed_clock_rate":0.8,
            "maximum_adjacent_rgb_difference":maximum_step,"maximum_white_fraction":maximum_white,"points":points}}));
    }
    fs::write(
        destination.join("music-response-report.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
        "status":if failures.is_empty(){"pass"}else{"fail"},"gpu":adapter.get_info().name,"worlds":reports,"failures":failures,
        "scope":"Synthetic ordinary-range0.2/0.5/0.8 at gain1.5 through real RenderMotion and raster history; Silk isolated normals/material, Aurora physical edge coordinates; fixed-clock paired12s clips. Not live music, display FPS or every transient."}))?,
    )?;
    anyhow::ensure!(
        failures.is_empty(),
        "Music response findings: {}",
        failures.join("; ")
    );
    Ok(())
}

/// Scoped production proof for immersive Silk. No native window/audio capture.
fn preview_silk_journey(
    destination: &Path,
    glass_reference: Option<&Path>,
    glass_only: bool,
) -> anyhow::Result<()> {
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
    let adapter_name = adapter.get_info().name;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
    let mut renderer = VisualResources::new(&device, HISTORY_FORMAT);
    renderer.specialize(&device, &[0, 1 << 11, 1 << 13]);
    let mut probe = Probe {
        device,
        queue,
        renderer,
    };
    let music = music();
    let quiet = AudioFeatures::default();
    let mappings = AudioMappings::default();
    let zero = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let mut failures = Vec::new();

    // Reproduce the exact .22 Glass fixture (including its real eight frames)
    // before any Silk probe. Compare encoded bytes, not a relaxed art metric.
    let mut glass_frames = Vec::new();
    let prepared = std::mem::take(&mut probe.renderer.prepared_pipelines);
    for prepared_pass in [false, true] {
        if prepared_pass {
            probe.renderer.prepared_pipelines = prepared.clone();
        }
        probe.renderer.history = None;
        for frame in 0..8 {
            let mut u = input(11, 0, 12.0 + frame as f32 * DT, &music, &mappings)
                .with_canvas_centre(0.0)
                .with_palette_mix(0.0);
            u.values[6][0] = 0.38;
            let pixels = probe.draw(u, SIZE)?;
            if frame == 7 {
                glass_frames.push(pixels);
            }
        }
    }
    let glass_path = destination.join("glass-regression.png");
    write_contact_sheet(&glass_path, &glass_frames, 2)?;
    let glass_same = glass_reference
        .map(|path| -> anyhow::Result<bool> { Ok(fs::read(path)? == fs::read(&glass_path)?) })
        .transpose()?;
    if glass_same == Some(false) {
        failures.push("Glass reference pixels changed".into());
    }
    if glass_only {
        fs::write(
            destination.join("glass-regression.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "status":if glass_same == Some(true) {"pass"} else {"fail"},
                "reference":glass_reference,"encoded_png_exact":glass_same,
                "scope":"Exact .22 fixture: eight production history frames, general and prepared Glass,640×400,look0,time12s. Shader/source preservation covers unprobed inputs."
            }))?,
        )?;
        anyhow::ensure!(
            glass_same == Some(true),
            "Glass reference pixels changed or absent"
        );
        return Ok(());
    }

    let mut gallery = Vec::new();
    let mut routes = Vec::new();
    let mut continuity = Vec::new();
    for (look, name) in LOOKS[12].iter().enumerate() {
        let frame = probe.cold(input(12, look, 18.0, &music, &mappings), SIZE)?;
        write_png(
            &destination.join(format!("silk-look-{look}.png")),
            &frame,
            SIZE,
        )?;
        gallery.push(frame);
        let disconnected = probe.cold(input(12, look, 18.0, &music, &zero), SIZE)?;
        let disconnected_quiet = probe.cold(input(12, look, 18.0, &quiet, &zero), SIZE)?;
        if difference(&disconnected, &disconnected_quiet) != 0.0 {
            failures.push(format!("{name}: zero gain leak"));
        }
        let mut time_frames = Vec::new();
        for time in [0.0, 6.0, 12.0, 20.0, 32.0, 44.0, 60.0, 7200.0, 21600.0] {
            let a = probe.cold(input(12, look, time, &music, &mappings), SIZE)?;
            let b = probe.cold(input(12, look, time + DT, &music, &mappings), SIZE)?;
            let delta = difference(&a, &b);
            continuity.push(serde_json::json!({"look":look,"seconds":time,"adjacent_frame_rgb_difference":delta}));
            if delta > 12.0 {
                failures.push(format!(
                    "{name}: large adjacent-frame change at{time}s ({delta})"
                ));
            }
            time_frames.push(a);
        }
        write_contact_sheet(
            &destination.join(format!("silk-look-{look}-journey.png")),
            &time_frames,
            3,
        )?;
    }
    write_contact_sheet(&destination.join("silk-ten-looks.png"), &gallery, 5)?;
    println!("Silk ten looks, FFT routes and long-time probes rendered");

    // A real held timeline: every frame advances the production ping-pong
    // renderer. Smooth synthetic phrases vary energy without changing the look.
    // Smaller readback bounds diagnostic cost; art sheets above use640×400.
    let journey_size = [320, 200];
    probe.renderer.history = None;
    let mut previous = Vec::new();
    let mut changes = Vec::new();
    let mut second_mean = Vec::new();
    let mut second_total = 0.0;
    // Optional standard encoder consumes pixels directly: no large frame dump
    // or player. The diagnostic works without ffmpeg too.
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
            "320x200",
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
        .arg(destination.join("silk-held-60s.mp4"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok();
    for frame in 0..=3600 {
        let time = frame as f32 * DT;
        let mut phrase = music.clone();
        let energy = 0.55 + 0.35 * (time * 0.27).sin();
        phrase.bass *= energy;
        phrase.mids *= 0.65 + 0.25 * (time * 0.19 + 1.0).sin();
        phrase.treble *= 0.65 + 0.25 * (time * 0.31 + 2.0).sin();
        phrase.beat *= (time * 2.0).sin().max(0.0).powi(6);
        for (i, band) in phrase.spectrum.iter_mut().enumerate() {
            *band *= energy * (0.8 + 0.2 * (time * 0.41 + i as f32 * 0.13).sin());
        }
        let pixels = probe.draw(input(12, 0, time, &phrase, &mappings), journey_size)?;
        if !previous.is_empty() {
            let delta = difference(&previous, &pixels);
            changes.push(delta);
            second_total += delta;
            if frame % 60 == 0 {
                second_mean.push(second_total / 60.0);
                second_total = 0.0;
            }
        }
        if frame % 300 == 0 {
            write_png(
                &destination.join(format!("held-{:02}s.png", frame / 60)),
                &pixels,
                journey_size,
            )?;
        }
        if frame % 2 == 0
            && let Some(encoder) = &mut video
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
    let maximum_step = changes.iter().copied().fold(0.0, f64::max);
    let mean_step = changes.iter().sum::<f64>() / changes.len() as f64;
    if maximum_step > 12.0 {
        failures.push(format!("held60s: large frame step {maximum_step}"));
    }
    if second_mean.iter().any(|movement| *movement < 0.001) {
        failures.push("held60s: stationary full-second interval".into());
    }
    println!("Silk actual60s held journey completed");

    let mut endpoints = Vec::new();
    for (from, to) in [(0, 9), (3, 7), (6, 1)] {
        let mut frames = Vec::new();
        for amount in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let mut u = input(12, to, 18.0, &music, &mappings);
            u.looks[12] = [from as f32, to as f32, amount, 18.0];
            frames.push(probe.cold(u, SIZE)?);
        }
        let first = probe.cold(input(12, from, 18.0, &music, &mappings), SIZE)?;
        let last = probe.cold(input(12, to, 18.0, &music, &mappings), SIZE)?;
        let delta = difference(&frames[0], &first).max(difference(&frames[4], &last));
        if delta != 0.0 {
            failures.push(format!("look{from}→{to}: endpoint changed"));
        }
        endpoints.push(
            serde_json::json!({"type":"look","from":from,"to":to,"endpoint_rgb_difference":delta}),
        );
        write_contact_sheet(
            &destination.join(format!("silk-look-morph-{from}-{to}.png")),
            &frames,
            5,
        )?;
    }
    for (from, to) in [(11, 12), (12, 13), (7, 12), (12, 11)] {
        let mut frames = Vec::new();
        for amount in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let mut u = input(to, 0, 18.0, &music, &mappings).with_auto_mix(0.75, 1.7);
            u.values[5][2] = from as f32;
            u.values[5][3] = amount;
            frames.push(probe.cold(u, SIZE)?);
        }
        let first = probe.cold(
            input(from, 0, 18.0, &music, &mappings).with_auto_mix(0.75, 1.7),
            SIZE,
        )?;
        let last = probe.cold(
            input(to, 0, 18.0, &music, &mappings).with_auto_mix(0.75, 1.7),
            SIZE,
        )?;
        let delta = difference(&frames[0], &first).max(difference(&frames[4], &last));
        if delta > 0.0002 {
            failures.push(format!("world{from}→{to}: endpoint changed ({delta})"));
        }
        endpoints.push(
            serde_json::json!({"type":"world","from":from,"to":to,"endpoint_rgb_difference":delta}),
        );
        write_contact_sheet(
            &destination.join(format!("silk-world-morph-{from}-{to}.png")),
            &frames,
            5,
        )?;
    }

    // Full-canvas geometry may leave all four edges; inspect actual isolated
    // mesh coverage, not the old 'every vertex fits inside a margin' rule.
    probe.isolate_silk();
    // Isolate the actual Silk material for causality: the approved shared
    // background also reacts to bands, and must not create a false pass here.
    for (look, name) in LOOKS[12].iter().enumerate() {
        let baseline = probe.cold(input(12, look, 18.0, &quiet, &mappings), SIZE)?;
        for (label, begin, end) in [("bass", 0, 8), ("mids", 8, 24), ("highs", 24, 48)] {
            let mut band = quiet.clone();
            for bin in begin..end {
                band.spectrum[bin] = 0.30 + (bin as f32 * 0.41).sin().powi(2) * 0.45;
            }
            let actual = probe.cold(input(12, look, 18.0, &band, &mappings), SIZE)?;
            let delta = difference(&baseline, &actual);
            if delta <= 0.015 {
                failures.push(format!(
                    "{name}: isolated {label} FFT route too quiet ({delta})"
                ));
            }
            routes.push(serde_json::json!({"look":look,"name":name,"band":label,"isolated_mesh_rgb_difference":delta}));
        }
        let stale = probe.cold(
            input(12, look, 18.0, &music, &mappings).with_audio_state(false, 1),
            SIZE,
        )?;
        let stale_quiet = probe.cold(
            input(12, look, 18.0, &quiet, &mappings).with_audio_state(false, 1),
            SIZE,
        )?;
        if difference(&stale, &stale_quiet) != 0.0 {
            failures.push(format!("{name}: isolated stale music leak"));
        }
    }
    let maximum = AudioFeatures {
        bass: 1.5,
        mids: 1.5,
        treble: 1.5,
        beat: 1.5,
        spectrum: vec![1.5; 48],
        ..music
    };
    let maximum_mappings = AudioMappings {
        bass_to_pulse: 1.5,
        mids_to_geometry: 1.5,
        treble_to_sparkle: 1.5,
        beat_to_flash: 1.5,
    };
    let mut coverage = Vec::new();
    for (layout, size, centre) in [
        ("wide", SIZE, 0.0),
        ("sidebar", SIZE, -0.4),
        ("portrait", [360, 640], 0.0),
    ] {
        for look in 0..10 {
            for time in [0.0, 18.0, 42.0] {
                let pixels = probe.cold(
                    input(12, look, time, &maximum, &maximum_mappings).with_canvas_centre(centre),
                    size,
                )?;
                let occupied = pixels
                    .chunks_exact(4)
                    .filter(|p| p[..3].iter().any(|c| *c > 3))
                    .count() as f64
                    / (size[0] * size[1]) as f64;
                let white = pixels
                    .chunks_exact(4)
                    .filter(|p| p[..3].iter().all(|c| *c > 250))
                    .count() as f64
                    / (size[0] * size[1]) as f64;
                if occupied < 0.02 || white > 0.10 {
                    failures.push(format!(
                        "{look}/{layout}/{time}: coverage{occupied}, white{white}"
                    ));
                }
                if time == 18.0 {
                    write_png(
                        &destination.join(format!("silk-envelope-{layout}-{look}.png")),
                        &pixels,
                        size,
                    )?;
                }
                coverage.push(serde_json::json!({"look":look,"layout":layout,"time":time,"mesh_coverage":occupied,"white_fraction":white}));
            }
        }
    }
    let report = serde_json::json!({"status":if failures.is_empty(){"pass"}else{"fail"},"adapter":adapter_name,
        "glass_same_encoded_pixels":glass_same,"glass_reference":glass_reference,"world":12,"looks":10,"fft_routes":routes,
        "time_probes":continuity,"held_journey":{"seconds":60,"frames":3601,"size":journey_size,"silent_video_encoded":video_encoded,"maximum_adjacent_rgb_difference":maximum_step,"mean_adjacent_rgb_difference":mean_step,"per_second_mean_difference":second_mean},
        "endpoints":endpoints,"immersive_envelopes":coverage,"failures":failures,
        "scope":"Prepared production raster+compositor; synthetic FFT with fixed choreography, zero-gain/stale, full60s held simulation and long-time adjacent frames. Edge coverage is intentional. Exact Glass PNG regression when supplied. This is not live-audio or display-FPS acceptance."});
    fs::write(
        destination.join("silk-journey-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    anyhow::ensure!(
        failures.is_empty(),
        "Silk journey findings: {}",
        failures.join("; ")
    );
    println!("Silk immersive journey checks passed");
    Ok(())
}

fn preview_filtered(destination: &Path, silk_only: bool) -> anyhow::Result<()> {
    fs::create_dir_all(destination)?;
    let report_path = destination.join("new-worlds-report.json");
    fs::write(
        &report_path,
        br#"{"status":"incomplete","input":"synthetic"}"#,
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
    .context("no GPU adapter for the new-world preview")?;
    let info = adapter.get_info();
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("amara new-world production preview"),
        ..Default::default()
    }))?;
    let renderer = VisualResources::new(&device, HISTORY_FORMAT);
    let mut probe = Probe {
        device,
        queue,
        renderer,
    };
    let mappings = AudioMappings::default();
    let zero = AudioMappings {
        bass_to_pulse: 0.0,
        mids_to_geometry: 0.0,
        treble_to_sparkle: 0.0,
        beat_to_flash: 0.0,
    };
    let quiet = AudioFeatures::default();
    let music = music();
    let mut failures = Vec::new();
    let mut worlds = Vec::new();
    for world in 11..14 {
        if silk_only && world != 12 {
            continue;
        }
        let mut gallery = Vec::new();
        let mut routes = Vec::new();
        for (look, name) in LOOKS[world].iter().enumerate() {
            let frame = probe.fixture(input(world, look, 18.0, &music, &mappings))?;
            write_png(
                &destination.join(format!("world-{world}-look-{look}.png")),
                &frame,
                SIZE,
            )?;
            let hot = frame
                .chunks_exact(4)
                .filter(|p| p[0] > 250 && p[1] > 250 && p[2] > 250)
                .count() as f64
                / (SIZE[0] * SIZE[1]) as f64;
            if hot > 0.10 {
                failures.push(format!("{name}: white clipping fraction {hot}"));
            }
            gallery.push(frame);
            let baseline = probe.cold(input(world, look, 18.0, &quiet, &mappings), SIZE)?;
            for (label, begin, end) in [("bass", 0, 8), ("mids", 8, 24), ("highs", 24, 48)] {
                // Equal scalar envelopes isolate the actual per-bin pathway.
                let mut band = quiet.clone();
                for bin in begin..end {
                    band.spectrum[bin] = 0.30 + (bin as f32 * 0.41).sin().powi(2) * 0.45;
                }
                let pixels = probe.cold(input(world, look, 18.0, &band, &mappings), SIZE)?;
                let response = difference(&baseline, &pixels);
                if response <= 0.015 {
                    failures.push(format!("{name}: {label} FFT route too quiet ({response})"));
                }
                routes.push(serde_json::json!({"look":look,"name":name,"band":label,"fft_rgb_difference":response}));
            }
            let disconnected_a = probe.cold(input(world, look, 18.0, &quiet, &zero), SIZE)?;
            let disconnected_b = probe.cold(input(world, look, 18.0, &music, &zero), SIZE)?;
            if difference(&disconnected_a, &disconnected_b) != 0.0 {
                failures.push(format!("{name}: zero mappings leak"));
            }
        }
        write_contact_sheet(
            &destination.join(format!("world-{world}-ten-looks.png")),
            &gallery,
            5,
        )?;
        let mut motion = Vec::new();
        for time in [0.0, 8.0, 18.0, 32.0, 60.0] {
            motion.push(probe.fixture(input(world, 0, time, &music, &mappings))?);
        }
        write_contact_sheet(
            &destination.join(format!("world-{world}-time-series.png")),
            &motion,
            5,
        )?;
        let mut morph = Vec::new();
        for amount in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let mut u = input(world, 9, 18.0, &music, &mappings);
            u.looks[world] = [0.0, 9.0, amount, 18.0];
            // Cold frames isolate actual topology rather than accumulated ink.
            morph.push(probe.cold(u, SIZE)?);
        }
        let first = probe.cold(input(world, 0, 18.0, &music, &mappings), SIZE)?;
        let last = probe.cold(input(world, 9, 18.0, &music, &mappings), SIZE)?;
        if difference(&morph[0], &first) != 0.0 || difference(&morph[4], &last) != 0.0 {
            failures.push(format!("{}: structure endpoint changed", SCENES[world]));
        }
        write_contact_sheet(
            &destination.join(format!("world-{world}-structure-morph.png")),
            &morph,
            5,
        )?;
        worlds.push(serde_json::json!({"world":world,"name":SCENES[world],"routes":routes}));
        println!(
            "Rendered all ten looks and separate FFT routes: {}",
            SCENES[world]
        );
    }

    let mut history_difference = None;
    let mut resets = Vec::new();
    let mut decay = Vec::new();
    if !silk_only {
        // Compare genuine ping-pong history with a cold render of the same endpoint.
        let neon = input(13, 0, 18.0, &music, &mappings);
        let warm = probe.fixture(neon)?;
        let cold = probe.cold(neon, SIZE)?;
        let measured_history_difference = difference(&warm, &cold);
        history_difference = Some(measured_history_difference);
        if measured_history_difference < 0.1 {
            failures.push("Neon has no visible retained history".into());
        }
        let mut no_echo = neon;
        no_echo.values[6][0] = 0.0;
        probe.fixture(neon)?;
        let echo_off_warm = probe.draw(no_echo, SIZE)?;
        let echo_off_cold = probe.cold(no_echo, SIZE)?;
        if difference(&echo_off_warm, &echo_off_cold) != 0.0 {
            failures.push("Echo0 retains history".into());
        }
        for reason in ["source", "mappings", "stale", "resume"] {
            probe.fixture(neon)?;
            let mut reset = neon;
            match reason {
                "source" => reset = reset.with_audio_state(true, 2),
                "mappings" => reset.values[4] = [0.0; 4],
                "stale" => reset = reset.with_audio_state(false, 1),
                _ => {
                    probe.draw(reset.with_audio_state(false, 1), SIZE)?;
                }
            }
            let actual = probe.draw(reset, SIZE)?;
            let expected = probe.cold(reset, SIZE)?;
            let delta = difference(&actual, &expected);
            if delta != 0.0 {
                failures.push(format!("Neon {reason} resurrects old ink ({delta})"));
            }
            resets.push(serde_json::json!({"reason":reason,"cold_reference_rgb_difference":delta}));
        }
        let mut decay_frames = vec![warm.clone()];
        for seconds in [1.0, 3.0, 8.0] {
            probe.fixture(neon)?;
            let quiet_input = input(13, 0, 18.0, &quiet, &mappings);
            probe.advance(quiet_input, (seconds * 60.0) as usize);
            let endpoint = input(13, 0, 18.0 + seconds, &quiet, &mappings);
            let retained = probe.draw(endpoint, SIZE)?;
            probe.cold(quiet_input, SIZE)?;
            probe.advance(quiet_input, (seconds * 60.0) as usize);
            let quiet_reference = probe.draw(endpoint, SIZE)?;
            decay.push(difference(&retained, &quiet_reference));
            decay_frames.push(retained);
        }
        if decay[2] >= decay[0] * 0.6 {
            failures.push(format!("Neon quiet ink does not decay: {decay:?}"));
        }
        write_contact_sheet(
            &destination.join("neon-history-decay.png"),
            &decay_frames,
            4,
        )?;
        write_contact_sheet(
            &destination.join("neon-real-history-vs-cold.png"),
            &[warm, cold],
            2,
        )?;
    }

    let mut transitions = Vec::new();
    for (from, to) in [(10, 11), (11, 12), (12, 13), (13, 9), (7, 12)] {
        if silk_only && from != 12 && to != 12 {
            continue;
        }
        let mut frames = Vec::new();
        for amount in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let mut u = input(to, 0, 18.0, &music, &mappings).with_mandelbrot_age(18.0);
            u.values[5][2] = from as f32;
            u.values[5][3] = amount;
            u.values[8][2] = 0.75;
            u.values[8][3] = 1.7;
            frames.push(probe.cold(u, SIZE)?);
        }
        write_contact_sheet(
            &destination.join(format!("world-morph-{from}-{to}.png")),
            &frames,
            5,
        )?;
        transitions.push([from, to]);
    }
    // Sample the actual depth-tested surface without a bright background
    // concealing a clipped edge. The geometry/shading pipeline stays identical.
    probe.isolate_silk();
    let mut fit = Vec::new();
    let maximum = AudioFeatures {
        bass: 1.5,
        mids: 1.5,
        treble: 1.5,
        beat: 1.5,
        spectrum: vec![1.5; 48],
        ..music.clone()
    };
    let maximum_mappings = AudioMappings {
        bass_to_pulse: 1.5,
        mids_to_geometry: 1.5,
        treble_to_sparkle: 1.5,
        beat_to_flash: 1.5,
    };
    for (layout, size, centre) in [
        ("wide", SIZE, 0.0),
        ("sidebar", SIZE, -0.40),
        ("portrait", [360, 640], 0.0),
    ] {
        for look in 0..10 {
            for time in [0.0, 18.0, 42.0] {
                let u =
                    input(12, look, time, &maximum, &maximum_mappings).with_canvas_centre(centre);
                let frame = probe.cold(u, size)?;
                let mut bounds = [size[0], size[1], 0, 0];
                let mut occupied = 0;
                for (index, pixel) in frame.chunks_exact(4).enumerate() {
                    if pixel[..3].iter().any(|channel| *channel > 3) {
                        let x = index as u32 % size[0];
                        let y = index as u32 / size[0];
                        bounds[0] = bounds[0].min(x);
                        bounds[1] = bounds[1].min(y);
                        bounds[2] = bounds[2].max(x);
                        bounds[3] = bounds[3].max(y);
                        occupied += 1;
                    }
                }
                // Immersive Silk intentionally continues beyond the canvas.
                // Edge contact is expected; reject empty/imperceptible output.
                if occupied < (size[0] * size[1]) / 50 {
                    failures.push(format!(
                        "Silk {look}/{layout}/{time}: too little visible surface {bounds:?}"
                    ));
                }
                if time == 18.0 {
                    write_png(
                        &destination.join(format!("silk-fit-{layout}-{look}.png")),
                        &frame,
                        size,
                    )?;
                }
                fit.push(serde_json::json!({"look":look,"layout":layout,"time":time,"bounds":bounds,"occupied_pixels":occupied}));
            }
        }
    }
    fs::write(
        &report_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "status":if failures.is_empty() {"pass"} else {"fail"},"adapter":info.name,
            "world_count":WORLD_COUNT,"silk_only":silk_only,"history_tested":!silk_only,"uniform_bytes":std::mem::size_of::<ShaderUniforms>(),
            "worlds":worlds,"history_difference":history_difference,"resets":resets,
        "quiet_decay_rgb_difference":decay,"world_morphs":transitions,"silk_fitting":fit,"failures":failures,
            "scope":"Production shader and mesh render, synthetic audio only; recorded worlds/looks, per-band FFT image response, zero mappings and bounded snapshots. When history_tested: exact echo/reset, real90-frame ping-pong history and quiet decay. Art/readability requires image review; not live audio, display FPS or perceptual fluidity proof."
        }))?,
    )?;
    anyhow::ensure!(
        failures.is_empty(),
        "new-world preview findings: {}",
        failures.join("; ")
    );
    println!("New-world production previews passed on {}", info.name);
    Ok(())
}
