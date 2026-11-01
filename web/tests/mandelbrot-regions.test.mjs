import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createWorldBag, createLookJourney } from '../choreography.js';
import { createVariationMode } from '../variation-mode.js';

const shader = readFileSync(new URL('../../src/worlds/mandelbrot.wgsl', import.meta.url), 'utf8');
const destinations = shader.split('let destinations = array<vec4<f32>, 10>(')[1].split(');')[0];
const recipes = [...destinations.matchAll(/vec4<f32>\(([-.\d]+),\s*([-.\d]+),\s*([-.\d]+),\s*([-.\d]+)\)/g)]
  .map(match => match.slice(1).map(Number));
assert.equal(recipes.length, 10);

// At the deepest excursion, sample the actual complex plane around each
// recipe. This is finite-iteration evidence, not an interior-membership proof.
for (const [index, [cx, cy, base, depth]] of recipes.entries()) {
  const span = base * Math.exp(-depth);
  let unresolved = 0;
  const width = 25, height = 17;
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    const real = cx + (x / (width - 1) * 2 - 1) * span * 1.5;
    const imaginary = cy + (y / (height - 1) * 2 - 1) * span;
    let zr = 0, zi = 0, escaped = false;
    for (let i = 0; i < 160; i++) {
      [zr, zi] = [zr * zr - zi * zi + real, 2 * zr * zi + imaginary];
      if (zr * zr + zi * zi > 4) { escaped = true; break; }
    }
    if (!escaped) unresolved++;
  }
  const fraction = unresolved / (width * height);
  assert.ok(fraction > 0.015 && fraction < 0.9,
    `region ${index}: deepest view needs a visible boundary; unresolved fraction ${fraction}`);
}

// Exercise every pair, not just destinations: a straight line between good
// endpoints can cross a solid lobe. Mirror the shader's eased outer arc and
// wide-pullback camera at several physical times and transition positions.
assert.match(shader, /vec2<f32>\(0\.0, 0\.48 \* bridge\)/);
assert.match(shader, /clamp\(view\.span \* 3\.0, 0\.24, 0\.65\)/);
function view(index, time) {
  const [x, y, base, depth] = recipes[index];
  const excursion = 0.5 - 0.5 * Math.cos(time * 0.075);
  const span = base * Math.exp(-depth * excursion);
  const orbit = time * 0.027 + index * 0.47;
  return { x: x + (index === 2 ? (1 - excursion) * 0.04 : 0) + Math.sin(orbit * 0.71) * span * 0.1,
    y: y + (index === 9 ? (1 - excursion) * 0.016 : 0) + Math.cos(orbit * 0.83) * span * 0.1, span };
}
for (let a = 0; a < 10; a++) for (let b = a + 1; b < 10; b++) {
  for (const time of [0, 42]) for (const blend of [0.2, 0.4, 0.6, 0.8]) {
    const p = view(a, time), q = view(b, time);
    const t = blend * blend * (3 - 2 * blend), bridge = Math.sin(t * Math.PI);
    const cx = p.x * (1 - t) + q.x * t;
    const cy = p.y * (1 - t) + q.y * t + 0.48 * bridge;
    const mixedSpan = Math.exp(Math.log(p.span) * (1 - t) + Math.log(q.span) * t)
      + Math.hypot(p.x - q.x, p.y - q.y) * bridge * 0.65;
    const span = Math.min(0.65, Math.max(0.24, mixedSpan * 3));
    let unresolved = 0;
    for (let y = 0; y < 11; y++) for (let x = 0; x < 17; x++) {
      const real = cx + (x / 16 * 2 - 1) * span * 1.5;
      const imaginary = cy + (y / 10 * 2 - 1) * span;
      let zr = 0, zi = 0, escaped = false;
      for (let i = 0; i < 100; i++) {
        [zr, zi] = [zr * zr - zi * zi + real, 2 * zr * zi + imaginary];
        if (zr * zr + zi * zi > 4) { escaped = true; break; }
      }
      if (!escaped) unresolved++;
    }
    assert.ok(unresolved / 187 < 0.75, `transition ${a} → ${b} crosses a solid interior`);
  }
}

let seed = 271828;
const random = () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed / 2 ** 32; };
const bag = createWorldBag(random);
const choices = Array.from({ length: 10 }, (_, i) => i);
let previous = 6;
for (let cycle = 0; cycle < 10; cycle++) {
  const seen = cycle === 0 ? [previous] : [];
  while (seen.length < 10) {
    const next = bag.next(choices, previous);
    assert.notEqual(next, previous);
    seen.push(next); previous = next;
  }
  assert.equal(new Set(seen).size, 10, 'every region gets a turn before repeats');
}

// A region requested at wide pullback starts at its existing picture. A
// profile/manual variation keeps its selection; Auto is free to rotate it.
const lane = createLookJourney(Array(11).fill(4));
const mode = createVariationMode();
mode.choose('manual', 0);
assert.equal(mode.allowsMandelbrotRegion('10'), false);
assert.equal(lane.state(10, 0).to, 4);
mode.choose('auto', 1);
assert.equal(mode.allowsMandelbrotRegion('10'), true);
lane.request(10, 8, 1);
assert.deepEqual([lane.state(10, 1).from, lane.state(10, 1).blend], [4, 0]);
assert.equal(lane.state(10, 8).blend, 1);
console.log('Mandelbrot boundary samples, fair region sequence and variation semantics passed');
