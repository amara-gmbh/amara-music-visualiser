use crate::{
    choreography::Choreography,
    dsp::AudioFeatures,
    motion::motion_rate,
    presets::{AudioMappings, VisualSettings, WORLD_COUNT},
};
use eframe::{
    egui,
    egui_wgpu::{self, wgpu},
};
use std::time::Instant;
use wgpu::util::DeviceExt as _;

mod attractor_verification;
mod aurora_verification;
mod mandelbrot_verification;
mod new_world_verification;
mod pipelines;
mod raster;
mod verification;
mod water_verification;
pub use attractor_verification::preview_attractor;
pub use aurora_verification::preview_aurora;
pub use mandelbrot_verification::preview_mandelbrot;
pub use mandelbrot_verification::preview_mandelbrot_auto;
pub use new_world_verification::preview_aurora_response;
pub use new_world_verification::preview_disco_silk;
pub use new_world_verification::preview_music_response;
pub use new_world_verification::preview_neon_coast;
pub use new_world_verification::preview_new_worlds;
pub use new_world_verification::verify_glass;
pub use verification::preview_background;
pub use verification::preview_chromatic;
pub use verification::preview_magic;
pub use verification::preview_media;
pub use verification::verify_render;
pub use water_verification::preview_water;
mod benchmark;
pub use benchmark::benchmark_attractor;
pub use benchmark::benchmark_aurora;
pub use benchmark::benchmark_disco_silk;
pub use benchmark::benchmark_neon_coast;
pub use benchmark::benchmark_new_worlds;
pub use benchmark::benchmark_render;
pub use benchmark::benchmark_specialization;
pub use new_world_verification::verify_specialization;
pub const PREPARATION_COUNT: usize = pipelines::COUNT;

const WORLD_SHADER_SOURCE: &str = concat!(
    include_str!("params.wgsl"),
    "\n",
    include_str!("shader.wgsl"),
    "\n",
    include_str!("worlds/flight.wgsl"),
    "\n",
    include_str!("worlds/mathematics.wgsl"),
    "\n",
    include_str!("worlds/tide.wgsl"),
    "\n",
    include_str!("worlds/feedback.wgsl"),
    "\n",
    include_str!("worlds/chromatic_space.wgsl"),
    "\n",
    include_str!("worlds/water.wgsl"),
    "\n",
    include_str!("worlds/aurora.wgsl"),
    "\n",
    include_str!("worlds/mandelbrot.wgsl"),
    "\n",
    include_str!("worlds/liquid_stained_glass.wgsl"),
    "\n",
    include_str!("worlds/neon_memory.wgsl"),
    "\n",
    include_str!("worlds/neon_coast.wgsl"),
    "\n",
    include_str!("worlds/mocha_world.wgsl"),
    "\n",
    include_str!("worlds/aquarelle_world.wgsl"),
);

