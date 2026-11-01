//! Real geometry passes: instanced point sprites, connected numerical chaos
//! trajectories and a depth-tested mesh of recent measured frequency history.
use super::{ShaderUniforms, wgpu};
use wgpu::util::DeviceExt as _;

pub(super) const RASTER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
pub(super) const SHADER_SOURCE: &str = concat!(
    include_str!("../params.wgsl"),
    "\n",
    include_str!("../raster.wgsl"),
    "\n",
    include_str!("../raster/constellation.wgsl"),
    "\n",
    include_str!("../raster/attractor.wgsl"),
    "\n",
    include_str!("../raster/dot_equaliser.wgsl"),
    "\n",
    include_str!("../raster/dot_morph.wgsl"),
    "\n",
    include_str!("../raster/disco_silk.wgsl"),
);
const RASTER_WORLD_COUNT: usize = 5;
const RASTER_PIPELINE_COUNT: usize = RASTER_WORLD_COUNT + 1;
const RASTER_WORLD_IDS: [f32; RASTER_WORLD_COUNT] = [4.0, 5.0, 6.0, 7.0, 12.0];
const DOT_MORPH_PIPELINE: usize = 4;
const DISCO_SILK_PIPELINE: usize = 5;
const DOT_EQUALISER_POINTS: u32 = 50 * 50 * 20;
const HISTORY_ROWS: usize = 64;
const BANDS: usize = 48;
const STEP: f64 = 0.10;
const DOT_CURRENT_OFFSET: usize = HISTORY_ROWS * BANDS;
const ATTRACTOR_RMS_OFFSET: usize = DOT_CURRENT_OFFSET + BANDS;

fn band_group(band: usize) -> usize {
    match band {
        0..=7 => 0,
        8..=23 => 1,
        _ => 2,
    }
}

/// These samples are already gain-mapped. Changing one slider remaps only
/// its retained evidence; beat gain has no frequency-history lane at all.
fn mapping_scales(previous: [f32; 4], next: [f32; 4]) -> [Option<f64>; 3] {
    let gain = |v: f32| {
        if v.is_finite() {
            v.clamp(0.0, 1.5)
        } else {
            0.0
        }
    };
    std::array::from_fn(|group| {
        let before = gain(previous[group]);
        let after = gain(next[group]);
        (before != after).then(|| {
            if before > 0.0 {
                f64::from(after) / f64::from(before)
            } else {
                0.0
            }
        })
    })
}

/// Measured rows stay immutable after insertion and scroll as actual geometry.
/// Interpolation is only between the two observed frames bracketing a row time,
/// so a repaint after that time does not move the ridge to the wrong timestamp.
struct SpectralHistory {
    samples: Vec<f32>,
    previous: [f32; BANDS],
    head: usize,
    elapsed: f64,
    stamp: Option<f32>,
    epoch: Option<f32>,
    mappings: Option<[f32; 4]>,
    initialized: bool,
}

impl Default for SpectralHistory {
    fn default() -> Self {
        Self {
            samples: vec![0.0; HISTORY_ROWS * BANDS],
            previous: [0.0; BANDS],
            head: 0,
            elapsed: 0.0,
            stamp: None,
            epoch: None,
            mappings: None,
            initialized: false,
        }
    }
}

impl SpectralHistory {
    fn prepare(&mut self, uniforms: &mut ShaderUniforms) -> bool {
        let epoch = uniforms.raster[3];
        let fresh = uniforms.raster[2] > 0.0;
        let mut changed = false;
        if self.epoch != Some(epoch) || !fresh {
            self.samples.fill(0.0);
            self.previous.fill(0.0);
            self.head = 0;
            self.elapsed = 0.0;
            self.stamp = None;
            self.initialized = false;
            self.epoch = Some(epoch);
            changed = true;
        } else if let Some(previous) = self.mappings {
            let scales = mapping_scales(previous, uniforms.values[4]);
            if scales.iter().any(Option::is_some) {
                for (index, sample) in self.samples.iter_mut().enumerate() {
                    if let Some(scale) = scales[band_group(index % BANDS)] {
                        *sample = (f64::from(*sample) * scale).clamp(0.0, 1.5) as f32;
                    }
                }
                for (band, sample) in self.previous.iter_mut().enumerate() {
                    if let Some(scale) = scales[band_group(band)] {
                        *sample = (f64::from(*sample) * scale).clamp(0.0, 1.5) as f32;
                    }
                }
                changed = true;
            }
        }
        self.mappings = Some(uniforms.values[4]);
        let stamp = uniforms.values[8][0];
        if fresh && self.stamp != Some(stamp) {
            self.stamp = Some(stamp);
            let current = std::array::from_fn::<_, BANDS, _>(|band| {
                let value = uniforms.spectrum[band / 4][band % 4];
                if value.is_finite() {
                    value.clamp(0.0, 1.5)
                } else {
                    0.0
                }
            });
            if !self.initialized {
                // No history is fabricated before the first observed frame.
                self.samples[..BANDS].copy_from_slice(&current);
                self.initialized = true;
                changed = true;
            } else {
                let dt = uniforms.values[6][1];
                let dt = if dt.is_finite() {
                    f64::from(dt.clamp(0.0, 0.25))
                } else {
                    0.0
                };
                let total = self.elapsed + dt;
                let rows = (total / STEP).floor() as usize;
                for row in 0..rows {
                    let fraction =
                        ((STEP - self.elapsed + row as f64 * STEP) / dt).clamp(0.0, 1.0) as f32;
                    self.head = (self.head + 1) % HISTORY_ROWS;
                    for (band, measured) in current.iter().enumerate() {
                        self.samples[self.head * BANDS + band] =
                            self.previous[band] + (*measured - self.previous[band]) * fraction;
                    }
                }
                self.elapsed = total - rows as f64 * STEP;
                changed |= rows > 0;
            }
            self.previous = current;
        }
        uniforms.raster[0] = self.head as f32;
        uniforms.raster[1] = (self.elapsed / STEP) as f32;
        changed
    }
}

