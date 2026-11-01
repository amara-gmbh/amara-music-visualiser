/** Curated, local-only browser journeys built from existing worlds and settings. */
const STORAGE_KEY = 'amaraMusicVisualiser.journey.v1';

export const JOURNEYS = Object.freeze({
  Surprise: Object.freeze({ label: 'Surprise Me', worlds: null, palette: 'Cosmic', pace: 0.85, dwell: 24 }),
  Dreamy: Object.freeze({ label: 'Dreamy', worlds: Object.freeze([1, 2, 6, 9, 15, 16]), palette: 'Aquarelle', pace: 0.55, dwell: 34 }),
  NightDrive: Object.freeze({ label: 'Night Drive', worlds: Object.freeze([0, 3, 12, 13, 14]), palette: 'Cosmic', pace: 0.9, dwell: 28 }),
  DeepFocus: Object.freeze({ label: 'Deep Focus', worlds: Object.freeze([1, 5, 6, 10, 11]), palette: 'MoccaVanilla', pace: 0.38, dwell: 45 }),
  Dancefloor: Object.freeze({ label: 'Dancefloor', worlds: Object.freeze([3, 4, 7, 12, 13]), palette: 'Cosmic', pace: 1.15, dwell: 20 }),
  Aquarelle: Object.freeze({ label: 'Aquarelle', worlds: Object.freeze([1, 2, 6, 9, 15, 16]), palette: 'Aquarelle', pace: 0.5, dwell: 38 }),
});

export function journeyDefinition(id) {
  const journey = JOURNEYS[id];
  if (!journey) throw new Error('Unknown journey');
  return journey;
}

export function journeyWorlds(id, available) {
  const journey = journeyDefinition(id);
  const allowed = journey.worlds ? new Set(journey.worlds) : null;
  return available.filter(world => !allowed || allowed.has(world));
}

export function journeyDwell(id, world) {
  const dwell = journeyDefinition(id).dwell;
  return world === 10 ? Math.max(65, dwell) : dwell;
}

export function describeJourney(state, world) {
  if (!state?.active) return `Journey paused · ${state?.reason || 'manual control'}`;
  const journey = journeyDefinition(state.selected);
  const scope = journey.worlds ? `${journey.worlds.length} curated worlds` : 'all available worlds';
  const settings = state.customized ? `customized ${state.reason}` : `${journey.palette} · pace ${journey.pace}`;
  return `${journey.label} · ${scope} · ${settings} · about ${journeyDwell(state.selected, world)}s per world`;
}

export function journeyShouldAdvance({ id, world, age, insight }) {
  const dwell = journeyDwell(id, world);
  if (!Number.isFinite(age) || age < dwell) return false;
  const overdue = age - dwell;
  if (insight?.event === 'drop' && insight.drop >= 0.8) return true;
  if ((!insight?.active || insight.section === 'quiet') && overdue < 12) return false;
  if (insight?.trend === 'rising' && overdue < 6) return false;
  return true;
}

export function createJourneyController(storage) {
  if (storage === undefined) {
    try { storage = globalThis.localStorage; } catch { storage = null; }
  }
  let selected = 'Surprise';
  try {
    const stored = storage?.getItem(STORAGE_KEY);
    if (stored === 'Manual' || Object.hasOwn(JOURNEYS, stored)) selected = stored;
  } catch { /* Local persistence is optional. */ }
  const state = { selected, active: selected !== 'Manual', customized: false, reason: '' };
  function persist() {
    try { storage?.setItem(STORAGE_KEY, state.selected); } catch { /* Keep the in-memory choice usable. */ }
  }
  function choose(id) {
    journeyDefinition(id);
    Object.assign(state, { selected: id, active: true, customized: false, reason: '' });
    persist();
    return state;
  }
  function pause(reason = 'manual choice') {
    Object.assign(state, { selected: 'Manual', active: false, customized: false, reason });
    persist();
    return state;
  }
  function customize(reason) {
    if (state.active) { state.customized = true; state.reason = reason; }
    return state;
  }
  return { state, choose, pause, customize };
}

export function createJourneySelectionGuard() {
  let depth = 0;
  let pendingAuto = false;
  let generation = 0;
  function run(action) {
    const token = ++generation;
    pendingAuto = true;
    depth++;
    try { return action(); } finally {
      depth--;
      setTimeout(() => { if (generation === token) pendingAuto = false; }, 0);
    }
  }
  function consumeAuto(value) {
    if (value !== 'auto' || (depth === 0 && !pendingAuto)) return false;
    pendingAuto = false;
    return true;
  }
  return { run, consumeAuto, get active() { return depth > 0; } };
}