pub const SCENES: [&str; WORLD_COUNT] = [
    "Deep Flight",
    "Fractal Bloom",
    "Glass Tide",
    "Feedback Reverie",
    "Particle Constellation",
    "Strange Attractor",
    "Spectral Landscape",
    "Dot Equaliser",
    "Water Ripples",
    "Northern Lights",
    "Mandelbrot",
    "Liquid Stained Glass",
    "Disco Silk",
    "Neon Memory",
    "Neon Coast",
    "Mocca & Cream",
    "Aquarelle",
];
pub const LOOKS: [[&str; 10]; WORLD_COUNT] = [
    [
        "Harmonic Vaults",
        "Braided Currents",
        "Trefoil Gardens",
        "Helical Ribs",
        "Folded Canyon",
        "Cathedral Lace",
        "Orbital Petals",
        "Gravity Wells",
        "Crystal Wings",
        "Double Helix",
    ],
    [
        "Original",
        "Cathedral Lace",
        "Pearl Cells",
        "Tidal Ribbons",
        "Dragon Veins",
        "Obsidian Petals",
        "Helix Garden",
        "Crystal Estuary",
        "Woven Nebula",
        "Ember Labyrinth",
    ],
    [
        "Pearl Currents",
        "Folded Membrane",
        "Vortex Pool",
        "Moire Laminae",
        "Splitting Droplets",
        "Shell Fold",
        "Braided Currents",
        "Cellular Glass",
        "Prismatic Fan",
        "Ripple Tessellation",
    ],
    [
        "Liquid Engraving",
        "Chladni Resonance",
        "Spiral Seeds",
        "Orbital Interference",
        "Hyperbolic Weave",
        "Logarithmic Shells",
        "Gyroid Dream",
        "Cellular Rifts",
        "Diffraction Garden",
        "Hexagonal Harmonics",
    ],
    [
        "Luminous Sphere",
        "Breathing Shells",
        "Trefoil Orbit",
        "Cinquefoil Ribbon",
        "Lissajous Lattice",
        "Harmonic Ribbon",
        "Curl Swarm",
        "Twin Vortices",
        "Spiral Observatory",
        "Crystal Lanterns",
    ],
    [
        "Living Attractor",
        "Lorenz Wings",
        "Rossler Scroll",
        "Thomas Labyrinth",
        "Wing to Scroll",
        "Labyrinth Wings",
        "Scroll Garden",
        "Braided Chaos",
        "Unfurling Wings",
        "Labyrinth Scroll",
    ],
    [
        "Frequency Highlands",
        "Bent Ridges",
        "Copper Valley",
        "Rolling Spectrum",
        "Sound Dunes",
        "Wide Basin",
        "Spectral Shelf",
        "Frequency Canyon",
        "Harmonic Mesa",
        "Living Soundscape",
    ],
    [
        "Crystal Columns",
        "Velvet Steps",
        "Spectrum Arcade",
        "Twin Terraces",
        "Glass Organ",
        "Frequency Orchard",
        "Harmonic Fold",
        "Aurora Pillars",
        "Bass Cathedral",
        "Prism Skyline",
    ],
    [
        "Listening Pool",
        "Twin Springs",
        "Silk Canal",
        "Rain Glass",
        "Tidal Silk",
        "Prism Lagoon",
        "Cross Current",
        "Mercury Pool",
        "Longshore",
        "Crystal Rain",
    ],
    [
        "Emerald Veil",
        "Boreal Ribbons",
        "Violet Crown",
        "Polar Cascade",
        "Twin Horizons",
        "Solar Silk",
        "Glacial Wings",
        "Amber Storm",
        "Midnight Spiral",
        "Prismatic Dawn",
    ],
    [
        "Seahorse Valley",
        "Cardioid Voyage",
        "Elephant Valley",
        "Triple Spiral",
        "Satellite Garden",
        "Dendrite Disco",
        "Needle Galaxy",
        "Coral Cathedral",
        "Sapphire Filaments",
        "Satellite Spiral",
    ],
    [
        "Prismatic Passage",
        "Honey Current",
        "Molten Petals",
        "Ribbon Prism",
        "Deep Mosaic",
        "Copper Lagoon",
        "Crystal Channels",
        "Velvet Lenses",
        "Broken Spectrum",
        "Opal Drift",
    ],
    [
        "Helix Current",
        "Braided Passage",
        "Turning Sails",
        "Silk Canyon",
        "Petal Vault",
        "Twin Currents",
        "Folded Canopy",
        "Infinity Weave",
        "Opening Bloom",
        "Spiral Loom",
    ],
    [
        "Afterglow River",
        "Velvet Infinity",
        "Split Current",
        "Electric Weave",
        "Magnetic Script",
        "Neon Cadence",
        "Helix Parade",
        "Spectral Delta",
        "Fresnel Garden",
        "Prismatic Fugue",
    ],
    [
        "Sunset Boulevard",
        "Ocean Drive",
        "Palm Causeway",
        "Midnight Marina",
        "Art Deco Rush",
        "Violet Skyline",
        "Lagoon Flight",
        "Chrome Rain",
        "Golden Keys",
        "Neon Afterhours",
    ],
    [
        "Crema Current",
        "Espresso Ribbons",
        "Vanilla Drift",
        "Copper Creases",
        "Crema Canopy",
        "Mocha Marble",
        "Silk Espresso",
        "Roasted Whorls",
        "Pearl Weave",
        "Honeyed Passage",
    ],
    [
        "Chromatic Wash",
        "Rose Tides",
        "Lagoon Petals",
        "Neon Ink",
        "Sunset Silk",
        "Floating Gardens",
        "Disco Marbling",
        "Misty Irises",
        "Electric Saffron",
        "Prismatic Rain",
    ],
];
const HISTORY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Water-only reduction of the already smoothed audio snapshot. Every FFT row
/// participates, including the lower mids missed by the old point samples.
/// Compute this once per frame, never repeat the reduction for every pixel.
fn water_frequency_bands(audio: &AudioFeatures, mappings: &AudioMappings) -> [[f32; 4]; 2] {
    let unit = |value: f32| {
        if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    let groups = [
        (0, 8),
        (8, 13),
        (13, 18),
        (18, 24),
        (24, 30),
        (30, 36),
        (36, 42),
        (42, 48),
    ];
    let bands: [f32; 8] = std::array::from_fn(|index| {
        let (start, end) = groups[index];
        let mut squares = 0.0_f32;
        let mut peak = 0.0_f32;
        for row in start..end {
            let value = unit(audio.spectrum.get(row).copied().unwrap_or(0.0));
            squares += value * value;
            peak = peak.max(value);
        }
        let spectral = 0.65 * (squares / (end - start) as f32).sqrt() + 0.35 * peak;
        // FFT display partitions are not exact acoustic band boundaries.
        // These independently measured envelopes also carry lower vocals and
        // 2.4–4.5kHz cymbals. This is measured support, NOT a nonzero floor.
        let (envelope, mapping) = match index {
            0 => (audio.bass, mappings.bass_to_pulse),
            1..=3 => (audio.mids, mappings.mids_to_geometry),
            _ => (audio.treble, mappings.treble_to_sparkle),
        };
        let support = unit(envelope) * 0.35;
        let gain = if mapping.is_finite() {
            mapping.clamp(0.0, 1.5)
        } else {
            0.0
        };
        (spectral * spectral + support * support).sqrt().min(1.0) * gain
    });
    [
        bands[..4].try_into().unwrap(),
        bands[4..].try_into().unwrap(),
    ]
}

/// Aligned vec4s shared with shader.wgsl, including 48 measured FFT bands.
/// Spectrum is mapped here, before upload, so a zero mapping disconnects it.
/// Audio values are snapshots;
/// neither inference nor audio processing is permitted in a render callback.
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct ShaderUniforms {
    values: [[f32; 4]; 10],
    spectrum: [[f32; 4]; 12],
    looks: [[f32; 4]; WORLD_COUNT],
    raster: [f32; 4],
    voice: [f32; 4],
    music: [f32; 4],
    ripples: [[f32; 4]; crate::ripples::RIPPLE_COUNT],
    background: [f32; 4],
    water_bands: [[f32; 4]; 2],
    material: [f32; 4],
}

impl ShaderUniforms {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        time: f32,
        dt: f32,
        scene_to: usize,
        scene_from: usize,
        transition: f32,
        visual: &VisualSettings,
        mappings: &AudioMappings,
        audio: &AudioFeatures,
    ) -> Self {
        Self {
            looks: [[0.0, 0.0, 1.0, 0.0]; WORLD_COUNT],
            raster: [0.0, 0.0, 1.0, 0.0],
            voice: [0.0, 1.0, 0.0, 0.0],
            music: [0.0; 4],
            ripples: [[0.0; 4]; crate::ripples::RIPPLE_COUNT],
            background: [0.0; 4],
            water_bands: water_frequency_bands(audio, mappings),
            material: [0.0; 4],
            spectrum: std::array::from_fn(|group| {
                std::array::from_fn(|lane| {
                    let band = group * 4 + lane;
                    let mapping = match band {
                        0..=7 => mappings.bass_to_pulse,
                        8..=23 => mappings.mids_to_geometry,
                        _ => mappings.treble_to_sparkle,
                    };
                    audio
                        .spectrum
                        .get(band)
                        .copied()
                        .unwrap_or(0.0)
                        .clamp(0.0, 1.0)
                        * mapping
                })
            }),
            values: [
                [0.0, 0.0, time, scene_to as f32],
                [audio.bass, audio.mids, audio.treble, audio.energy],
                [
                    audio.beat,
                    visual.intensity,
                    motion_rate(visual.motion),
                    visual.complexity,
                ],
                [
                    visual.hue,
                    visual.glow,
                    visual.warp,
                    visual.palette.amount(),
                ],
                [
                    mappings.bass_to_pulse,
                    mappings.mids_to_geometry,
                    mappings.treble_to_sparkle,
                    mappings.beat_to_flash,
                ],
                [
                    audio.dna.brightness,
                    audio.dna.danceability,
                    scene_from as f32,
                    transition,
                ],
                [visual.feedback, dt, 0.0, 0.0],
                [0.35, 0.75, 0.2, 0.0],
                [time, 1.0, 0.0, 0.0],
                [0.0; 4],
            ],
        }
    }

    pub fn with_choreography(mut self, phrase: Choreography) -> Self {
        self.values[7] = [phrase.drive, phrase.openness, phrase.lift, phrase.accent];
        self.values[8] = [phrase.phase, phrase.travel_speed, 0.0, 0.0];
        self
    }

    pub fn with_palette_mix(mut self, amount: f32) -> Self {
        self.values[3][3] = if amount.is_finite() {
            amount.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self
    }

    pub fn with_math_blend(mut self, from: usize, to: usize, blend: f32, phase: f32) -> Self {
        self.values[9] = [
            from.min(3) as f32,
            to.min(3) as f32,
            if blend.is_finite() {
                blend.clamp(0.0, 1.0)
            } else {
                0.0
            },
            if phase.is_finite() { phase } else { 0.0 },
        ];
        self
    }

    pub fn with_canvas_centre(mut self, offset: f32) -> Self {
        self.values[6][3] = if offset.is_finite() {
            offset.clamp(-2.0, 2.0)
        } else {
            0.0
        };
        self
    }

    pub fn with_auto_mix(mut self, amount: f32, seed: f32) -> Self {
        self.values[8][2] = if amount.is_finite() {
            amount.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.values[8][3] = if seed.is_finite() { seed } else { 0.0 };
        self
    }

    pub fn with_vocal_response(mut self, voice: [f32; 4]) -> Self {
        self.voice = if self.values[4][1] <= 0.0 || voice.iter().any(|value| !value.is_finite()) {
            [0.0, 1.0, 0.0, 0.0]
        } else {
            [
                voice[0].clamp(0.0, 1.5),
                voice[1].clamp(0.0, 1.0),
                voice[2].clamp(0.0, 1.5),
                voice[3],
            ]
        };
        self
    }

    pub fn with_looks(mut self, looks: [[f32; 4]; WORLD_COUNT]) -> Self {
        self.looks = looks.map(|look| {
            if look.iter().any(|value| !value.is_finite()) {
                return [0.0, 0.0, 1.0, 0.0];
            }
            [
                look[0].round().clamp(0.0, 9.0),
                look[1].round().clamp(0.0, 9.0),
                look[2].clamp(0.0, 1.0),
                look[3],
            ]
        });
        self
    }

    pub fn with_music_response(mut self, music: [f32; 4]) -> Self {
        let gates = [
            self.values[4][3],
            self.values[4][3],
            self.values[4][0],
            self.values[4][2],
        ];
        self.music = std::array::from_fn(|index| {
            if gates[index] <= 0.0 || !music[index].is_finite() {
                0.0
            } else {
                music[index].clamp(0.0, 1.0)
            }
        });
        self
    }

    pub fn with_audio_state(mut self, fresh: bool, epoch: u64) -> Self {
        self.raster[2] = if fresh { 1.0 } else { 0.0 };
        self.raster[3] = (epoch % 1_000_000) as f32;
        if !fresh {
            self.voice = [0.0, 1.0, 0.0, 0.0];
            self.music = [0.0; 4];
            self.ripples = [[0.0; 4]; crate::ripples::RIPPLE_COUNT];
        }
        self
    }

    pub fn with_background(mut self, background: [f32; 4]) -> Self {
        self.background = background.map(|value| {
            if value.is_finite() {
                value.clamp(0.0, 1.0)
            } else {
                0.0
            }
        });
        self
    }

    pub fn with_material_hue(mut self, offset: f32) -> Self {
        self.material[0] = if offset.is_finite() {
            offset.rem_euclid(1.0)
        } else {
            0.0
        };
        self
    }

    pub fn with_mandelbrot_age(mut self, seconds: f32) -> Self {
        self.material[1] = if seconds.is_finite() {
            seconds.clamp(0.0, 120.0)
        } else {
            0.0
        };
        self.material[2] = 1.0;
        self
    }

    pub fn with_mandelbrot_motion(mut self, amount: f32) -> Self {
        self.material[3] = if amount.is_finite() {
            amount.clamp(0.0, 1.0)
        } else {
            0.0
        };
        self
    }

    pub fn with_ripples(mut self, ripples: [[f32; 4]; crate::ripples::RIPPLE_COUNT]) -> Self {
        self.ripples = ripples.map(|event| {
            if event.iter().any(|v| !v.is_finite()) {
                return [0.0; 4];
            }
            [
                event[0].clamp(-2.0, 2.0),
                event[1].clamp(-2.0, 2.0),
                event[2].clamp(0.0, crate::ripples::EVENT_MAX_AGE),
                event[3].clamp(0.0, 1.5),
            ]
        });
        self
    }
}

pub fn install(
    cc: &eframe::CreationContext<'_>,
    initial_scene: usize,
) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
    let state = cc.wgpu_render_state.as_ref().expect("wgpu render state");
    let mut resources = VisualResources::new(&state.device, state.target_format);
    let progress = resources.prepare_async(&state.device, initial_scene);
    state.renderer.write().callback_resources.insert(resources);
    progress
}

pub struct CanvasBackdrop {
    pub rect: egui::Rect,
    slot: egui::layers::ShapeIdx,
}

/// Reserve the background before controls paint, so translucent panels really
/// reveal the art. The shader centre still follows the unobstructed canvas.
pub fn reserve_canvas(ui: &egui::Ui) -> CanvasBackdrop {
    CanvasBackdrop {
        rect: ui.available_rect_before_wrap(),
        slot: ui.painter().add(egui::Shape::Noop),
    }
}

pub fn paint_canvas(
    ui: &mut egui::Ui,
    uniforms: ShaderUniforms,
    sharp: bool,
    backdrop: CanvasBackdrop,
    pending_world: Option<usize>,
) {
    ui.painter().set(
        backdrop.slot,
        egui_wgpu::Callback::new_paint_callback(
            backdrop.rect,
            VisualCallback {
                uniforms,
                logical_size: backdrop.rect.size(),
                sharp,
                pending_mask: pending_world.map(|world| {
                    pipelines::world_mask(uniforms.values[0][3] as usize)
                        | pipelines::world_mask(world)
                }),
            },
        ),
    );
    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, egui::Sense::hover());
}

