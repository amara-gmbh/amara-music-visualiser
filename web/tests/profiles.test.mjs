import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createEfficientGovernor, createProfileStore, defaultProfile, graphicsBudget, graphicsDecision, graphicsFrame, paletteAmount, parseProfile, validateProfile } from '../profiles.js';

class MemoryStorage {
  constructor() { this.values = new Map(); }
  get length() { return this.values.size; }
  key(i) { return [...this.values.keys()][i] ?? null; }
  getItem(key) { return this.values.has(key) ? this.values.get(key) : null; }
  setItem(key, value) { this.values.set(key, String(value)); }
  removeItem(key) { this.values.delete(key); }
}

const memory = new MemoryStorage();
const store = createProfileStore(memory);
const saved = defaultProfile('Purple flight');
saved.visual.scene = 14;
saved.visual.look = 7;
saved.visual.palette = 'MoccaVanilla';
saved.mappings.treble_to_sparkle = 1.3;
store.save(saved);
saved.visual.scene = 2; // Saved values do not alias the editable object.
assert.equal(store.load('Purple flight').visual.scene, 14);
assert.deepEqual(store.list(), ['Purple flight']);
assert.throws(() => store.save(saved), /already exists/);
assert.equal(store.load('Purple flight').visual.scene, 14);
assert.deepEqual(parseProfile(store.exportText('Purple flight')), store.load('Purple flight'));
assert.throws(() => store.importText(store.exportText('Purple flight')), /already exists/);
store.save(saved, { overwrite: true });
assert.equal(store.load('Purple flight').visual.scene, 2);

for (const [palette, amount] of [['Cosmic', 0], ['MoccaVanilla', 1], ['Aquarelle', 2]]) {
  const profile = defaultProfile(`Palette ${palette}`);
  profile.visual.palette = palette;
  assert.equal(parseProfile(JSON.stringify(profile)).visual.palette, palette);
  assert.equal(paletteAmount(palette), amount);
}

const fourK = {
  Low: graphicsBudget('Low', 3840, 2160, 1),
  Balanced: graphicsBudget('Balanced', 3840, 2160, 1),
  High: graphicsBudget('High', 3840, 2160, 1),
};
const html = await readFile(new URL('../index.html', import.meta.url), 'utf8');
assert.match(html, /<option value="Low">Efficient<\/option>/);
assert.match(html, /<option value="Auto">Automatic<\/option>/);
assert.match(html, /<option value="Balanced" selected>Balanced<\/option>/);
assert.match(html, /<option value="High">High Detail<\/option>/);
assert.deepEqual(fourK.Low, { width: 1152, height: 648, fps: 30 });
assert.deepEqual(fourK.Balanced, { width: 1440, height: 810, fps: 45 });
assert.deepEqual(fourK.High, { width: 1600, height: 1000, fps: 60 });
assert.deepEqual(graphicsDecision('Auto', 0, 0, false, 0),
  { quality: 'Balanced', adaptiveStep: 0, reason: 'balanced' });
assert.deepEqual(graphicsBudget('Auto', 3840, 2160, 1, 0, 0, false, 0), fourK.Balanced);
assert.deepEqual(graphicsDecision('Auto', 9, 9, false, 0),
  { quality: 'Low', adaptiveStep: 0, reason: 'world' });
assert.deepEqual(graphicsBudget('Auto', 3840, 2160, 1, 9, 9, false, 0),
  { width: 1152, height: 648, fps: 24 });
assert.deepEqual(graphicsDecision('Auto', 0, 1, true, 0),
  { quality: 'Low', adaptiveStep: 0, reason: 'transition' });
assert.deepEqual(graphicsBudget('Auto', 3840, 2160, 1, 0, 1, true, 0),
  { width: 1152, height: 648, fps: 24 });
assert.deepEqual(graphicsDecision('Auto', 0, 0, false, 1),
  { quality: 'Low', adaptiveStep: 0, reason: 'load' });
assert.deepEqual(graphicsBudget('Auto', 3840, 2160, 1, 0, 0, false, 1),
  { width: 1152, height: 648, fps: 30 });
assert.deepEqual(graphicsBudget('Auto', 3840, 2160, 1, 0, 0, false, 2),
  { width: 1152, height: 648, fps: 20 });
assert.deepEqual(graphicsDecision('Auto', 9, 9, false, 2),
  { quality: 'Low', adaptiveStep: 2, reason: 'load' });
assert.deepEqual(graphicsBudget('Auto', 3840, 2160, 1, 9, 9, false, 2),
  { width: 1152, height: 648, fps: 20 });
for (const costlyWorld of [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]) {
  assert.deepEqual(graphicsBudget('Low', 3840, 2160, 1, costlyWorld, costlyWorld, false, 0),
    { width: 1152, height: 648, fps: 24 });
}
assert.deepEqual(graphicsBudget('Low', 3840, 2160, 1, 0, 1, true, 0),
  { width: 1152, height: 648, fps: 24 }); // Transitions change cadence, never resolution.
assert.deepEqual(graphicsBudget('Low', 3840, 2160, 1, 9, 9, false, 1),
  { width: 1152, height: 648, fps: 20 }); // Aurora plus a governor step uses the cadence fallback.
assert.deepEqual(graphicsBudget('Low', 3840, 2160, 1, 0, 0, false, 2),
  { width: 1152, height: 648, fps: 20 });