/// A separate response/history lane for Dot Equaliser and the line sculpture's
/// local flex. Original uniforms and landscape history remain unmodified.
/// Two cascaded poles soften both the height and its movement; their exact
/// constant-input solution avoids frame-rate-dependent Euler integration.
struct DotEqualiserHistory {
    history: SpectralHistory,
    first: [f64; BANDS],
    second: [f64; BANDS],
    // Binding 2: retained rows [0, 3072), current 48 filtered bands, then
    // three Attractor group RMS values and one zero padding lane. Existing
    // Dot Equaliser/history offsets and the uniform ABI remain unchanged.
    upload: Vec<f32>,
}

impl Default for DotEqualiserHistory {
    fn default() -> Self {
        Self {
            history: SpectralHistory::default(),
            first: [0.0; BANDS],
            second: [0.0; BANDS],
            upload: vec![0.0; ATTRACTOR_RMS_OFFSET + 4],
        }
    }
}

impl DotEqualiserHistory {
    fn prepare(&mut self, original: &ShaderUniforms) -> bool {
        let fresh = original.raster[2] > 0.0;
        let reset = self.history.epoch != Some(original.raster[3]) || !fresh;
        if reset {
            self.first.fill(0.0);
            self.second.fill(0.0);
        } else if let Some(previous) = self.history.mappings {
            let scales = mapping_scales(previous, original.values[4]);
            for band in 0..BANDS {
                if let Some(scale) = scales[band_group(band)] {
                    self.first[band] = (self.first[band] * scale).clamp(0.0, 1.5);
                    self.second[band] = (self.second[band] * scale).clamp(0.0, 1.5);
                }
            }
        }
        let new_frame = fresh && self.history.stamp != Some(original.values[8][0]);
        let dt = original.values[6][1];
        let dt = if dt.is_finite() {
            f64::from(dt.clamp(0.0, 0.25))
        } else {
            0.0
        };
        if new_frame && !reset && dt > 0.0 {
            for band in 0..BANDS {
                let gate = match band {
                    0..=7 => original.values[4][0],
                    8..=23 => original.values[4][1],
                    _ => original.values[4][2],
                };
                // Inputs are already mapped once. Do not multiply again, and
                // never let a disconnected band decay with old sound in it.
                if !gate.is_finite() || gate <= 0.0 {
                    self.first[band] = 0.0;
                    self.second[band] = 0.0;
                    continue;
                }
                let measured = original.spectrum[band / 4][band % 4];
                let target = if measured.is_finite() {
                    f64::from(measured.clamp(0.0, 1.5))
                } else {
                    0.0
                };
                // Each pole uses 55 ms attack / 180 ms release. A held step
                // reaches 90% in about 214 ms; a settled value releases 90% in
                // about 700 ms. Preserve clear attacks without invented peaks.
                let tau = if target >= self.first[band] {
                    0.055
                } else {
                    0.18
                };
                let step = dt / tau;
                let decay = (-step).exp();
                let first_offset = self.first[band] - target;
                self.second[band] = (target
                    + (self.second[band] - target + first_offset * step) * decay)
                    .clamp(0.0, 1.5);
                self.first[band] = (target + first_offset * decay).clamp(0.0, 1.5);
            }
        }

        // Preserve all original clocks, gates and metadata. This history sees
        // exactly the same physical frames as the original, but different data.
        let mut filtered = *original;
        for band in 0..BANDS {
            filtered.spectrum[band / 4][band % 4] = self.second[band] as f32;
        }
        let history_changed = self.history.prepare(&mut filtered);
        if history_changed {
            self.upload[..DOT_CURRENT_OFFSET].copy_from_slice(&self.history.samples);
        }
        let mut current_changed = false;
        let mut energy = [0.0_f32; 3];
        for band in 0..BANDS {
            let value = self.second[band] as f32;
            let current = &mut self.upload[DOT_CURRENT_OFFSET + band];
            current_changed |= *current != value;
            *current = value;
            let group = match band {
                0..=7 => 0,
                8..=23 => 1,
                _ => 2,
            };
            energy[group] += value * value;
        }
        // Reduce the very same filtered f32 bands once on the CPU, instead
        // of repeating 48 storage reads at every Attractor line vertex.
        // Silence, stale sources and disconnected mappings are already zero
        // in this lane; never reapply a mapping or invent normalized energy.
        for (group, count) in [8.0, 16.0, 24.0].into_iter().enumerate() {
            let rms = (energy[group] / count).sqrt();
            let current = &mut self.upload[ATTRACTOR_RMS_OFFSET + group];
            current_changed |= *current != rms;
            *current = rms;
        }
        // The live leading lane must upload between the 10 Hz history rows.
        history_changed || current_changed
    }
}