struct VisualCallback {
    uniforms: ShaderUniforms,
    logical_size: egui::Vec2,
    sharp: bool,
    pending_mask: Option<u32>,
}

impl egui_wgpu::CallbackTrait for VisualCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let resources: &mut VisualResources = resources.get_mut().expect("visual resources");
        if resources.requested_hint != self.pending_mask {
            if let (Some(mask), Some(preparation)) = (self.pending_mask, &resources.preparation) {
                let _ = preparation.requests.try_send(mask);
            }
            resources.requested_hint = self.pending_mask;
        }
        if let Some(cadence) = &mut resources.cadence {
            cadence.record();
        }
        let desired = self.logical_size * screen.pixels_per_point;
        // A bounded offscreen canvas avoids quadratic Retina cost while keeping
        // the native controls crisp. Presentation is filtered at native size.
        let limit = if self.sharp {
            [1600.0, 1000.0]
        } else {
            [1280.0, 800.0]
        };
        let scale = (limit[0] / desired.x.max(1.0))
            .min(limit[1] / desired.y.max(1.0))
            .min(1.0);
        let size = [
            (desired.x * scale).round().max(1.0) as u32,
            (desired.y * scale).round().max(1.0) as u32,
        ];
        resources.render(device, queue, encoder, self.uniforms, size);
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let resources: &VisualResources = resources.get().expect("visual resources");
        if let Some(history) = &resources.history {
            render_pass.set_pipeline(&resources.present_pipeline);
            render_pass.set_bind_group(0, &history.bind_groups[history.current], &[]);
            render_pass.draw(0..3, 0..1);
        }
    }
}

