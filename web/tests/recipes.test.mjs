import assert from 'node:assert/strict';
import { createFavoriteStore, createRecipe, decodeRecipe, encodeRecipe, parseRecipeText,
  recipeApplication, recipeFromHash, recipeLink, recipeText } from '../recipes.js';

const recipe = createRecipe({
  format_version: 1,
  journey: { id: 'NightDrive', customized: true },
  visual: { world: 14, selection: 'auto', look: 7, palette: 'Aquarelle',
    graphics_quality: 'Auto', pace: 0.91, intensity: 1.02, detail: 0.77 },
  mappings: { bass: 1.2, mids: 0.8, highs: 1.1, beat: 0.7 },
});
assert.deepEqual(parseRecipeText(recipeText(recipe)), recipe);
assert.deepEqual(decodeRecipe(encodeRecipe(recipe)), recipe);
const link = recipeLink(recipe, 'http://127.0.0.1:8768/web/');
assert.match(link, /^http:\/\/127\.0\.0\.1:8768\/web\/#recipe=/u);
assert.deepEqual(recipeFromHash(new URL(link).hash), recipe);
const application = recipeApplication(recipe);
assert.deepEqual(application, recipe);
const audioState = { source: 'system', playing: true, fileName: 'private-track.wav' };
assert.deepEqual(audioState, { source: 'system', playing: true, fileName: 'private-track.wav' },
  'creating an application plan must not read or mutate audio state');
for (const forbidden of ['audio', 'source', 'filename', 'fileName', 'hardware', 'calibration', 'history']) {
  assert.equal(JSON.stringify(recipe).includes(forbidden), false, `recipe excludes ${forbidden}`);
  assert.equal(Object.hasOwn(application, forbidden), false, `application has no ${forbidden} action`);
}

const manual = createRecipe({ ...recipe, journey: { id: 'Manual', customized: false },
  visual: { ...recipe.visual, selection: 'manual', world: 6 } });
assert.equal(recipeApplication(manual).journey.id, 'Manual');
assert.equal(recipeApplication(manual).visual.selection, 'manual');
assert.throws(() => createRecipe({ ...recipe, visual: { ...recipe.visual, selection: 'manual' } }), /requires Auto/);
assert.throws(() => createRecipe({ ...recipe, journey: { id: 'NightDrive', customized: false },
  visual: { ...recipe.visual, selection: 'auto', palette: 'MoccaVanilla' } }), /palette and pace defaults/u);
assert.throws(() => parseRecipeText('{"format_version":2}'), /version|object/u);
assert.throws(() => parseRecipeText('{"format_version":1,"__proto__":{"polluted":true},"journey":{},"visual":{},"mappings":{}}'), /Unknown recipe field/);
assert.equal({}.polluted, undefined);
assert.throws(() => parseRecipeText('x'.repeat(12 * 1024 + 1)), /12 KiB/);
assert.throws(() => decodeRecipe('!not-base64!'), /Invalid recipe link/);
assert.throws(() => createRecipe({ ...recipe, visual: { ...recipe.visual, world: 99 } }), /visual.scene/);
assert.throws(() => createRecipe({ ...recipe, visual: { ...recipe.visual, pace: -1 } }), /visual.motion/);

class MemoryStorage {
  constructor() { this.values = new Map(); }
  get length() { return this.values.size; }
  key(index) { return [...this.values.keys()][index] ?? null; }
  getItem(key) { return this.values.has(key) ? this.values.get(key) : null; }
  setItem(key, value) { this.values.set(key, String(value)); }
  removeItem(key) { this.values.delete(key); }
}
const store = createFavoriteStore(new MemoryStorage());
store.save('Night coast', recipe);
assert.deepEqual(store.list(), ['Night coast']);
assert.deepEqual(store.load('Night coast'), recipe);
assert.throws(() => store.save('Night coast', recipe), /already exists/);
store.remove('Night coast');
assert.deepEqual(store.list(), []);
assert.throws(() => store.load('Night coast'), /not found/);

console.log('Visual recipe round-trip, validation, privacy and favorite storage checks passed');
