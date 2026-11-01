// Browser adapter for the Mac's original geometry shaders. The equations,
// materials and compositing stay shared; this file supplies WebGPU resources
// and the measured spectral history expected by those shaders.
const BANDS = 48;
const ROWS = 64;
const CURRENT = BANDS * ROWS;
const STEP = 0.1;
const DOTS = 50 * 50 * 20;
const POINTS = 12_000;
const SHADERS = [
  '../src/params.wgsl', '../src/raster.wgsl',
  '../src/raster/constellation.wgsl', '../src/raster/attractor.wgsl',
  '../src/raster/dot_equaliser.wgsl', '../src/raster/dot_morph.wgsl',
  '../src/raster/disco_silk.wgsl',
];

const clamp = (value, min, max) => Math.max(min, Math.min(max, value));

async function loadShader(path) {
  const url = new URL(path, import.meta.url);
  const revision = new URL(import.meta.url).searchParams.get('v');
  if (revision) url.searchParams.set('v', revision);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Could not load ${path}: ${response.status}`);
  return response.text();
}

// The geometry uses two distinct, synchronized lanes: unfiltered measured
// history for the landscape and two-pole filtered bands for the sculptures.
// Rows enter at fixed physical times rather than on arbitrary rendered frames.
class GeometryHistory {
  constructor() {
    this.measured = new Float32Array(CURRENT);
    this.filtered = new Float32Array(CURRENT + BANDS + 4);
    this.previousMeasured = new Float32Array(BANDS);
    this.previousFiltered = new Float32Array(BANDS);
    this.values = new Float32Array(BANDS);
    this.first = new Float64Array(BANDS);
    this.second = new Float64Array(BANDS);
    this.reset();
  }

  reset() {
    this.measured.fill(0);
    this.filtered.fill(0);
    this.previousMeasured.fill(0);
    this.previousFiltered.fill(0);
    this.first.fill(0);
    this.second.fill(0);
    this.head = 0;
    this.elapsed = 0;
    this.initialized = false;
  }

  update(spectrum, delta, fresh) {
    if (!fresh || !spectrum || spectrum.length < BANDS) {
      this.reset();
      return { head: 0, fraction: 0, fresh: 0 };
    }
    const dt = Number.isFinite(delta) ? clamp(delta, 0, 0.25) : 0;
    const values = this.values;
    for (let band = 0; band < BANDS; band++) {
      values[band] = Number.isFinite(spectrum[band]) ? clamp(spectrum[band], 0, 1.5) : 0;
    }
    if (this.initialized && dt > 0) {
      for (let band = 0; band < BANDS; band++) {
        const target = values[band];
        const tau = target >= this.first[band] ? 0.055 : 0.18;
        const step = dt / tau;
        const decay = Math.exp(-step);
        const firstOffset = this.first[band] - target;
        this.second[band] = clamp(target +
          (this.second[band] - target + firstOffset * step) * decay, 0, 1.5);
        this.first[band] = clamp(target + firstOffset * decay, 0, 1.5);
      }
    }
    if (!this.initialized) {
      // Landscape receives its initial observation, but the filtered lane
      // starts at zero to avoid a false musical transient at connection.
      this.measured.set(values, 0);
      this.initialized = true;
    } else if (dt > 0) {
      const total = this.elapsed + dt;
      const rows = Math.floor(total / STEP);
      for (let row = 0; row < rows; row++) {
        const fraction = clamp((STEP - this.elapsed + row * STEP) / dt, 0, 1);
        this.head = (this.head + 1) % ROWS;
        for (let band = 0; band < BANDS; band++) {
          const index = this.head * BANDS + band;
          this.measured[index] = this.previousMeasured[band] +
            (values[band] - this.previousMeasured[band]) * fraction;
          this.filtered[index] = this.previousFiltered[band] +
            (this.second[band] - this.previousFiltered[band]) * fraction;
        }
      }
      this.elapsed = Math.max(0, total - rows * STEP);
    }
    const energy = [0, 0, 0];
    for (let band = 0; band < BANDS; band++) {
      const value = this.second[band];
      this.previousMeasured[band] = values[band];
      this.previousFiltered[band] = value;
      this.filtered[CURRENT + band] = value;
      energy[band < 8 ? 0 : band < 24 ? 1 : 2] += value * value;
    }
    this.filtered[CURRENT + BANDS] = Math.sqrt(energy[0] / 8);
    this.filtered[CURRENT + BANDS + 1] = Math.sqrt(energy[1] / 16);
    this.filtered[CURRENT + BANDS + 2] = Math.sqrt(energy[2] / 24);
    return { head: this.head, fraction: this.elapsed / STEP, fresh: 1 };
  }
}

// Match src/attractor.rs: three actual RK4 trajectories, generated once when
// initializing this adapter and never integrated in the render loop.
function attractorSamples() {
  const flows = [
    ([x, y, z]) => [10 * (y - x), x * (28 - z) - y, x * y - (8 / 3) * z],
    ([x, y, z]) => [-y - z, x + 0.2 * y, 0.2 + z * (x - 5.7)],
    ([x, y, z]) => [Math.sin(y) - 0.18 * x, Math.sin(z) - 0.18 * y, Math.sin(x) - 0.18 * z],
  ];
  const initial = [[0.1, 0, 0], [0.1, 0, 0], [0.17, 0.03, -0.11]];
  const steps = [0.005, 0.01, 0.025];
  const burns = [8000, 20000, 12000];
  const strides = [3, 4, 3];
  const output = new Float32Array(POINTS * 9);
  for (let system = 0; system < 3; system++) {
    const flow = flows[system];
    const dt = steps[system];
    let state = initial[system];
    const add = (a, b, s) => a.map((v, i) => v + b[i] * s);
    const advance = () => {
      const a = flow(state);
      const b = flow(add(state, a, dt * 0.5));
      const c = flow(add(state, b, dt * 0.5));
      const d = flow(add(state, c, dt));
      state = state.map((v, i) => v + (a[i] + 2 * b[i] + 2 * c[i] + d[i]) * (dt / 6));
    };
    for (let i = 0; i < burns[system]; i++) advance();
    const samples = new Float64Array(POINTS * 3);
    const centre = [0, 0, 0];
    for (let i = 0; i < POINTS; i++) {
      for (let step = 0; step < strides[system]; step++) advance();
      for (let axis = 0; axis < 3; axis++) {
        samples[i * 3 + axis] = state[axis];
        centre[axis] += state[axis] / POINTS;
      }
    }
    let radius = 0;
    for (let i = 0; i < POINTS; i++) {
      radius = Math.max(radius, Math.hypot(...centre.map((c, axis) => samples[i * 3 + axis] - c)));
    }
    const scale = 1.2 / Math.max(radius, Number.EPSILON);
    for (let i = 0; i < POINTS; i++) {
      for (let axis = 0; axis < 3; axis++) {
        output[i * 9 + system * 3 + axis] = (samples[i * 3 + axis] - centre[axis]) * scale;
      }
    }
  }
  return output;
}

/** All five native geometry worlds, including the paired dot-world morph. */
export async function createGeometryPass({ device, uniformBuffer, rasterTexture }) {
  const code = (await Promise.all(SHADERS.map(loadShader))).join('\n');
  const shader = device.createShaderModule({ label: 'native geometry worlds', code });
  const compilation = await shader.getCompilationInfo();
  const errors = compilation.messages.filter(message => message.type === 'error');
  if (errors.length) {
    throw new Error(`Geometry shader: ${errors.map(e => `${e.lineNum}:${e.linePos} ${e.message}`).join('; ')}`);
  }
  const history = new GeometryHistory();
  const measured = device.createBuffer({ size: history.measured.byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
  const filtered = device.createBuffer({ size: history.filtered.byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST });
  const trajectoryData = attractorSamples();
  const trajectory = device.createBuffer({ size: trajectoryData.byteLength,
    usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST });
  device.queue.writeBuffer(trajectory, 0, trajectoryData);
  const layout = device.createBindGroupLayout({ entries: [
    { binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT, buffer: { type: 'uniform' } },
    { binding: 1, visibility: GPUShaderStage.VERTEX, buffer: { type: 'read-only-storage' } },
    { binding: 2, visibility: GPUShaderStage.VERTEX, buffer: { type: 'read-only-storage' } },
  ] });
  const group = device.createBindGroup({ layout, entries: [
    { binding: 0, resource: { buffer: uniformBuffer } },
    { binding: 1, resource: { buffer: measured } },
    { binding: 2, resource: { buffer: filtered } },
  ] });
  const pipelineLayout = device.createPipelineLayout({ bindGroupLayouts: [layout] });
  const additive = {
    color: { operation: 'add', srcFactor: 'one', dstFactor: 'one' },
    alpha: { operation: 'add', srcFactor: 'one', dstFactor: 'one-minus-src-alpha' },
  };
  const alpha = {
    color: { operation: 'add', srcFactor: 'src-alpha', dstFactor: 'one-minus-src-alpha' },
    alpha: { operation: 'add', srcFactor: 'one', dstFactor: 'one-minus-src-alpha' },
  };
  const specs = [
    { id: 4, vertex: 'vs_constellation', fragment: 'fs_raster', layer: 0, count: 6, instances: 12000 },
    { id: 5, vertex: 'vs_attractor', fragment: 'fs_raster', layer: 1, count: 6, instances: 11999 },
    { id: 6, vertex: 'vs_landscape', fragment: 'fs_raster', layer: 2, count: 47 * 63 * 6, instances: 1, depth: true },
    { id: 12, vertex: 'vs_disco_silk', fragment: 'fs_disco_silk', layer: 4, count: 55296, instances: 1, depth: true, silk: true },
    { id: 'morph', vertex: 'vs_dot_morph', fragment: 'fs_raster', count: 6, instances: DOTS },
    { id: 7, vertex: 'vs_dot_equaliser', fragment: 'fs_raster', layer: 3, count: 6, instances: DOTS },
  ];
  for (const spec of specs) {
    const buffers = spec.id === 5 ? [0, 3].map(base => ({
      arrayStride: 36, stepMode: 'instance', attributes: [0, 1, 2].map(index => ({
        shaderLocation: base + index, offset: index * 12, format: 'float32x3',
      })),
    })) : [];
    spec.pipeline = await device.createRenderPipelineAsync({
      label: `native ${spec.vertex}`,
      layout: pipelineLayout,
      vertex: { module: shader, entryPoint: spec.vertex, buffers },
      fragment: { module: shader, entryPoint: spec.fragment,
        targets: [{ format: 'rgba16float', ...(spec.id === 6 ? {} : { blend: spec.silk ? alpha : additive }) }] },
      primitive: { topology: 'triangle-list', cullMode: 'none' },
      depthStencil: { format: 'depth32float', depthWriteEnabled: Boolean(spec.depth),
        depthCompare: spec.depth ? 'less' : 'always' },
    });
  }
  const byId = new Map(specs.map(spec => [spec.id, spec]));
  let target = rasterTexture;
  let depth = null;
  let targetWidth = 0;
  let targetHeight = 0;
  function setTexture(texture, width, height) {
    target = texture;
    if (width === targetWidth && height === targetHeight && depth) return;
    depth?.destroy();
    targetWidth = width;
    targetHeight = height;
    depth = device.createTexture({ size: [width, height], format: 'depth32float',
      usage: GPUTextureUsage.RENDER_ATTACHMENT });
  }
  function draw(encoder, spec, layer = spec.layer) {
    const view = target.createView({ dimension: '2d', baseArrayLayer: layer, arrayLayerCount: 1 });
    const pass = encoder.beginRenderPass({
      label: `native ${spec.vertex} geometry`,
      colorAttachments: [{ view, clearValue: [0, 0, 0, 0], loadOp: 'clear', storeOp: 'store' }],
      depthStencilAttachment: { view: depth.createView(), depthClearValue: 1,
        depthLoadOp: 'clear', depthStoreOp: 'discard' },
    });
    pass.setPipeline(spec.pipeline);
    pass.setBindGroup(0, group);
    if (spec.id === 5) {
      pass.setVertexBuffer(0, trajectory, 0);
      pass.setVertexBuffer(1, trajectory, 36);
    }
    pass.draw(spec.count, spec.instances);
    pass.end();
  }
  return {
    setTexture,
    update(spectrum, dt, fresh) {
      const state = history.update(spectrum, dt, fresh);
      device.queue.writeBuffer(measured, 0, history.measured);
      device.queue.writeBuffer(filtered, 0, history.filtered);
      return state;
    },
    render(encoder, from, destination, melt) {
      if (!depth) throw new Error('Geometry target size was not initialized');
      const paired = melt < 1 && ((from === 4 && destination === 7) || (from === 7 && destination === 4));
      if (paired) {
        // Exact first endpoint uses the original source geometry; subsequent
        // frames move corresponding vertices in the destination layer.
        const pipeline = melt === 0 ? byId.get(from) : byId.get('morph');
        draw(encoder, pipeline, destination === 4 ? 0 : 3);
        return;
      }
      for (const id of new Set(melt < 1 ? [from, destination] : [destination])) {
        const spec = byId.get(id);
        if (spec) draw(encoder, spec);
      }
    },
    destroy() {
      depth?.destroy();
      measured.destroy();
      filtered.destroy();
      trajectory.destroy();
    },
  };
}
