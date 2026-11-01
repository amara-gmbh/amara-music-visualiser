import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { AUTO_WORLD_TRANSITION_SECONDS, createSelectionEventGuard, manualPipelineMasks,
  manualSwitchPlan, manualTransitionAnchor, wallClockProgress } from '../world-transition.js';

assert.equal(AUTO_WORLD_TRANSITION_SECONDS, 7.5,
  'automatic journeys keep the established cinematic transition');
assert.equal(manualTransitionAnchor(2, null, 1), 2);
assert.equal(manualTransitionAnchor(2, 8, 0.49), 2,
  'an early interrupted transition settles on its source');
assert.equal(manualTransitionAnchor(2, 8, 0.5), 8,
  'a late interrupted transition settles on its destination');
assert.deepEqual(manualSwitchPlan(2, 8, 0.2, 14), { anchor: 2, target: 14, duration: 0 });
assert.deepEqual(manualSwitchPlan(2, 8, 0.8, 14), { anchor: 8, target: 14, duration: 0 });
assert.deepEqual(manualPipelineMasks([0, 1, 4, 7, 8], id => [0, 1, 8].includes(id) ? 1 << id : 0),
  [1, 2, 0, 256], 'manual warmup compiles standalone masks, not source/destination pairs');
assert.equal(wallClockProgress(10, 17.5, AUTO_WORLD_TRANSITION_SECONDS), 1,
  'transition timing follows wall time even when very few frames render');
assert.equal(wallClockProgress(10, 11.875, AUTO_WORLD_TRANSITION_SECONDS), 0.25);
const events = createSelectionEventGuard();
assert.equal(events.handle('input', '14'), true);
assert.equal(events.handle('change', '14'), false, 'paired native change must not restart an input selection');
assert.equal(events.handle('change', '9'), true, 'change-only browser paths remain supported');
const mainSource = await readFile(new URL('../main.js', import.meta.url), 'utf8');
assert.match(mainSource, /selector\.addEventListener\('input', handleWorldSelection\)/u);
assert.match(mainSource, /selector\.addEventListener\('change', handleWorldSelection\)/u);
assert.match(mainSource, /buildPipeline\(worldMask\(id\)\)/u,
  'manual selection must use a standalone destination pipeline');
assert.match(mainSource, /wallElapsed - journeyWorldStartWall/u,
  'journey dwell must use wall time rather than capped animation time');

console.log('Immediate manual world switch and automatic transition checks passed');