pub(super) struct RasterWorlds {
    pipelines: [wgpu::RenderPipeline; RASTER_PIPELINE_COUNT],
    group: wgpu::BindGroup,
    trajectory: wgpu::Buffer,
    spectral_buffer: wgpu::Buffer,
    history: SpectralHistory,
    dot_spectral_buffer: wgpu::Buffer,
    dot_history: DotEqualiserHistory,
    targets: Targets,
}

/// The paired dot passage is one geometry pass in the destination's layer.
/// Its compositor must sample that layer directly, never crossfade two clouds.
/// At progress zero, use the unchanged source pipeline/count in that layer.
fn dot_morph_pass(uniforms: &ShaderUniforms) -> Option<(usize, usize, u32)> {
    let destination = uniforms.values[0][3];
    let source = uniforms.values[5][2];
    let amount = uniforms.values[5][3];
    if !(0.0..1.0).contains(&amount)
        || !((destination == 4.0 && source == 7.0) || (destination == 7.0 && source == 4.0))
    {
        return None;
    }
    let layer = destination as usize - 4;
    if amount == 0.0 {
        let pipeline = source as usize - 4;
        let instances = if pipeline == 0 {
            12_000
        } else {
            DOT_EQUALISER_POINTS
        };
        Some((layer, pipeline, instances))
    } else {
        Some((layer, DOT_MORPH_PIPELINE, DOT_EQUALISER_POINTS))
    }
}

struct Targets {
    size: [u32; 2],
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    layers: [wgpu::TextureView; RASTER_WORLD_COUNT],
    _depth: wgpu::Texture,
    depth: wgpu::TextureView,
}

impl Targets {
    fn new(device: &wgpu::Device, size: [u32; 2]) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("five distinct geometry worlds"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: RASTER_WORLD_COUNT as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: RASTER_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let layers = std::array::from_fn(|layer| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: layer as u32,
                array_layer_count: Some(1),
                ..Default::default()
            })
        });
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("spectral landscape depth"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&Default::default());
        Self {
            size,
            _texture: texture,
            view,
            layers,
            _depth: depth,
            depth: depth_view,
        }
    }
}

