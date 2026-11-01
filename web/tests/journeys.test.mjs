import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { JOURNEYS, createJourneyController, createJourneySelectionGuard, describeJourney, journeyDefinition, journeyDwell, journeyShouldAdvance, journeyWorlds } from '../journeys.js';

assert.deepEqual(Object.keys(JOURNEYS), ['Surprise', 'Dreamy', 'NightDrive', 'DeepFocus', 'Dancefloor', 'Aquarelle']);
const html = await readFile(new URL('../index.html', import.meta.url), 'utf8');
for (const [value, label] of [['Surprise', 'Surprise Me'], ['Dreamy', 'Dreamy'], ['NightDrive', 'Night Drive'],
  ['DeepFocus', 'Deep Focus'], ['Dancefloor', 'Dancefloor'], ['Aquarelle', 'Aquarelle']]) {
  assert.match(html, new RegExp(`<option value="${value}"[^>]*>${label}<\\/option>`));
}
assert.match(html, /id="journey-status"[^>]*role="status"[^>]*aria-live="polite"/);
assert.deepEqual(JOURNEYS.Dreamy.worlds, [1, 2, 6, 9, 15, 16]);
assert.deepEqual(JOURNEYS.NightDrive.worlds, [0, 3, 12, 13, 14]);
assert.deepEqual(JOURNEYS.DeepFocus.worlds, [1, 5, 6, 10, 11]);
assert.deepEqual(JOURNEYS.Dancefloor.worlds, [3, 4, 7, 12, 13]);
assert.deepEqual(JOURNEYS.Aquarelle.worlds, [1, 2, 6, 9, 15, 16]);
assert.deepEqual(journeyWorlds('Dreamy', [0, 1, 2, 6, 8, 9, 16]), [1, 2, 6, 9, 16]);
assert.deepEqual(journeyWorlds('Surprise', [0, 4, 16]), [0, 4, 16]);
assert.equal(journeyDwell('Dancefloor', 3), 20);
assert.equal(journeyDwell('Dancefloor', 10), 65, 'Mandelbrot keeps its formation window');

const quiet = { active: true, section: 'quiet', trend: 'steady', event: 'none', drop: 0 };
const steady = { active: true, section: 'medium', trend: 'steady', event: 'none', drop: 0 };
const rising = { active: true, section: 'medium', trend: 'rising', event: 'none', drop: 0 };
const drop = { active: true, section: 'high', trend: 'rising', event: 'drop', drop: 1 };
assert.equal(journeyShouldAdvance({ id: 'Dancefloor', world: 3, age: 19.9, insight: drop }), false,
  'music can never switch before the journey cadence');
assert.equal(journeyShouldAdvance({ id: 'Dancefloor', world: 3, age: 20, insight: quiet }), false,
  'quiet passage delays an already-due switch');
assert.equal(journeyShouldAdvance({ id: 'Dancefloor', world: 3, age: 31.9, insight: quiet }), false);
assert.equal(journeyShouldAdvance({ id: 'Dancefloor', world: 3, age: 32, insight: quiet }), true,
  'quiet delay is bounded');
assert.equal(journeyShouldAdvance({ id: 'Dancefloor', world: 3, age: 20, insight: rising }), false);
assert.equal(journeyShouldAdvance({ id: 'Dancefloor', world: 3, age: 26, insight: rising }), true,
  'buildup wait is bounded');
assert.equal(journeyShouldAdvance({ id: 'Dancefloor', world: 3, age: 20, insight: drop }), true,
  'a confident drop can finish an already-due wait');
assert.equal(journeyShouldAdvance({ id: 'Dancefloor', world: 3, age: 20, insight: steady }), true);

class MemoryStorage {
  constructor() { this.value = new Map(); }
  getItem(key) { return this.value.get(key) ?? null; }
  setItem(key, value) { this.value.set(key, String(value)); }
}
const storage = new MemoryStorage();
const controller = createJourneyController(storage);
assert.equal(controller.state.selected, 'Surprise');
controller.choose('Dreamy');
controller.customize('palette');
assert.equal(controller.state.customized, true);
controller.pause('manual world');
assert.equal(controller.state.selected, 'Manual');
assert.equal(controller.state.reason, 'manual world');
assert.equal(createJourneyController(storage).state.selected, 'Manual', 'pause state stays local');
assert.throws(() => controller.choose('Unknown'), /Unknown journey/);

// Reproduce the real select/change boundary: internal World Auto setup must not
// be mistaken for a user choosing World Auto, while an explicit later change is.
const integrationController = createJourneyController(new MemoryStorage());
const guard = createJourneySelectionGuard();
const journeySelect = new EventTarget();
const worldSelect = new EventTarget();
journeySelect.value = 'Surprise';
worldSelect.value = 'auto';
let integrationStatus = describeJourney(integrationController.state, 16);
let appliedDefaults = null;
worldSelect.addEventListener('change', () => {
  if (guard.consumeAuto(worldSelect.value)) return;
  if (worldSelect.value === 'auto' && !integrationController.state.active) integrationController.choose('Surprise');
});
function handleJourneySelection() {
  const requested = journeySelect.value;
  if (integrationController.state.selected === requested && integrationController.state.active) {
    integrationStatus = describeJourney(integrationController.state, 16);
    return;
  }
  guard.run(() => {
    integrationController.choose(requested);
    const journey = journeyDefinition(requested);
    appliedDefaults = { palette: journey.palette, pace: journey.pace };
    worldSelect.value = 'auto';
    worldSelect.dispatchEvent(new Event('change'));
    integrationStatus = describeJourney(integrationController.state, 16);
  });
}
journeySelect.addEventListener('input', handleJourneySelection);
journeySelect.addEventListener('change', handleJourneySelection);
journeySelect.value = 'NightDrive';
journeySelect.dispatchEvent(new Event('input'));
assert.equal(integrationController.state.selected, 'NightDrive',
  'internal World Auto setup cannot snap Night Drive back to Surprise Me');
assert.match(integrationStatus, /^Night Drive · 5 curated worlds · Cosmic · pace 0\.9/,
  'the visible status comes from the selected controller model');
assert.deepEqual(appliedDefaults, { palette: 'Cosmic', pace: 0.9 });
assert.deepEqual(journeyWorlds(integrationController.state.selected, [0, 1, 3, 12, 13, 14, 16]),
  [0, 3, 12, 13, 14], 'the active model supplies the Night Drive pool');
journeySelect.dispatchEvent(new Event('change'));
assert.equal(integrationController.state.selected, 'NightDrive', 'duplicate native change is idempotent');
integrationController.pause('manual world');
worldSelect.value = 'auto';
worldSelect.dispatchEvent(new Event('change'));
assert.equal(integrationController.state.selected, 'Surprise',
  'an explicit user World Auto choice still resumes Surprise Me');

console.log('Curated journey pools, cadence, music timing, pause and persistence checks passed');
