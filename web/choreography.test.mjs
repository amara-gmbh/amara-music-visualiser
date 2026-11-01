import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createWorldBag, createMandelbrotJourney, createSelectionGate, createLookJourney } from './choreography.js';
import { LOOK_NAMES } from './look-names.js';

let seed = 1321;
const random = () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed / 2 ** 32; };
const bag = createWorldBag(random);
const worlds = Array.from({ length: 17 }, (_, i) => i);
let current = 4;
// The initial current world plus the first 16 picks cover every world; each
// following full bag covers all worlds and avoids a repeated boundary.
let first = [current];
for (let i = 0; i < 16; i++) {
  const next = bag.next(worlds, current);
  assert.notEqual(next, current);
  first.push(next); current = next;
}
assert.equal(new Set(first).size, 17);
for (let cycle = 0; cycle < 30; cycle++) {
  const seen = [];
  for (let i = 0; i < 17; i++) {
    const next = bag.next(worlds, current);
    assert.notEqual(next, current);
    seen.push(next); current = next;
  }
  assert.equal(new Set(seen).size, 17);
}
assert.equal(createWorldBag(random).next([4], 4), null);
assert.equal(LOOK_NAMES.length, 17);
assert.ok(LOOK_NAMES.every(names => names.length === 10));
const nativeSource = readFileSync(new URL('../src/visual.rs', import.meta.url), 'utf8');
const nativeLooks = nativeSource.split('pub const LOOKS:')[1].split('const HISTORY_FORMAT:')[0];
const nativeNames = Array.from(nativeLooks.matchAll(/"([^"\n]+)"/g), match => match[1]);
assert.deepEqual(LOOK_NAMES.flat(), nativeNames, 'browser names must match the actual native look indices');

const gate = createSelectionGate();
const firstPending = gate.begin();
gate.cancel(); // User returns to Auto while GPU compilation is pending.
const latest = gate.begin();
assert.equal(gate.current(firstPending), false);
gate.finish(firstPending);
assert.equal(gate.current(latest), true, 'stale completion must not clear latest selection');
gate.finish(latest);
assert.equal(gate.busy(), false);

const looks = createLookJourney([0, 4]);
looks.request(0, 2, 0);
assert.deepEqual(looks.state(0, 0), { from: 0, to: 2, blend: 0, queued: null, startedAt: 0 });
looks.request(0, 3, 2);
assert.deepEqual(looks.state(0, 2), { from: 0, to: 2, blend: 2 / 7, queued: 3, startedAt: 0 });
looks.request(0, 5, 3);
assert.equal(looks.state(0, 3).queued, 5, 'latest request replaces only the queued target');
looks.advance(7);
assert.deepEqual(looks.state(0, 7), { from: 2, to: 5, blend: 0, queued: null, startedAt: 7 });
assert.equal(looks.state(1, 7).to, 4, 'changing one world must not alter another');
looks.request(0, 1, 8);
looks.request(0, 5, 8.1);
assert.equal(looks.state(0, 8.1).queued, null, 'reselecting the active target cancels a queued change');
looks.request(0, 1, 8.2);
looks.cancelQueued();
assert.equal(looks.state(0, 8.2).queued, null, 'returning to Auto clears pending manual choices');
assert.throws(() => looks.request(0, 10, 9));

const journey = createMandelbrotJourney();
let previous = 0;
let regions = 0;
let falling = false;
let risingAgain = false;
for (let frame = 0; frame < 60 * 160; frame++) {
  const state = journey.advance(1 / 60, 4);
  assert.ok(Number.isFinite(state.age) && state.age >= 0);
  assert.ok(Math.abs(state.age - previous) < 0.06, 'camera age jumps between frames');
  if (state.age < previous - 1e-5) falling = true;
  if (falling && state.age > previous + 1e-5 && state.age < 10) risingAgain = true;
  if (state.newRegion) {
    assert.ok(state.age < 0.06, 'region should change only at broad framing');
    regions++;
  }
  previous = state.age;
}
assert.ok(falling && risingAgain && regions >= 2, 'journey must pull back and unfold again');
journey.reset();
assert.ok(journey.advance(1 / 60, 4).age < 0.1);
console.log('World bag and continuous Mandelbrot journey passed');
