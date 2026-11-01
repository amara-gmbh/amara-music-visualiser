// Node-only checks of adapter routing and audio history; actual WebGPU shader
// validation and visual quality must also be tested in a browser.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createGeometryPass } from '../geometry-pass.js';

globalThis.fetch = async url => ({ ok: true, text: () => readFile(url, 'utf8') });
globalThis.GPUBufferUsage = { STORAGE: 1, COPY_DST: 2, VERTEX: 4 };
globalThis.GPUTextureUsage = { RENDER_ATTACHMENT: 1 };
globalThis.GPUShaderStage = { VERTEX: 1, FRAGMENT: 2 };

const writes = [];
const pipelines = [];
const passes = [];
const device = {
  queue: { writeBuffer: (buffer, offset, data) => writes.push({ buffer, data: Array.from(data) }) },
  createShaderModule: ({ code }) => ({ code, getCompilationInfo: async () => ({ messages: [] }) }),
  createBuffer: spec => ({ ...spec, destroy() {} }),
  createBindGroupLayout: spec => spec,
  createBindGroup: spec => spec,
  createPipelineLayout: spec => spec,
  createRenderPipelineAsync: async spec => (pipelines.push(spec), spec),
  createTexture: spec => ({ spec, createView: () => ({}), destroy() {} }),
};
const raster = { createView: spec => spec };
const pass = await createGeometryPass({ device, uniformBuffer: {}, rasterTexture: raster });
assert.deepEqual(pipelines.map(p => p.vertex.entryPoint), [
  'vs_constellation', 'vs_attractor', 'vs_landscape', 'vs_disco_silk',
  'vs_dot_morph', 'vs_dot_equaliser',
]);
assert.equal(writes[0].data.length, 12000 * 9);
assert.ok(writes[0].data.every(Number.isFinite));
// Each independently generated flow is centred and fitted to radius 1.2.
for (let system = 0; system < 3; system++) {
  const centre = [0, 0, 0];
  let radius = 0;
  for (let point = 0; point < 12000; point++) {
    const coordinates = writes[0].data.slice(point * 9 + system * 3, point * 9 + system * 3 + 3);
    for (let axis = 0; axis < 3; axis++) centre[axis] += coordinates[axis] / 12000;
    radius = Math.max(radius, Math.hypot(...coordinates));
  }
  assert.ok(centre.every(value => Math.abs(value) < 0.000002));
  assert.ok(Math.abs(radius - 1.2) < 0.000002);
}
pass.setTexture(raster, 640, 480);

const audio = new Float32Array(48);
audio[0] = 0.8;
audio[16] = 0.6;
audio[32] = 0.4;
pass.update(audio, 0.01, true);
const state = pass.update(audio, 0.11, true);
assert.equal(state.head, 1);
assert.ok(state.fraction >= 0 && state.fraction < 1);
const measured = writes.at(-2).data;
const filtered = writes.at(-1).data;
assert.ok(measured[48] > 0.7);
assert.equal(measured[49], 0, 'silent neighbouring bins stay silent');
assert.ok(filtered[3072] > 0);
assert.ok(filtered[3120] > 0 && filtered[3121] > 0 && filtered[3122] > 0);

const encoder = { beginRenderPass: descriptor => {
  const record = { layer: descriptor.colorAttachments[0].view.baseArrayLayer };
  passes.push(record);
  return {
    setPipeline: pipeline => { record.pipeline = pipeline.vertex.entryPoint; },
    setBindGroup() {}, setVertexBuffer() {},
    draw: (vertices, instances) => { record.vertices = vertices; record.instances = instances; },
    end() {},
  };
} };
pass.render(encoder, 4, 7, 0);
assert.deepEqual(passes.at(-1), { layer: 3, pipeline: 'vs_constellation', vertices: 6, instances: 12000 });
pass.render(encoder, 4, 7, 0.4);
assert.deepEqual(passes.at(-1), { layer: 3, pipeline: 'vs_dot_morph', vertices: 6, instances: 50000 });
pass.render(encoder, 7, 4, 0);
assert.deepEqual(passes.at(-1), { layer: 0, pipeline: 'vs_dot_equaliser', vertices: 6, instances: 50000 });
pass.render(encoder, 5, 12, 0.5);
assert.deepEqual(passes.slice(-2), [
  { layer: 1, pipeline: 'vs_attractor', vertices: 6, instances: 11999 },
  { layer: 4, pipeline: 'vs_disco_silk', vertices: 55296, instances: 1 },
]);
pass.render(encoder, 6, 6, 1);
assert.deepEqual(passes.at(-1), { layer: 2, pipeline: 'vs_landscape', vertices: 47 * 63 * 6, instances: 1 });
pass.update(audio, 0.1, false);
assert.ok(writes.at(-1).data.every(value => value === 0));
assert.ok(writes.at(-2).data.every(value => value === 0));
pass.destroy();
console.log('Geometry adapter checks passed.');