struct VisualResources {
    scene_pipeline: wgpu::RenderPipeline,
    prepared_pipelines: Vec<(u32, wgpu::RenderPipeline)>,
    preparation: Option<pipelines::Preparation>,
    requested_mask: Option<u32>,
    requested_hint: Option<u32>,
    present_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    uniform_buffer: wgpu::Buffer,
    sampler: wgpu::Sampler,
    history: Option<History>,
    cadence: Option<RenderCadence>,
    raster: raster::RasterWorlds,
}

struct RenderCadence {
    last: Option<Instant>,
    intervals: Vec<f32>,
}

impl RenderCadence {
    fn record(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last.replace(now) {
            self.intervals
                .push(now.saturating_duration_since(last).as_secs_f32());
        }
        if self.intervals.len() == 300 {
            let mean = self.intervals.iter().sum::<f32>() / 300.0;
            self.intervals.sort_by(f32::total_cmp);
            log::info!(
                "GPU canvas prepare callbacks: {:.1}/s, mean {:.2} ms, p95 {:.2} ms (300 callbacks; not a display scanout measurement)",
                1.0 / mean.max(0.000001),
                mean * 1000.0,
                self.intervals[285] * 1000.0
            );
            self.intervals.clear();
        }
    }
}

struct History {
    size: [u32; 2],
    _textures: [wgpu::Texture; 2],
    views: [wgpu::TextureView; 2],
    bind_groups: [wgpu::BindGroup; 2],
    current: usize,
    initialized: bool,
    centre: f32,
    input: HistoryInput,
}