impl RasterWorlds {
    pub(super) fn new(device: &wgpu::Device, uniform: &wgpu::Buffer) -> Self {
        let history = SpectralHistory::default();
        let spectral_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bounded in-memory measured spectral history"),
            contents: bytemuck::cast_slice(&history.samples),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let dot_history = DotEqualiserHistory::default();
        let dot_spectral_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Dot Equaliser smoothed history and current spectrum"),
            contents: bytemuck::cast_slice(&dot_history.upload),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let points = crate::attractor::attractor_points();
        let trajectory = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("actual precomputed RK4 Lorenz Rossler Thomas samples"),
            contents: bytemuck::cast_slice(&points),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let bindings = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("geometry audio uniforms and bounded spectrum history"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("geometry uniforms"),
            layout: &bindings,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: spectral_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: dot_spectral_buffer.as_entire_binding(),
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("real geometry layout"),
            bind_group_layouts: &[Some(&bindings)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("point cloud chaos trajectory and spectral mesh"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });
        let current_attributes =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];
        let next_attributes =
            wgpu::vertex_attr_array![3 => Float32x3, 4 => Float32x3, 5 => Float32x3];
        let buffers = [
            Some(wgpu::VertexBufferLayout {
                array_stride: 36,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &current_attributes,
            }),
            Some(wgpu::VertexBufferLayout {
                array_stride: 36,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &next_attributes,
            }),
        ];
        let additive = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent::OVER,
        };
        let entries = [
            "vs_constellation",
            "vs_attractor",
            "vs_landscape",
            "vs_dot_equaliser",
            "vs_dot_morph",
            "vs_disco_silk",
        ];
        let pipelines = std::array::from_fn(|index| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entries[index]),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(entries[index]),
                    buffers: if index == 1 { &buffers } else { &[] },
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(if index == DISCO_SILK_PIPELINE {
                        "fs_disco_silk"
                    } else {
                        "fs_raster"
                    }),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: RASTER_FORMAT,
                        blend: if index == 2 {
                            None
                        } else if index == DISCO_SILK_PIPELINE {
                            Some(wgpu::BlendState::ALPHA_BLENDING)
                        } else {
                            Some(additive)
                        },
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(index == 2 || index == DISCO_SILK_PIPELINE),
                    depth_compare: Some(if index == 2 || index == DISCO_SILK_PIPELINE {
                        wgpu::CompareFunction::Less
                    } else {
                        wgpu::CompareFunction::Always
                    }),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        });
        Self {
            pipelines,
            group,
            trajectory,
            spectral_buffer,
            history,
            dot_spectral_buffer,
            dot_history,
            targets: Targets::new(device, [1, 1]),
        }
    }

    pub(super) fn ensure_size(&mut self, device: &wgpu::Device, size: [u32; 2]) {
        if self.targets.size != size {
            self.targets = Targets::new(device, size);
        }
    }

    pub(super) fn view(&self) -> &wgpu::TextureView {
        &self.targets.view
    }

    /// Diagnostic only: retain the actual Silk vertices/depth/history, but
    /// expose their normals independently of musical pigment and lighting.
    /// The production constructor never calls this override.
    pub(super) fn probe_silk_normals(&mut self, device: &wgpu::Device) {
        let source = format!(
            "{SHADER_SOURCE}\n{}",
            r#"
@fragment fn fs_silk_normals(in: SilkVertex) -> @location(0) vec4<f32> {
    let footprint = max(fwidth(in.local), vec2<f32>(0.0001));
    let edge = 1.0 - smoothstep(1.0 - footprint.y * 1.1, 1.0, abs(in.local.y));
    let coverage = edge * (1.0 - smoothstep(23.0, 31.5, -in.point.z));
    if coverage <= 0.001 { discard; }
    return vec4<f32>(silk_unit(in.normal) * 0.5 + 0.5, coverage);
}"#
        );
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("diagnostic actual Silk normals"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let bindings = self.pipelines[DISCO_SILK_PIPELINE].get_bind_group_layout(0);
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&bindings)],
            immediate_size: 0,
        });
        self.pipelines[DISCO_SILK_PIPELINE] =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("diagnostic Silk geometry, unchanged draw/resources"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_disco_silk"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_silk_normals"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: RASTER_FORMAT,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
    }

    pub(super) fn prepare(&mut self, queue: &wgpu::Queue, uniforms: &mut ShaderUniforms) {
        if self.history.prepare(uniforms) {
            queue.write_buffer(
                &self.spectral_buffer,
                0,
                bytemuck::cast_slice(&self.history.samples),
            );
        }
        if self.dot_history.prepare(uniforms) {
            queue.write_buffer(
                &self.dot_spectral_buffer,
                0,
                bytemuck::cast_slice(&self.dot_history.upload),
            );
        }
        debug_assert_eq!(self.history.head, self.dot_history.history.head);
        debug_assert_eq!(self.history.elapsed, self.dot_history.history.elapsed);
    }

    pub(super) fn render(&self, encoder: &mut wgpu::CommandEncoder, uniforms: &ShaderUniforms) {
        let scenes = [uniforms.values[0][3], uniforms.values[5][2]];
        let active_source = uniforms.values[5][3] < 1.0;
        let dot_pass = dot_morph_pass(uniforms);
        for (index, &id) in RASTER_WORLD_IDS.iter().enumerate() {
            if let Some((layer, _, _)) = dot_pass {
                if index != layer {
                    continue;
                }
            } else if scenes[0] != id && !(active_source && scenes[1] == id) {
                continue;
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("actual geometry world prepass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.targets.layers[index],
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            let pipeline = if index == 4 {
                DISCO_SILK_PIPELINE
            } else {
                index
            };
            pass.set_pipeline(
                &self.pipelines[dot_pass.map_or(pipeline, |(_, pipeline, _)| pipeline)],
            );
            pass.set_bind_group(0, &self.group, &[]);
            if let Some((_, _, instances)) = dot_pass {
                pass.draw(0..6, 0..instances);
                continue;
            }
            match index {
                0 => pass.draw(0..6, 0..12_000),
                1 => {
                    pass.set_vertex_buffer(0, self.trajectory.slice(..));
                    pass.set_vertex_buffer(1, self.trajectory.slice(36..));
                    pass.draw(0..6, 0..11_999);
                }
                2 => pass.draw(0..47 * 63 * 6, 0..1),
                3 => pass.draw(0..6, 0..DOT_EQUALISER_POINTS),
                _ => pass.draw(0..55_296, 0..1),
            }
        }
    }
}

#[cfg(test)]
mod history_tests {
    use super::*;
    use crate::{
        dsp::AudioFeatures,
        presets::{AudioMappings, VisualSettings},
    };

    fn frame(time: f32, dt: f32, level: f32) -> ShaderUniforms {
        let audio = AudioFeatures {
            spectrum: vec![level; BANDS],
            ..Default::default()
        };
        let mappings = AudioMappings {
            bass_to_pulse: 1.0,
            mids_to_geometry: 1.0,
            treble_to_sparkle: 1.0,
            beat_to_flash: 1.0,
        };
        ShaderUniforms::new(
            time,
            dt,
            6,
            6,
            1.0,
            &VisualSettings::default(),
            &mappings,
            &audio,
        )
        .with_audio_state(true, 1)
    }

    #[test]
    fn dot_world_morph_is_one_destination_pass_with_exact_endpoint_pipelines() {
        for (source, destination, source_pipeline, source_count) in
            [(4.0, 7.0, 0, 12_000), (7.0, 4.0, 3, 50_000)]
        {
            let mut input = frame(0.0, 0.0, 0.3);
            input.values[0][3] = destination;
            input.values[5][2] = source;
            input.values[5][3] = 0.0;
            let layer = destination as usize - 4;
            assert_eq!(
                dot_morph_pass(&input),
                Some((layer, source_pipeline, source_count))
            );
            for amount in [0.001, 0.25, 0.5, 0.75, 0.999] {
                input.values[5][3] = amount;
                assert_eq!(dot_morph_pass(&input), Some((layer, 4, 50_000)));
            }
            input.values[5][3] = 1.0;
            assert_eq!(dot_morph_pass(&input), None);
        }
        for source in 0..8 {
            for destination in 0..8 {
                if (source == 4 && destination == 7) || (source == 7 && destination == 4) {
                    continue;
                }
                let mut input = frame(0.0, 0.0, 0.3);
                input.values[0][3] = destination as f32;
                input.values[5][2] = source as f32;
                input.values[5][3] = 0.5;
                assert_eq!(dot_morph_pass(&input), None);
            }
        }
    }

    #[test]
    fn dot_world_morph_correspondence_preserves_constellation_light() {
        let mut counts = [0_u32; 12_000];
        for index in 0..DOT_EQUALISER_POINTS {
            counts[index as usize % 12_000] += 1;
        }
        for (origin, count) in counts.into_iter().enumerate() {
            let multiplicity = 1 + (DOT_EQUALISER_POINTS - 1 - origin as u32) / 12_000;
            assert_eq!(count, multiplicity);
            assert!((count as f32 * (1.0 / multiplicity as f32) - 1.0).abs() < 0.000001);
        }
    }

    #[test]
    fn rows_use_interpolated_boundary_time_not_late_repaint_value() {
        let mut history = SpectralHistory::default();
        history.prepare(&mut frame(0.0, 0.0, 0.0));
        assert!(!history.prepare(&mut frame(0.06, 0.06, 0.48)));
        let mut next = frame(0.12, 0.06, 0.96);
        assert!(history.prepare(&mut next));
        assert_eq!(history.head, 1);
        assert!((history.samples[BANDS] - 0.80).abs() < 0.00001);
        assert!((next.raster[1] - 0.20).abs() < 0.00001);
        // Unobserved earlier rows remain silent, not a repeated current wave.
        assert!(history.samples[BANDS * 2..].iter().all(|v| *v == 0.0));
    }

    #[test]
    fn retained_ridge_scroll_and_grid_phase_are_continuous_at_row_insertion() {
        let mut history = SpectralHistory::default();
        history.prepare(&mut frame(0.0, 0.0, 0.2));
        let mut before = frame(0.099, 0.099, 0.7);
        history.prepare(&mut before);
        let retained_value = history.samples[0];
        let before_age = before.raster[1]; // old head is row 1
        let mut after = frame(0.101, 0.002, 0.8);
        history.prepare(&mut after);
        let after_age = 1.0 + after.raster[1]; // old head is now row 2
        assert_eq!(history.samples[0], retained_value);
        assert!((after_age - before_age - 0.02).abs() < 0.00001);
        let before_grid = before_age + 1.0 - before.raster[1];
        let after_grid = after_age + 1.0 - after.raster[1];
        assert!((after_grid - before_grid - 1.0).abs() < 0.00001);
        assert_eq!(history.head, 1);
    }

    #[test]
    fn history_is_consistent_at_30_60_and_120_hz() {
        let mut captures = Vec::new();
        for hz in [30, 60, 120] {
            let mut history = SpectralHistory::default();
            history.prepare(&mut frame(0.0, 0.0, 0.0));
            for index in 1..=hz * 3 {
                let time = index as f32 / hz as f32;
                history.prepare(&mut frame(time, 1.0 / hz as f32, time * 0.25));
            }
            assert_eq!(history.head, 30);
            captures.push(history.samples);
        }
        for capture in &captures[1..] {
            for (a, b) in captures[0].iter().zip(capture) {
                assert!((a - b).abs() < 0.00001);
            }
        }
    }

    #[test]
    fn repeated_submission_and_zero_delta_do_not_advance_history() {
        let mut history = SpectralHistory::default();
        let mut initial = frame(0.0, 0.1, 0.7);
        history.prepare(&mut initial);
        assert!(!history.prepare(&mut initial));
        assert!(!history.prepare(&mut frame(0.1, 0.0, 0.8)));
        assert_eq!(history.head, 0);
        assert_eq!(history.elapsed, 0.0);
    }

    #[test]
    fn stale_source_epoch_and_zero_mappings_cannot_retain_old_sound() {
        let mut history = SpectralHistory::default();
        history.prepare(&mut frame(0.0, 0.0, 0.8));
        history.prepare(&mut frame(0.2, 0.2, 0.9));
        assert!(history.samples.iter().any(|v| *v > 0.0));
        let mut stale = frame(0.3, 0.1, 1.0).with_audio_state(false, 1);
        history.prepare(&mut stale);
        assert!(history.samples.iter().all(|v| *v == 0.0));
        assert_eq!(stale.raster[1], 0.0);
        history.prepare(&mut frame(0.4, 0.1, 0.3).with_audio_state(true, 2));
        assert_eq!(history.samples[0], 0.3);
        assert!(history.samples[BANDS..].iter().all(|v| *v == 0.0));
        let mut disconnected = frame(0.5, 0.1, 1.0);
        disconnected.values[4] = [0.0; 4];
        disconnected.spectrum = [[0.0; 4]; 12];
        history.prepare(&mut disconnected);
        assert!(history.samples.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn dot_equaliser_step_is_eased_bounded_and_live_uploads_between_history_rows() {
        let mut dots = DotEqualiserHistory::default();
        assert!(dots.prepare(&frame(0.0, 0.0, 0.0)));
        let mut previous = 0.0_f32;
        for index in 1..=60 {
            assert!(dots.prepare(&frame(index as f32 / 60.0, 1.0 / 60.0, 1.0)));
            let value = dots.upload[DOT_CURRENT_OFFSET];
            assert!((previous..=1.0).contains(&value));
            assert!(
                value - previous < 0.115,
                "an FFT step must not teleport the cloud"
            );
            if index == 1 {
                assert!((0.0..0.045).contains(&value));
                assert_eq!(dots.history.head, 0);
                assert!(dots.history.samples.iter().all(|sample| *sample == 0.0));
            }
            if index == 13 {
                assert!(
                    value > 0.90,
                    "attacks must remain clearly visible within 217 ms"
                );
            }
            previous = value;
        }
        assert!(previous > 0.99);
        for index in 61..=240 {
            dots.prepare(&frame(index as f32 / 60.0, 1.0 / 60.0, 0.0));
            let value = dots.upload[DOT_CURRENT_OFFSET];
            assert!((0.0..=previous + 0.00001).contains(&value));
            assert!(previous - value < 0.036);
            previous = value;
        }
        assert!(previous < 0.00002);
        assert_eq!(dots.upload.len(), 64 * 48 + 48 + 4);
        assert_eq!(DOT_EQUALISER_POINTS, 50_000);
    }

    #[test]
    fn dot_equaliser_response_is_consistent_at_30_60_and_120_hz() {
        let mut captures = Vec::new();
        for hz in [30, 60, 120] {
            let mut dots = DotEqualiserHistory::default();
            dots.prepare(&frame(0.0, 0.0, 0.0));
            let mut current = Vec::new();
            for index in 1..=hz * 2 {
                let level = if index <= hz { 0.9 } else { 0.0 };
                dots.prepare(&frame(index as f32 / hz as f32, 1.0 / hz as f32, level));
                if index % (hz / 10) == 0 {
                    current.push(dots.upload[DOT_CURRENT_OFFSET]);
                }
            }
            captures.push((current, dots.history.samples));
        }
        for (current, history) in &captures[1..] {
            for (a, b) in captures[0].0.iter().zip(current) {
                assert!((a - b).abs() < 0.00001, "same physical time, same response");
            }
            for (a, b) in captures[0].1.iter().zip(history) {
                assert!((a - b).abs() < 0.00001);
            }
        }
    }

    #[test]
    fn gain_edits_remap_only_their_band_without_resetting_history_or_time() {
        let mut history = SpectralHistory::default();
        let mut dots = DotEqualiserHistory::default();
        let mut input = frame(0.0, 0.0, 0.6);
        for i in 0..=60 {
            input = frame(i as f32 / 60.0, 1.0 / 60.0, 0.6);
            history.prepare(&mut input);
            dots.prepare(&input);
        }
        let original_rows = history.samples.clone();
        let original_dot_rows = dots.history.samples.clone();
        let first = dots.first;
        let second = dots.second;
        let upload = dots.upload.clone();
        let clock = (history.head, history.elapsed, history.stamp);
        // A same-frame beat slider edit cannot erase or advance any FFT state.
        input.values[4][3] = 0.0;
        assert!(!history.prepare(&mut input));
        assert!(!dots.prepare(&input));
        assert_eq!(history.samples, original_rows);
        assert_eq!(dots.upload, upload);
        assert_eq!(dots.first, first);
        assert_eq!(dots.second, second);
        // Halving bass preserves every mid/high byte and the shared row clock.
        input.values[4][0] *= 0.5;
        for band in 0..8 {
            input.spectrum[band / 4][band % 4] *= 0.5;
        }
        history.prepare(&mut input);
        dots.prepare(&input);
        for band in 0..BANDS {
            let scale = if band < 8 { 0.5 } else { 1.0 };
            assert_eq!(dots.first[band], first[band] * scale);
            assert_eq!(dots.second[band], second[band] * scale);
        }
        for index in 0..HISTORY_ROWS * BANDS {
            let scale = if index % BANDS < 8 { 0.5 } else { 1.0 };
            assert_eq!(history.samples[index], original_rows[index] * scale);
            assert_eq!(
                dots.history.samples[index],
                original_dot_rows[index] * scale
            );
        }
        assert_eq!((history.head, history.elapsed, history.stamp), clock);
        assert_eq!(
            (dots.history.head, dots.history.elapsed, dots.history.stamp),
            clock
        );
        // Disconnect/reconnect cannot resurrect bass; unrelated bands remain.
        input.values[4][0] = 0.0;
        history.prepare(&mut input);
        dots.prepare(&input);
        assert!(
            dots.first[..8]
                .iter()
                .chain(&dots.second[..8])
                .all(|x| *x == 0.0)
        );
        assert!(
            history
                .samples
                .iter()
                .enumerate()
                .filter(|(i, _)| i % BANDS < 8)
                .all(|(_, v)| *v == 0.0)
        );
        input.values[4][0] = 1.5;
        history.prepare(&mut input);
        dots.prepare(&input);
        assert!(dots.second[..8].iter().all(|x| *x == 0.0));
        assert_eq!(&dots.second[8..], &second[8..]);
        assert_eq!((history.head, history.elapsed, history.stamp), clock);
    }

    #[test]
    fn attractor_rms_covers_every_filtered_bin_without_changing_dot_offsets() {
        for active in 0..BANDS {
            let mut dots = DotEqualiserHistory::default();
            dots.prepare(&frame(0.0, 0.0, 0.0));
            for index in 1..=30 {
                let mut input = frame(index as f32 / 60.0, 1.0 / 60.0, 0.0);
                input.spectrum[active / 4][active % 4] = 0.9;
                dots.prepare(&input);
            }
            let (group, count) = match active {
                0..=7 => (0, 8.0_f32),
                8..=23 => (1, 16.0_f32),
                _ => (2, 24.0_f32),
            };
            let filtered = dots.upload[DOT_CURRENT_OFFSET + active];
            assert!(filtered > 0.89);
            for lane in 0..3 {
                let expected = if lane == group {
                    filtered / count.sqrt()
                } else {
                    0.0
                };
                assert!((dots.upload[ATTRACTOR_RMS_OFFSET + lane] - expected).abs() < 0.000_001);
            }
            assert_eq!(&dots.upload[..DOT_CURRENT_OFFSET], dots.history.samples);
            for band in 0..BANDS {
                assert_eq!(
                    dots.upload[DOT_CURRENT_OFFSET + band],
                    dots.second[band] as f32
                );
            }
            assert_eq!(dots.upload[ATTRACTOR_RMS_OFFSET + 3], 0.0);
        }
    }

    #[test]
    fn attractor_rms_flushes_on_source_or_band_mapping_changes() {
        for change_source in [true, false] {
            let mut dots = DotEqualiserHistory::default();
            dots.prepare(&frame(0.0, 0.0, 0.0));
            for index in 1..=30 {
                dots.prepare(&frame(index as f32 / 60.0, 1.0 / 60.0, 0.9));
            }
            assert!(
                dots.upload[ATTRACTOR_RMS_OFFSET..ATTRACTOR_RMS_OFFSET + 3]
                    .iter()
                    .all(|rms| *rms > 0.89)
            );
            let mut changed = frame(0.6, 0.1, 0.9);
            if change_source {
                changed = changed.with_audio_state(true, 2);
            } else {
                changed.values[4][1] = 0.0;
            }
            assert!(dots.prepare(&changed));
            if change_source {
                assert!(dots.upload.iter().all(|value| *value == 0.0));
            } else {
                assert_eq!(dots.upload[ATTRACTOR_RMS_OFFSET + 1], 0.0);
                assert!(dots.upload[ATTRACTOR_RMS_OFFSET] > 0.89);
                assert!(dots.upload[ATTRACTOR_RMS_OFFSET + 2] > 0.89);
            }
            changed.values[8][0] = 0.7;
            assert!(dots.prepare(&changed));
            assert!(dots.upload[ATTRACTOR_RMS_OFFSET] > 0.0);
            assert!(dots.upload[ATTRACTOR_RMS_OFFSET + 2] > 0.0);
            if change_source {
                assert!(dots.upload[ATTRACTOR_RMS_OFFSET + 1] > 0.0);
            } else {
                assert_eq!(dots.upload[ATTRACTOR_RMS_OFFSET + 1], 0.0);
            }
            let before = dots.upload.clone();
            assert!(!dots.prepare(&changed));
            assert_eq!(dots.upload, before);
        }
    }

    #[test]
    fn dot_history_clock_tracks_original_without_changing_its_data() {
        let mut original = SpectralHistory::default();
        let mut reference = SpectralHistory::default();
        let mut dots = DotEqualiserHistory::default();
        let mut saw_wrap = false;
        let mut previous_head = 0;
        for index in 0..2400 {
            let mut input = frame(
                index as f32,
                [0.0, 0.012, 0.03, 0.2][index % 4],
                (index % 67) as f32 / 67.0,
            )
            .with_audio_state(
                !(400..405).contains(&index),
                if index < 900 { 1 } else { 2 },
            );
            if (1200..1300).contains(&index) {
                input.values[4][0] = 0.0;
                input.spectrum[0] = [0.0; 4];
                input.spectrum[1] = [0.0; 4];
            }
            let spectrum = input.spectrum;
            let mut expected = input;
            reference.prepare(&mut expected);
            original.prepare(&mut input);
            dots.prepare(&input);
            assert_eq!(input.spectrum, spectrum, "never replace other worlds' FFT");
            assert_eq!(original.samples, reference.samples);
            assert_eq!(original.previous, reference.previous);
            assert_eq!(original.head, dots.history.head);
            assert_eq!(original.elapsed, dots.history.elapsed);
            assert_eq!(original.stamp, dots.history.stamp);
            assert_eq!(original.epoch, dots.history.epoch);
            assert_eq!(original.mappings, dots.history.mappings);
            assert_eq!(input.raster, expected.raster);
            saw_wrap |= previous_head == HISTORY_ROWS - 1 && original.head < 3;
            previous_head = original.head;
        }
        assert!(
            saw_wrap,
            "clock alignment must include real buffer wrapping"
        );
    }

    #[test]
    fn dot_equaliser_silence_invalid_inputs_and_repeated_frames_invent_no_energy() {
        let mut dots = DotEqualiserHistory::default();
        for index in 0..300 {
            let mut input = frame(index as f32 / 60.0, 1.0 / 60.0, 0.0);
            if index % 2 == 0 {
                input.spectrum = [[f32::NAN; 4]; 12];
            }
            dots.prepare(&input);
            assert!(dots.upload.iter().all(|value| *value == 0.0));
        }
        let initial = frame(10.0, 0.1, 0.8);
        dots.prepare(&initial);
        let before = dots.upload.clone();
        let head = dots.history.head;
        let elapsed = dots.history.elapsed;
        assert!(!dots.prepare(&initial));
        assert_eq!(dots.upload, before);
        assert_eq!(dots.history.head, head);
        assert_eq!(dots.history.elapsed, elapsed);
        dots.prepare(&frame(11.0, 0.0, 1.5));
        assert_eq!(
            &dots.upload[DOT_CURRENT_OFFSET..],
            &before[DOT_CURRENT_OFFSET..]
        );
    }

    #[test]
    fn dot_equaliser_resets_clear_stale_source_and_mapping_energy_without_replay() {
        let mut dots = DotEqualiserHistory::default();
        dots.prepare(&frame(0.0, 0.0, 0.0));
        for index in 1..=60 {
            dots.prepare(&frame(index as f32 / 60.0, 1.0 / 60.0, 0.8));
        }
        assert!(dots.upload[DOT_CURRENT_OFFSET] > 0.79);
        assert!(
            dots.upload[ATTRACTOR_RMS_OFFSET..ATTRACTOR_RMS_OFFSET + 3]
                .iter()
                .all(|rms| *rms > 0.79)
        );
        dots.prepare(&frame(1.1, 0.1, 1.0).with_audio_state(false, 1));
        assert!(dots.upload.iter().all(|value| *value == 0.0));
        dots.prepare(&frame(1.2, 0.1, 0.0).with_audio_state(true, 2));
        assert!(dots.upload.iter().all(|value| *value == 0.0));
        for index in 1..=60 {
            dots.prepare(
                &frame(2.0 + index as f32 / 60.0, 1.0 / 60.0, 0.9).with_audio_state(true, 2),
            );
        }
        assert!(dots.upload[DOT_CURRENT_OFFSET] > 0.89);
        assert!(
            dots.upload[ATTRACTOR_RMS_OFFSET..ATTRACTOR_RMS_OFFSET + 3]
                .iter()
                .all(|rms| *rms > 0.89)
        );
        let mut disconnected = frame(4.0, 0.1, 1.0).with_audio_state(true, 2);
        disconnected.values[4] = [0.0; 4];
        // Even an incorrectly populated disconnected input cannot replay sound.
        dots.prepare(&disconnected);
        assert!(dots.upload.iter().all(|value| *value == 0.0));
        disconnected.values[8][0] = 4.1;
        dots.prepare(&disconnected);
        assert!(dots.upload.iter().all(|value| *value == 0.0));
        dots.prepare(&frame(4.2, 0.1, 0.0).with_audio_state(true, 2));
        assert!(dots.upload.iter().all(|value| *value == 0.0));
    }
}
