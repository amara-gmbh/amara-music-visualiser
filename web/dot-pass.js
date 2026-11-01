// The browser runs the same Dot Equaliser geometry and material as the Mac.
// The shader sources below are loaded directly from the native renderer; only
// the WebGPU plumbing and live audio-history adapter are browser-specific.
const BANDS = 48;
const ROWS = 64;
const CURRENT = BANDS * ROWS;
const STEP = 0.1;
const DOTS = 50 * 50 * 20;
const SHADERS = [
  "../src/params.wgsl",
  "../src/raster.wgsl",
  "../src/raster/constellation.wgsl",
  "../src/raster/attractor.wgsl",
  "../src/raster/dot_equaliser.wgsl",
  "../src/raster/dot_morph.wgsl",
  "../src/raster/disco_silk.wgsl",
];

async function loadShader(path) {
  const url = new URL(path, import.meta.url);
  const revision = new URL(import.meta.url).searchParams.get('v');
  if (revision) url.searchParams.set('v', revision);
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Could not load ${path}: ${response.status}`);
  return response.text();
}

// This mirrors the native DotEqualiserHistory and SpectralHistory: two poles
// smooth measured FFT levels, while recorded rows remain fixed in physical
// time. No beat, synthetic waveform or normalization invents sound.
class DotHistory {
  constructor() {
    this.samples = new Float32Array(CURRENT + BANDS + 4);
    this.first = new Float64Array(BANDS);
    this.second = new Float64Array(BANDS);
    this.previous = new Float32Array(BANDS);
    this.reset();
  }

  reset() {
    this.samples.fill(0);
    this.first.fill(0);
    this.second.fill(0);
    this.previous.fill(0);
    this.head = 0;
    this.elapsed = 0;
    this.initialized = false;
  }

  update(spectrum, delta, fresh) {
    if (!fresh || !spectrum || spectrum.length < BANDS) {
      this.reset();
      return { head: 0, fraction: 0, fresh: 0 };
    }

    const dt = Number.isFinite(delta) ? Math.min(Math.max(delta, 0), 0.25) : 0;
    if (this.initialized && dt > 0) {
      for (let band = 0; band < BANDS; band++) {
        const measured = spectrum[band];
        const target = Number.isFinite(measured) ? Math.min(Math.max(measured, 0), 1.5) : 0;
        const tau = target >= this.first[band] ? 0.055 : 0.18;
        const step = dt / tau;
        const decay = Math.exp(-step);
        const firstOffset = this.first[band] - target;
        this.second[band] = Math.min(Math.max(target +
          (this.second[band] - target + firstOffset * step) * decay, 0), 1.5);
        this.first[band] = Math.min(Math.max(target + firstOffset * decay, 0), 1.5);
      }
    }

    if (!this.initialized) {
      this.initialized = true;
    } else if (dt > 0) {
      const total = this.elapsed + dt;
      const rows = Math.floor(total / STEP);
      for (let row = 0; row < rows; row++) {
        const fraction = Math.min(Math.max((STEP - this.elapsed + row * STEP) / dt, 0), 1);
        this.head = (this.head + 1) % ROWS;
        for (let band = 0; band < BANDS; band++) {
          this.samples[this.head * BANDS + band] = this.previous[band] +
            (this.second[band] - this.previous[band]) * fraction;
        }
      }
      this.elapsed = Math.max(0, total - rows * STEP);
    }

    for (let band = 0; band < BANDS; band++) {
      this.previous[band] = this.second[band];
      this.samples[CURRENT + band] = this.second[band];
    }
    return { head: this.head, fraction: this.elapsed / STEP, fresh: 1 };
  }
}

/** Create the native 50,000-point Dot Equaliser pass in raster array layer 3. */
export async function createDotPass({ device, uniformBuffer, rasterTexture }) {
  const code = (await Promise.all(SHADERS.map(loadShader))).join("\n");
  const shader = device.createShaderModule({ label: "native dot geometry", code });
  const compilation = await shader.getCompilationInfo();
  const errors = compilation.messages.filter((message) => message.type === "error");
  if (errors.length) {
    throw new Error(`Dot Equaliser shader: ${errors.map((e) => `${e.lineNum}:${e.linePos} ${e.message}`).join("; ")}`);
  }

  const history = new DotHistory();
  const geometryHistory = device.createBuffer({
    label: "browser spectral history",
    size: ROWS * BANDS * 4,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
  });
  const dotHistory = device.createBuffer({
    label: "browser dot spectral history",
    size: history.samples.byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
  });
  const bindingLayout = device.createBindGroupLayout({
    entries: [
      { binding: 0, visibility: GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT,
        buffer: { type: "uniform" } },
      { binding: 1, visibility: GPUShaderStage.VERTEX, buffer: { type: "read-only-storage" } },
      { binding: 2, visibility: GPUShaderStage.VERTEX, buffer: { type: "read-only-storage" } },
    ],
  });
  const bindGroup = device.createBindGroup({
    layout: bindingLayout,
    entries: [
      { binding: 0, resource: { buffer: uniformBuffer } },
      { binding: 1, resource: { buffer: geometryHistory } },
      { binding: 2, resource: { buffer: dotHistory } },
    ],
  });
  const pipeline = await device.createRenderPipelineAsync({
    label: "native 50000-dot equaliser",
    layout: device.createPipelineLayout({ bindGroupLayouts: [bindingLayout] }),
    vertex: { module: shader, entryPoint: "vs_dot_equaliser" },
    fragment: {
      module: shader,
      entryPoint: "fs_raster",
      targets: [{
        format: "rgba16float",
        blend: {
          color: { operation: "add", srcFactor: "one", dstFactor: "one" },
          alpha: { operation: "add", srcFactor: "one", dstFactor: "one-minus-src-alpha" },
        },
      }],
    },
    primitive: { topology: "triangle-list", cullMode: "none" },
  });

  let target = rasterTexture;
  return {
    setTexture(texture) { target = texture; },
    update(spectrum, dt, fresh) {
      // The caller updates every rendered frame even when the dot world is
      // hidden. A slow GPU frame must not erase the smoothing poles: doing
      // so at intervals over 500 ms can keep the equaliser permanently dark.
      // Losing the audio source (fresh=false) already clears stale history.
      const state = history.update(spectrum, dt, fresh);
      device.queue.writeBuffer(dotHistory, 0, history.samples);
      return state;
    },
    render(encoder) {
      const view = target.createView({ dimension: "2d", baseArrayLayer: 3, arrayLayerCount: 1 });
      const pass = encoder.beginRenderPass({
        label: "native Dot Equaliser geometry",
        colorAttachments: [{ view, clearValue: [0, 0, 0, 0], loadOp: "clear", storeOp: "store" }],
      });
      pass.setPipeline(pipeline);
      pass.setBindGroup(0, bindGroup);
      pass.draw(6, DOTS);
      pass.end();
    },
    destroy() {
      geometryHistory.destroy();
      dotHistory.destroy();
    },
  };
}