/// Image trails belong to one live input and one mapping configuration.
/// Invalidating on both edges of stale input prevents its last rendered frame
/// from returning when a source resumes, even if no source epoch changed.
#[derive(Default)]
struct HistoryInput {
    epoch: Option<f32>,
    mappings: Option<[f32; 4]>,
    fresh: bool,
}

impl HistoryInput {
    fn update(&mut self, uniforms: &ShaderUniforms) -> bool {
        let fresh = uniforms.raster[2] > 0.0;
        let invalid = !fresh
            || !self.fresh
            || self.epoch != Some(uniforms.raster[3])
            || self.mappings != Some(uniforms.values[4]);
        self.epoch = Some(uniforms.raster[3]);
        self.mappings = Some(uniforms.values[4]);
        self.fresh = fresh;
        invalid
    }
}

impl VisualResources {
    fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("amara procedural worlds + feedback"),
            source: wgpu::ShaderSource::Wgsl(WORLD_SHADER_SOURCE.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("world uniforms and previous frame"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("amara world layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = |label, entry, format: wgpu::TextureFormat| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    targets: &[Some(format.into())],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let scene_pipeline = pipeline("world + feedback pass", "fs_main", HISTORY_FORMAT);
        let present_pipeline = pipeline("native presentation pass", "fs_present", target_format);
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("amara uniforms"),
            contents: bytemuck::bytes_of(&ShaderUniforms {
                values: [[0.0; 4]; 10],
                spectrum: [[0.0; 4]; 12],
                looks: [[0.0, 0.0, 1.0, 0.0]; WORLD_COUNT],
                raster: [0.0; 4],
                voice: [0.0, 1.0, 0.0, 0.0],
                music: [0.0; 4],
                ripples: [[0.0; 4]; crate::ripples::RIPPLE_COUNT],
                background: [0.0; 4],
                water_bands: [[0.0; 4]; 2],
                material: [0.0; 4],
            }),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("feedback sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let raster = raster::RasterWorlds::new(device, &uniform_buffer);
        Self {
            scene_pipeline,
            prepared_pipelines: Vec::new(),
            preparation: None,
            requested_mask: None,
            requested_hint: None,
            present_pipeline,
            bind_group_layout,
            uniform_buffer,
            sampler,
            history: None,
            raster,
            cadence: std::env::var_os("AMARA_FRAME_STATS").map(|_| RenderCadence {
                last: None,
                intervals: Vec::with_capacity(300),
            }),
        }
    }

    fn preparation_layout(&self, device: &wgpu::Device) -> wgpu::PipelineLayout {
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("prepared world layout"),
            bind_group_layouts: &[Some(&self.bind_group_layout)],
            immediate_size: 0,
        })
    }

    /// Diagnostic prebuild, outside any measured frame interval.
    fn specialize(&mut self, device: &wgpu::Device, masks: &[u32]) {
        let layout = self.preparation_layout(device);
        self.prepared_pipelines = masks
            .iter()
            .copied()
            .map(|mask| (mask, pipelines::build(device, &layout, mask)))
            .collect();
    }

    fn prepare_async(
        &mut self,
        device: &wgpu::Device,
        initial_scene: usize,
    ) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
        match pipelines::Preparation::start(
            device.clone(),
            self.preparation_layout(device),
            pipelines::world_mask(initial_scene),
        ) {
            Ok(preparation) => {
                let progress = std::sync::Arc::clone(&preparation.completed);
                self.preparation = Some(preparation);
                progress
            }
            Err(error) => {
                log::warn!("World preparation could not start; using general renderer: {error}");
                std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(pipelines::COUNT))
            }
        }
    }

    fn receive_prepared(&mut self, mask: u32) {
        if let Some(preparation) = &self.preparation {
            for (key, result) in preparation.ready.try_iter() {
                match result {
                    Ok(pipeline) => self.prepared_pipelines.push((key, pipeline)),
                    Err(error) => log::warn!(
                        "World preparation {key} failed; using general renderer: {error}"
                    ),
                }
            }
            if self.requested_mask != Some(mask) && preparation.requests.try_send(mask).is_ok() {
                self.requested_mask = Some(mask);
            }
        }
    }

    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        mut uniforms: ShaderUniforms,
        size: [u32; 2],
    ) {
        let mask = pipelines::frame_mask(
            uniforms.values[5][2],
            uniforms.values[0][3],
            uniforms.values[5][3],
        );
        self.receive_prepared(mask);
        self.raster.ensure_size(device, size);
        if self
            .history
            .as_ref()
            .is_none_or(|history| history.size != size)
        {
            self.history = Some(History::new(
                device,
                size,
                &self.bind_group_layout,
                &self.uniform_buffer,
                &self.sampler,
                self.raster.view(),
            ));
        }
        let history = self.history.as_mut().expect("history created");
        if history.input.update(&uniforms) {
            history.initialized = false;
        }
        if (history.centre - uniforms.values[6][3]).abs() > 0.00001 {
            history.initialized = false;
            history.centre = uniforms.values[6][3];
        }
        uniforms.values[0][0] = size[0] as f32;
        uniforms.values[0][1] = size[1] as f32;
        uniforms.values[6][2] = if history.initialized { 1.0 } else { 0.0 };
        self.raster.prepare(queue, &mut uniforms);
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));
        self.raster.render(encoder, &uniforms);
        let next = 1 - history.current;
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("amara procedural render"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &history.views[next],
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            let pipeline = self
                .prepared_pipelines
                .iter()
                .find(|(key, _)| *key == mask)
                .map(|(_, pipeline)| pipeline)
                .unwrap_or(&self.scene_pipeline);
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &history.bind_groups[history.current], &[]);
            pass.draw(0..3, 0..1);
        }
        history.current = next;
        history.initialized = true;
    }
}

