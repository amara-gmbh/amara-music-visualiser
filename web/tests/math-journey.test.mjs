import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createMathJourney } from '../choreography.js';

const maps = { bass: 1, mids: 1, treble: 1, beat: 1 };
const music = { bass: 0.9, mids: 0.7, treble: 0.6, beat: 0.7 };
const journey = createMathJourney(() => 0.35);
let state = journey.update(music, maps, 'InversionLace', 0);
assert.deepEqual([state.from, state.to, state.blend], [0, 0, 1], 'paused updates must not alter state');
for (let i = 0; i < 30; i++) state = journey.update(music, maps, 'InversionLace', 0.1);
assert.deepEqual([state.from, state.to], [0, 1]);
assert.ok(Math.abs(state.blend - 0.5) < 1e-9, 'six-second morph must be halfway at three seconds');
// Requesting a new family during a morph cannot change its endpoints.
state = journey.update(music, maps, 'VortexWeave', 0.1);
assert.deepEqual([state.from, state.to], [0, 1]);
for (let i = 0; i < 29; i++) state = journey.update(music, maps, 'VortexWeave', 0.1);
assert.deepEqual([state.from, state.to], [1, 3], 'the queued family starts only at the endpoint');
assert.ok(state.blend < 1e-8);

const auto = createMathJourney(() => 0.35);
const chosen = [0];
let previous = auto.update({}, maps, 'Journey', 0);
for (let i = 0; i < 3000; i++) {
  const next = auto.update(music, maps, 'Journey', 0.1);
  assert.ok(next.from >= 0 && next.from < 4 && next.to >= 0 && next.to < 4);
  assert.ok(Number.isFinite(next.phase) && next.phase >= previous.phase);
  if (next.to !== previous.to) {
    assert.equal(previous.blend, 1, 'family cannot change mid-morph');
    assert.notEqual(next.to, chosen.at(-1));
    chosen.push(next.to);
  } else if (next.from === previous.from && next.to === previous.to) {
    assert.ok(next.blend >= previous.blend, 'morph interpolation must not reverse');
  }
  previous = next;
}
assert.ok(chosen.length >= 8);
assert.equal(new Set(chosen.slice(0, 4)).size, 4, 'first family cycle covers all four');

function firstChange(features) {
  const value = createMathJourney(() => 0.35);
  for (let i = 1; i <= 230; i++) if (value.update(features, maps, 'Journey', 0.1).to !== 0) return i / 10;
  return Infinity;
}
assert.ok(Math.abs(firstChange({}) - 22) < 0.11, 'silent hold should be about 22 seconds');
const loudHold = firstChange(music);
assert.ok(loudHold >= 12 && loudHold < 22, 'measured music shortens but bounds the held phrase');

const source = readFileSync(new URL('../main.js', import.meta.url), 'utf8');
assert.match(source, /uniforms\.set\(\[math\.from, math\.to, math\.blend, math\.phase\], 36\)/);
assert.doesNotMatch(source, /uniforms\.set\(\[looks\[1\], lookTargets\[1\], blendLook\(1\)/);
console.log('Independent fractal-family journey passed');