assert.deepEqual(graphicsBudget('Balanced', 3840, 2160, 1, 9, 3, true, 2), fourK.Balanced);
assert.deepEqual(graphicsBudget('High', 3840, 2160, 1, 9, 3, true, 2), fourK.High);
assert.ok(fourK.Low.width * fourK.Low.height < fourK.Balanced.width * fourK.Balanced.height);
assert.ok(fourK.Balanced.width * fourK.Balanced.height < fourK.High.width * fourK.High.height);
const governor = createEfficientGovernor({ sampleInterval: 100, cooldown: 200,
  recoveryCooldown: 300, heavySamples: 2, recoverySamples: 3 });
assert.equal(governor.shouldSample(0), true);
assert.equal(governor.shouldSample(1), false); // Never overlap queue samples.
assert.equal(governor.observe(35, 5), false);
assert.equal(governor.shouldSample(99), false);
assert.equal(governor.shouldSample(100), true);
assert.equal(governor.observe(34, 105), true);
assert.equal(governor.step, 1);
assert.deepEqual(graphicsDecision('Auto', 0, 0, false, governor.step),
  { quality: 'Low', adaptiveStep: 0, reason: 'load' });
assert.deepEqual(graphicsBudget('Low', 3840, 2160, 1, 9, 9, false, governor.step),
  { width: 1152, height: 648, fps: 20 });
assert.equal(governor.shouldSample(304), false); // Step-down cooldown prevents resize churn.
for (const time of [305, 405]) {
  assert.equal(governor.shouldSample(time), true);
  assert.equal(governor.observe(8, time + 1), false);
}
assert.equal(governor.shouldSample(505), true);
assert.equal(governor.observe(8, 506), true);
assert.equal(governor.step, 0); // Recovery requires more sustained samples than step-down.
assert.deepEqual(graphicsDecision('Auto', 0, 0, false, governor.step),
  { quality: 'Balanced', adaptiveStep: 0, reason: 'balanced' });
assert.deepEqual(graphicsBudget('Low', 3840, 2160, 1, 9, 9, false, governor.step),
  { width: 1152, height: 648, fps: 24 });
assert.equal(governor.shouldSample(805), false);
assert.equal(governor.shouldSample(806), true);
governor.cancel(807);
assert.equal(governor.shouldSample(900), false);
let frame = graphicsFrame(100, 0, 45);
assert.equal(frame.due, true);
assert.equal(graphicsFrame(110, frame.deadline, 45).due, false);
assert.equal(graphicsFrame(123, frame.deadline, 45).due, true);
const legacy = defaultProfile('Legacy graphics');
delete legacy.visual.graphics_quality;
assert.equal(validateProfile(legacy).visual.graphics_quality, 'Balanced');
const automatic = defaultProfile('Automatic graphics');
automatic.visual.graphics_quality = 'Auto';
assert.equal(validateProfile(automatic).visual.graphics_quality, 'Auto');
assert.throws(() => graphicsBudget('Ultra', 1920, 1080), /graphics_quality/);

// Bundled Mac presets without the later optional fields remain importable.
for (const file of ['Fractal-Mocca.json', 'Liquid-Light.json', 'Velvet-Flight.json']) {
  const text = await readFile(new URL(`../../presets/${file}`, import.meta.url), 'utf8');
  const profile = parseProfile(text);
  assert.equal(profile.visual.look, 0);
  assert.equal(profile.visual.background, 'Auto');
  store.importText(text);
  assert.deepEqual(parseProfile(store.exportText(profile.name)), profile);
}

const malformed = defaultProfile('Malformed');
malformed.visual.scene = 17;
assert.throws(() => validateProfile(malformed), /visual.scene/);
malformed.visual.scene = 0;
malformed.visual.look = 1.5;
assert.throws(() => validateProfile(malformed), /visual.look/);
malformed.visual.look = 0;
malformed.mappings.treble_to_sparkle = Infinity;
assert.throws(() => validateProfile(malformed), /treble_to_sparkle/);
malformed.mappings.treble_to_sparkle = 0.9;
malformed.visual.unknown = true;
assert.throws(() => validateProfile(malformed), /Unknown visual field/);
assert.throws(() => parseProfile('{bad'), /Invalid profile JSON/);
assert.throws(() => parseProfile(' '.repeat(65537)), /64 KiB/);
assert.throws(() => defaultProfile('../bad'), /Profile name/);
assert.throws(() => defaultProfile('  leading'), /Profile name/);

// A malformed stored entry is skipped but never silently overwritten/deleted.
const corruptKey = 'amaraMusicVisualiser.profile.v1:Corrupt';
memory.setItem(corruptKey, '{bad');
assert.ok(!store.list().includes('Corrupt'));
assert.throws(() => store.save(defaultProfile('Corrupt')), /already exists/);
assert.equal(memory.getItem(corruptKey), '{bad');
const failingStorage = new MemoryStorage();
failingStorage.setItem = () => { throw new Error('quota'); };
assert.throws(() => createProfileStore(failingStorage).save(defaultProfile()), /full or unavailable/);
assert.equal(failingStorage.length, 0);
store.remove('Purple flight');
assert.throws(() => store.load('Purple flight'), /not found/);

console.log('Profile import, export, validation and safe storage tests passed');