impl History {
    fn new(
        device: &wgpu::Device,
        size: [u32; 2],
        layout: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
        raster: &wgpu::TextureView,
    ) -> Self {
        let textures = std::array::from_fn(|_| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("amara feedback history"),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HISTORY_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        });
        let views = textures
            .each_ref()
            .map(|texture| texture.create_view(&Default::default()));
        let bind_groups = views.each_ref().map(|view| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("previous frame bindings"),
                layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(raster),
                    },
                ],
            })
        });
        Self {
            size,
            _textures: textures,
            views,
            bind_groups,
            current: 0,
            initialized: false,
            centre: 0.0,
            input: HistoryInput::default(),
        }
    }
}

#[cfg(test)]
mod uniform_tests {
    use super::*;

    #[test]
    fn image_history_invalidates_input_changes_and_both_stale_edges() {
        let mut input = HistoryInput::default();
        let frame = ShaderUniforms::new(
            1.0,
            0.016,
            13,
            13,
            1.0,
            &VisualSettings::default(),
            &AudioMappings::default(),
            &AudioFeatures::default(),
        )
        .with_audio_state(true, 7);
        assert!(input.update(&frame));
        assert!(!input.update(&frame));
        let stale = frame.with_audio_state(false, 7);
        assert!(input.update(&stale));
        assert!(input.update(&stale));
        assert!(input.update(&frame));
        assert!(!input.update(&frame));
        let changed = frame.with_audio_state(true, 8);
        assert!(input.update(&changed));
        assert!(!input.update(&changed));
        let mut disconnected = changed;
        disconnected.values[4] = [0.0; 4];
        assert!(input.update(&disconnected));
        assert!(!input.update(&disconnected));
        assert!(input.update(&changed));
    }

    fn validate_shader(source: &str) {
        let module = wgpu::naga::front::wgsl::parse_str(source)
            .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap_or_else(|error| panic!("{}", error.emit_to_string(source)));
    }

    #[test]
    fn actual_world_shader_parses_and_validates_without_a_gpu() {
        validate_shader(WORLD_SHADER_SOURCE);
    }

    #[test]
    fn actual_geometry_shader_parses_and_validates_without_a_gpu() {
        validate_shader(raster::SHADER_SOURCE);
    }

