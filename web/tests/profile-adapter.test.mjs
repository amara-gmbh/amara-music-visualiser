import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { defaultProfile, validateProfile } from '../profiles.js';
import { snapshotProfile } from '../profile-adapter.js';
import { createLookJourney } from '../choreography.js';

const html = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
for (const id of ['pace', 'intensity', 'detail', 'bass-gain', 'mids-gain', 'highs-gain']) {
  const input = html.match(new RegExp(`<input id="${id}"[^>]*>`))?.[0];
  assert.ok(input, `missing ${id} slider`);
  assert.match(input, /step="any"/, `${id} must retain valid native profile precision`);
}

const base = defaultProfile('Original');
base.visual.motion = 0.56;
base.visual.intensity = 0.92;
base.mappings.mids_to_geometry = 0.72;
base.visual.background = 'OceanJade'; // Not applied in browser; must survive.
const value = n => ({ value: String(n) });
const controls = { pace: value(0.56), intensity: value(0.92), detail: value(0.8),
  graphics: { value: 'Low' }, bass: value(1), mids: value(0.72), highs: value(0.9) };
const lookTargets = Array.from({ length: 17 }, (_, id) => id % 10);
const input = { base, name: 'Copy', selection: '12', current: 3, next: 8, lookTargets, controls };
const manual = validateProfile(snapshotProfile(input));
assert.equal(manual.visual.scene, 12, 'queued manual choice is the saved world');
assert.equal(manual.visual.look, lookTargets[12]);
assert.equal(manual.visual.motion, 0.56);
assert.equal(manual.visual.intensity, 0.92);
assert.equal(manual.visual.graphics_quality, 'Low');
assert.equal(manual.mappings.mids_to_geometry, 0.72);
assert.equal(manual.visual.background, 'OceanJade');
assert.equal(base.name, 'Original', 'snapshot must not mutate the loaded profile');
assert.equal(snapshotProfile({ ...input, selection: 'auto' }).visual.scene, 8);
assert.equal(snapshotProfile({ ...input, selection: 'auto', next: null }).visual.scene, 3);
const lane = createLookJourney(Array(17).fill(0));
lane.request(12, 2, 0);
lane.request(12, 7, 2);
const pendingTargets = Array.from({ length: 17 }, (_, id) => {
  const state = lane.state(id, 2);
  return state.queued ?? state.to;
});
assert.equal(snapshotProfile({ ...input, lookTargets: pendingTargets }).visual.look, 7,
  'saved profile keeps the explicitly selected look even if its morph is queued');
console.log('Profile snapshot precision and queued selection passed');
