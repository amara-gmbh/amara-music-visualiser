import { defaultProfile, validateProfile } from './profiles.js?v=20261031.16';
import { JOURNEYS } from './journeys.js?v=20261031.16';

const FORMAT = 1;
const MAX_BYTES = 12 * 1024;
const FAVORITE_PREFIX = 'amaraMusicVisualiser.favorite.v1:';
const ROOT_KEYS = ['format_version', 'journey', 'visual', 'mappings'];
const JOURNEY_KEYS = ['id', 'customized'];
const VISUAL_KEYS = ['world', 'selection', 'look', 'palette', 'graphics_quality', 'pace', 'intensity', 'detail'];
const MAPPING_KEYS = ['bass', 'mids', 'highs', 'beat'];

function object(value, label, allowed) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${label} must be an object`);
  for (const key of Object.keys(value)) if (!allowed.includes(key)) throw new Error(`Unknown ${label} field: ${key}`);
  return value;
}

function favoriteName(value) {
  if (typeof value !== 'string' || value !== value.trim() || value.length < 1 || [...value].length > 80 ||
      /[\u0000-\u001f\u007f-\u009f/\\]/u.test(value)) {
    throw new Error('Favorite name must be 1–80 characters without slashes or control characters');
  }
  return value;
}

function byteLength(text) { return new TextEncoder().encode(text).byteLength; }

export function validateRecipe(input) {
  const root = object(input, 'recipe', ROOT_KEYS);
  if (root.format_version !== FORMAT) throw new Error('Unsupported recipe version');
  const journey = object(root.journey, 'recipe journey', JOURNEY_KEYS);
  const visual = object(root.visual, 'recipe visual', VISUAL_KEYS);
  const mappings = object(root.mappings, 'recipe mappings', MAPPING_KEYS);
  const journeyIds = ['Manual', ...Object.keys(JOURNEYS)];
  if (!journeyIds.includes(journey.id)) throw new Error('Unsupported recipe journey');
  if (typeof journey.customized !== 'boolean') throw new Error('recipe journey customized must be true or false');
  if (visual.selection !== 'auto' && visual.selection !== 'manual') throw new Error('Unsupported recipe world selection');
  if (journey.id !== 'Manual' && visual.selection !== 'auto') throw new Error('An active journey requires Auto world selection');
  if (journey.id !== 'Manual' && !journey.customized) {
    const defaults = JOURNEYS[journey.id];
    if (visual.palette !== defaults.palette || visual.pace !== defaults.pace) {
      throw new Error('An uncustomized journey must use its palette and pace defaults');
    }
  }

  // Reuse the established profile bounds and enum validation without retaining
  // the profile's audio fields in the recipe result.
  const profile = defaultProfile('Visual recipe');
  Object.assign(profile.visual, { scene: visual.world, look: visual.look, palette: visual.palette,
    graphics_quality: visual.graphics_quality, motion: visual.pace,
    intensity: visual.intensity, complexity: visual.detail });
  Object.assign(profile.mappings, { bass_to_pulse: mappings.bass,
    mids_to_geometry: mappings.mids, treble_to_sparkle: mappings.highs,
    beat_to_flash: mappings.beat });
  const safe = validateProfile(profile);
  return {
    format_version: FORMAT,
    journey: { id: journey.id, customized: journey.customized },
    visual: { world: safe.visual.scene, selection: visual.selection, look: safe.visual.look,
      palette: safe.visual.palette, graphics_quality: safe.visual.graphics_quality,
      pace: safe.visual.motion, intensity: safe.visual.intensity, detail: safe.visual.complexity },
    mappings: { bass: safe.mappings.bass_to_pulse, mids: safe.mappings.mids_to_geometry,
      highs: safe.mappings.treble_to_sparkle, beat: safe.mappings.beat_to_flash },
  };
}

export function createRecipe(values) { return validateRecipe(values); }

export function recipeApplication(input) {
  const recipe = validateRecipe(input);
  return structuredClone(recipe); // Deliberately contains no audio action or source field.
}

export function recipeText(input) {
  const text = JSON.stringify(validateRecipe(input));
  if (byteLength(text) > MAX_BYTES) throw new Error('Recipe is too large');
  return text;
}

export function parseRecipeText(text) {
  if (typeof text !== 'string' || byteLength(text) > MAX_BYTES) throw new Error('Recipe must be text no larger than 12 KiB');
  let value;
  try { value = JSON.parse(text); } catch { throw new Error('Invalid recipe JSON'); }
  return validateRecipe(value);
}

export function encodeRecipe(input) {
  const bytes = new TextEncoder().encode(recipeText(input));
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/u, '');
}

export function decodeRecipe(token) {
  if (typeof token !== 'string' || token.length > MAX_BYTES * 2 || !/^[A-Za-z0-9_-]+$/u.test(token)) {
    throw new Error('Invalid recipe link');
  }
  const padded = token.replace(/-/g, '+').replace(/_/g, '/') + '='.repeat((4 - token.length % 4) % 4);
  let binary;
  try { binary = atob(padded); } catch { throw new Error('Invalid recipe link'); }
  const bytes = Uint8Array.from(binary, character => character.charCodeAt(0));
  return parseRecipeText(new TextDecoder().decode(bytes));
}

export function recipeLink(input, href) {
  const url = new URL(href);
  url.hash = `recipe=${encodeRecipe(input)}`;
  return url.href;
}

export function recipeFromHash(hash) {
  const match = /^#recipe=([A-Za-z0-9_-]+)$/u.exec(hash || '');
  return match ? decodeRecipe(match[1]) : null;
}

export function createFavoriteStore(storage) {
  if (storage === undefined) {
    try { storage = globalThis.localStorage; } catch { storage = null; }
  }
  if (!storage) throw new Error('Browser storage is unavailable');
  const keyFor = name => FAVORITE_PREFIX + encodeURIComponent(favoriteName(name));
  function list() {
    const names = [];
    try {
      for (let i = 0; i < storage.length; i++) {
        const key = storage.key(i);
        if (typeof key !== 'string' || !key.startsWith(FAVORITE_PREFIX)) continue;
        const name = decodeURIComponent(key.slice(FAVORITE_PREFIX.length));
        try { favoriteName(name); parseRecipeText(storage.getItem(key)); names.push(name); } catch { /* Preserve invalid data. */ }
      }
    } catch { throw new Error('Browser storage is unavailable'); }
    return names.sort((a, b) => a.localeCompare(b));
  }
  function save(name, input) {
    const safeName = favoriteName(name);
    const key = keyFor(safeName);
    try {
      if (storage.getItem(key) !== null) throw new Error('A favorite with this name already exists');
      storage.setItem(key, recipeText(input));
    } catch (error) {
      if (error instanceof Error && /already exists/u.test(error.message)) throw error;
      throw new Error('Could not save favorite; browser storage may be unavailable');
    }
    return safeName;
  }
  function load(name) {
    let text;
    try { text = storage.getItem(keyFor(name)); } catch { throw new Error('Browser storage is unavailable'); }
    if (text === null) throw new Error('Favorite not found');
    return parseRecipeText(text);
  }
  function remove(name) {
    const key = keyFor(name);
    try {
      if (storage.getItem(key) === null) throw new Error('Favorite not found');
      storage.removeItem(key);
    } catch (error) {
      if (error instanceof Error && /not found/u.test(error.message)) throw error;
      throw new Error('Could not remove favorite');
    }
  }
  return { list, save, load, remove };
}