    #[test]
    fn uniform_layout_matches_worlds_impacts_tails_and_colour_journeys() {
        assert_eq!(std::mem::size_of::<ShaderUniforms>(), 1120);
        let module = wgpu::naga::front::wgsl::parse_str(include_str!("params.wgsl")).unwrap();
        let params = module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some("Params"))
            .unwrap()
            .1;
        let wgpu::naga::TypeInner::Struct { members, span } = &params.inner else {
            panic!("Params must remain a struct")
        };
        assert_eq!(*span as usize, std::mem::size_of::<ShaderUniforms>());
        for (name, offset) in [
            ("looks", std::mem::offset_of!(ShaderUniforms, looks)),
            ("raster", std::mem::offset_of!(ShaderUniforms, raster)),
            ("voice", std::mem::offset_of!(ShaderUniforms, voice)),
            ("music", std::mem::offset_of!(ShaderUniforms, music)),
            ("ripples", std::mem::offset_of!(ShaderUniforms, ripples)),
            (
                "background",
                std::mem::offset_of!(ShaderUniforms, background),
            ),
            (
                "water_bands",
                std::mem::offset_of!(ShaderUniforms, water_bands),
            ),
            ("material", std::mem::offset_of!(ShaderUniforms, material)),
        ] {
            assert_eq!(
                members
                    .iter()
                    .find(|m| m.name.as_deref() == Some(name))
                    .unwrap()
                    .offset as usize,
                offset,
                "GPU/CPU offset for{name}"
            );
        }
        let uniforms = ShaderUniforms::new(
            1.0,
            0.016,
            1,
            1,
            1.0,
            &VisualSettings::default(),
            &AudioMappings::default(),
            &AudioFeatures::default(),
        );
        assert_eq!(uniforms.values[9], [0.0; 4]);
        assert_eq!(uniforms.spectrum.len(), 12);
        assert_eq!(uniforms.looks.len(), WORLD_COUNT);
        assert_eq!(uniforms.voice, [0.0, 1.0, 0.0, 0.0]);
        assert_eq!(uniforms.music, [0.0; 4]);
        assert_eq!(uniforms.water_bands, [[0.0; 4]; 2]);
        assert_eq!(uniforms.material, [0.0; 4]);
        assert_eq!(std::mem::offset_of!(ShaderUniforms, material), 1104);
        let uniforms = uniforms
            .with_math_blend(99, 2, f32::NAN, f32::INFINITY)
            .with_canvas_centre(f32::NAN);
        assert_eq!(uniforms.values[9], [3.0, 2.0, 0.0, 0.0]);
        assert_eq!(uniforms.values[6][3], 0.0);
    }

    #[test]
    fn background_uniform_is_bounded_and_does_not_modify_music_or_geometry() {
        let original = ShaderUniforms::new(
            1.0,
            0.016,
            8,
            8,
            1.0,
            &VisualSettings::default(),
            &AudioMappings::default(),
            &AudioFeatures::default(),
        );
        let changed = original.with_background([f32::NAN, -1.0, 3.0, 0.46]);
        assert_eq!(changed.background, [0.0, 0.0, 1.0, 0.46]);
        assert_eq!(changed.values, original.values);
        assert_eq!(changed.spectrum, original.spectrum);
        assert_eq!(changed.ripples, original.ripples);
        assert_eq!(changed.water_bands, original.water_bands);
    }

    #[test]
    fn mandelbrot_age_and_material_hue_preserve_each_other_in_either_order() {
        let original = ShaderUniforms::new(
            900.0,
            0.016,
            10,
            10,
            1.0,
            &VisualSettings::default(),
            &AudioMappings::default(),
            &AudioFeatures::default(),
        );
        let first = original.with_mandelbrot_age(3.5).with_material_hue(0.75);
        let second = original.with_material_hue(0.75).with_mandelbrot_age(3.5);
        assert_eq!(first.material, [0.75, 3.5, 1.0, 0.0]);
        assert_eq!(first.material, second.material);
        assert_eq!(first.values, original.values);
        assert_eq!(first.spectrum, original.spectrum);
        assert_eq!(first.looks, original.looks);
        assert_eq!(
            first.with_mandelbrot_age(f32::NAN).material,
            [0.75, 0.0, 1.0, 0.0]
        );
        assert_eq!(first.with_mandelbrot_age(-4.0).material[1], 0.0);
        assert_eq!(first.with_mandelbrot_age(900.0).material[1], 120.0);
        let drifting = original
            .with_mandelbrot_motion(0.6)
            .with_mandelbrot_age(3.5)
            .with_material_hue(0.75);
        assert_eq!(drifting.material, [0.75, 3.5, 1.0, 0.6]);
        assert_eq!(drifting.with_mandelbrot_motion(f32::NAN).material[3], 0.0);
    }

    #[test]
    fn material_hue_is_independent_of_manual_hue_background_and_audio() {
        let original = ShaderUniforms::new(
            1.0,
            0.016,
            7,
            7,
            1.0,
            &VisualSettings::default(),
            &AudioMappings::default(),
            &AudioFeatures::default(),
        )
        .with_background([0.12, 0.07, 0.3, 0.46]);
        let changed = original.with_material_hue(-0.25);
        assert_eq!(changed.material, [0.75, 0.0, 0.0, 0.0]);
        assert_eq!(changed.values, original.values);
        assert_eq!(changed.background, original.background);
        assert_eq!(changed.spectrum, original.spectrum);
        assert_eq!(changed.looks, original.looks);
        assert_eq!(changed.water_bands, original.water_bands);
        assert_eq!(changed.ripples, original.ripples);
        assert_eq!(original.with_material_hue(f32::NAN).material, [0.0; 4]);
    }

    #[test]
    fn water_reduction_covers_each_fft_row_without_leaking_between_mappings() {
        let all = AudioMappings {
            bass_to_pulse: 1.0,
            mids_to_geometry: 1.0,
            treble_to_sparkle: 1.0,
            beat_to_flash: 0.0,
        };
        for row in 0..48 {
            let mut audio = AudioFeatures::default();
            audio.spectrum[row] = 0.05;
            let bands = water_frequency_bands(&audio, &all);
            assert_eq!(
                bands.iter().flatten().filter(|v| **v > 0.0).count(),
                1,
                "row{row}"
            );
            let mut muted = all.clone();
            match row {
                0..=7 => muted.bass_to_pulse = 0.0,
                8..=23 => muted.mids_to_geometry = 0.0,
                _ => muted.treble_to_sparkle = 0.0,
            }
            assert_eq!(water_frequency_bands(&audio, &muted), [[0.0; 4]; 2]);
        }
    }

    #[test]
    fn water_reduction_is_finite_quiet_and_applies_user_gain_once() {
        let mut audio = AudioFeatures::default();
        let mut mappings = AudioMappings::default();
        assert_eq!(water_frequency_bands(&audio, &mappings), [[0.0; 4]; 2]);
        audio.mids = 0.1;
        audio.treble = 0.08;
        audio.spectrum.fill(0.07);
        mappings.mids_to_geometry = 0.5;
        let half = water_frequency_bands(&audio, &mappings);
        mappings.mids_to_geometry = 1.0;
        let whole = water_frequency_bands(&audio, &mappings);
        for lane in 1..4 {
            assert!((half[0][lane] * 2.0 - whole[0][lane]).abs() < 1e-7);
        }
        assert_eq!(half[1], whole[1]);
        audio.spectrum.fill(f32::NAN);
        audio.mids = f32::INFINITY;
        audio.treble = -1.0;
        mappings.bass_to_pulse = f32::NAN;
        assert_eq!(water_frequency_bands(&audio, &mappings), [[0.0; 4]; 2]);
    }

    #[test]
    fn water_acoustic_envelopes_support_upper_bands_without_creating_bass() {
        let mut audio = AudioFeatures::default();
        let mut mappings = AudioMappings::default();
        audio.mids = 0.1;
        audio.treble = 0.1;
        let bands = water_frequency_bands(&audio, &mappings);
        assert_eq!(bands[0][0], 0.0);
        assert!(bands[0][1..].iter().all(|value| *value > 0.0));
        assert!(bands[1].iter().all(|value| *value > 0.0));
        mappings.mids_to_geometry = 0.0;
        mappings.treble_to_sparkle = 0.0;
        assert_eq!(water_frequency_bands(&audio, &mappings), [[0.0; 4]; 2]);
    }

    #[test]
    fn delayed_water_packets_keep_travelling_through_their_terminal_fade_on_gpu() {
        let mut events = [[0.0; 4]; crate::ripples::RIPPLE_COUNT];
        events[0] = [0.0, 0.0, 6.06, 0.4];
        events[1] = [0.0, 0.0, 7.0, 0.4];
        let uniforms = ShaderUniforms::new(
            1.0,
            0.016,
            8,
            8,
            1.0,
            &VisualSettings::default(),
            &AudioMappings::default(),
            &AudioFeatures::default(),
        )
        .with_ripples(events);
        assert_eq!(uniforms.ripples[0][2], 6.06);
        assert_eq!(uniforms.ripples[1][2], crate::ripples::EVENT_MAX_AGE);
    }

    #[test]
    fn dot_equaliser_appends_without_renumbering_existing_worlds() {
        assert_eq!(
            &SCENES[..7],
            &[
                "Deep Flight",
                "Fractal Bloom",
                "Glass Tide",
                "Feedback Reverie",
                "Particle Constellation",
                "Strange Attractor",
                "Spectral Landscape",
            ]
        );
        assert_eq!(SCENES[7], "Dot Equaliser");
        assert_eq!(SCENES[8], "Water Ripples");
        assert_eq!(SCENES[9], "Northern Lights");
        assert_eq!(SCENES[10], "Mandelbrot");
        assert_eq!(
            &SCENES[11..],
            &[
                "Liquid Stained Glass",
                "Disco Silk",
                "Neon Memory",
                "Neon Coast",
                "Mocca & Cream",
                "Aquarelle"
            ]
        );
        assert_eq!(LOOKS.len(), WORLD_COUNT);
        assert_eq!(LOOKS[7].len(), crate::look_journey::LOOK_COUNT);
        assert!(LOOKS[7].iter().all(|name| !name.is_empty()));
        for (index, name) in LOOKS[7].iter().enumerate() {
            assert!(!LOOKS[7][index + 1..].contains(name));
        }
    }

    #[test]
    fn vocal_uniform_is_shared_by_all_worlds_and_obeys_disconnect_and_freshness() {
        for scene in 0..WORLD_COUNT {
            let mut mappings = AudioMappings::default();
            let make = |mappings: &AudioMappings| {
                ShaderUniforms::new(
                    0.0,
                    0.016,
                    scene,
                    scene,
                    1.0,
                    &VisualSettings::default(),
                    mappings,
                    &AudioFeatures::default(),
                )
            };
            let active = [0.5, 0.4, 0.6, 1.0];
            assert_eq!(make(&mappings).with_vocal_response(active).voice, active);
            assert_eq!(
                make(&mappings)
                    .with_vocal_response(active)
                    .with_audio_state(false, 1)
                    .voice,
                [0.0, 1.0, 0.0, 0.0]
            );
            assert_eq!(
                make(&mappings).with_vocal_response([f32::NAN; 4]).voice,
                [0.0, 1.0, 0.0, 0.0]
            );
            mappings.mids_to_geometry = 0.0;
            assert_eq!(
                make(&mappings).with_vocal_response(active).voice,
                [0.0, 1.0, 0.0, 0.0]
            );
        }
        let module = wgpu::naga::front::wgsl::parse_str(WORLD_SHADER_SOURCE).unwrap();
        let params = module
            .types
            .iter()
            .find(|(_, ty)| ty.name.as_deref() == Some("Params"))
            .unwrap()
            .1;
        if let wgpu::naga::TypeInner::Struct { members, span } = &params.inner {
            assert_eq!(*span as usize, std::mem::size_of::<ShaderUniforms>());
            let voice = members
                .iter()
                .find(|member| member.name.as_deref() == Some("voice"))
                .unwrap();
            assert_eq!(
                voice.offset as usize,
                std::mem::offset_of!(ShaderUniforms, voice)
            );
            let music = members
                .iter()
                .find(|member| member.name.as_deref() == Some("music"))
                .unwrap();
            assert_eq!(
                music.offset as usize,
                std::mem::offset_of!(ShaderUniforms, music)
            );
            let material = members
                .iter()
                .find(|member| member.name.as_deref() == Some("material"))
                .unwrap();
            assert_eq!(
                material.offset as usize,
                std::mem::offset_of!(ShaderUniforms, material)
            );
        } else {
            panic!("Params must be the shared uniform structure");
        }
    }

    #[test]
    fn music_uniform_reaches_every_world_and_obeys_independent_gates() {
        for scene in 0..WORLD_COUNT {
            let make = |mappings: &AudioMappings| {
                ShaderUniforms::new(
                    0.0,
                    0.016,
                    scene,
                    scene,
                    1.0,
                    &VisualSettings::default(),
                    mappings,
                    &AudioFeatures::default(),
                )
            };
            let mut mappings = AudioMappings::default();
            let active = [0.7, 0.8, 0.5, 0.6];
            assert_eq!(make(&mappings).with_music_response(active).music, active);
            assert_eq!(
                make(&mappings)
                    .with_music_response(active)
                    .with_audio_state(false, 1)
                    .music,
                [0.0; 4]
            );
            assert_eq!(
                make(&mappings).with_music_response([f32::NAN; 4]).music,
                [0.0; 4]
            );
            mappings.beat_to_flash = 0.0;
            assert_eq!(
                make(&mappings).with_music_response(active).music,
                [0.0, 0.0, 0.5, 0.6]
            );
            mappings.bass_to_pulse = 0.0;
            mappings.treble_to_sparkle = 0.0;
            assert_eq!(make(&mappings).with_music_response(active).music, [0.0; 4]);
        }
    }
}
