// Version 1 profiles share the Mac app's JSON format and validation limits.
// Audio access is never started or changed by loading a profile.
const PREFIX = 'amaraMusicVisualiser.profile.v1:';
const MAX_BYTES = 64 * 1024;
const VISUAL_KEYS = ['scene', 'intensity', 'motion', 'complexity', 'hue', 'glow', 'warp', 'feedback', 'palette', 'background', 'math_form', 'look', 'graphics_quality'];
const MAPPING_KEYS = ['bass_to_pulse', 'mids_to_geometry', 'treble_to_sparkle', 'beat_to_flash'];
const PROFILE_KEYS = ['format_version', 'name', 'visual', 'mappings', 'audio_mode', 'director_enabled', 'director_freedom'];
const PALETTES = ['Cosmic', 'MoccaVanilla', 'Aquarelle'];
const GRAPHICS_QUALITIES = ['Auto', 'Low', 'Balanced', 'High'];
const EFFICIENT_HEAVY_WORLDS = new Set([3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]);
const EFFICIENT_LIMITS = [
  { width: 1152, height: 720, pixels: 829_440, fps: 30 },
  { width: 1152, height: 720, pixels: 829_440, fps: 24 },
  { width: 1152, height: 720, pixels: 829_440, fps: 20 },
];
const BALANCED_LIMITS = { width: 1440, height: 900, pixels: 1_200_000, fps: 45 };
const BACKGROUNDS = ['Auto', 'WorldColours', 'VioletNight', 'OceanJade', 'EmberGold', 'RoseGlass', 'DeepBlue', 'PearlAmber', 'Lagoon', 'SlateLilac'];
const MATH_FORMS = ['Journey', 'Julia', 'InversionLace', 'HarmonicSculpture', 'VortexWeave'];

function object(value, label, allowed) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error(`${label} must be an object`);
  for (const key of Object.keys(value)) {
    if (!allowed.includes(key)) throw new Error(`Unknown ${label} field: ${key}`);
  }
  return value;
}

function nameValue(value) {
  if (typeof value !== 'string' || value.length === 0 || value !== value.trim() ||
      [...value].length > 80 || value === '.' || value === '..' ||
      /[\u0000-\u001f\u007f-\u009f/\\]/u.test(value)) {
    throw new Error('Profile name must be 1–80 characters, without surrounding spaces, slashes or control characters');
  }
  try { encodeURIComponent(value); } catch { throw new Error('Profile name contains invalid Unicode'); }
  return value;
}

function number(value, label, min, max, integer = false) {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < min || value > max || (integer && !Number.isInteger(value))) {
    throw new Error(`${label} must be ${integer ? 'an integer' : 'a number'} from ${min} to ${max}`);
  }
  return value;
}

function choice(value, label, options) {
  if (!options.includes(value)) throw new Error(`Unsupported ${label}`);
  return value;
}

export function defaultProfile(name = 'My profile') {
  return {
    format_version: 1, name: nameValue(name),
    visual: { scene: 0, intensity: 0.92, motion: 0.75, complexity: 0.8, hue: 0.72, glow: 0.95, warp: 0.7, feedback: 0.22, palette: 'Cosmic', background: 'Auto', math_form: 'Journey', look: 0, graphics_quality: 'Balanced' },
    mappings: { bass_to_pulse: 1, mids_to_geometry: 0.72, treble_to_sparkle: 0.9, beat_to_flash: 0.8 },
    audio_mode: 'Demo', director_enabled: false, director_freedom: 0.46,
  };
}

export function paletteAmount(palette) {
  return PALETTES.indexOf(choice(palette, 'visual.palette', PALETTES));
}

export function graphicsDecision(quality, world = 0, destination = world,
  transitioning = false, adaptiveStep = 0) {
  const selected = choice(quality, 'visual.graphics_quality', GRAPHICS_QUALITIES);
  const costlyWorld = EFFICIENT_HEAVY_WORLDS.has(world) || EFFICIENT_HEAVY_WORLDS.has(destination);
  const adaptive = Math.max(0, Math.min(2, Math.floor(Number.isFinite(adaptiveStep) ? adaptiveStep : 0)));
  if (selected !== 'Auto') return { quality: selected, adaptiveStep: adaptive, reason: 'manual' };
  if (!transitioning && !costlyWorld && adaptive === 0) {
    return { quality: 'Balanced', adaptiveStep: 0, reason: 'balanced' };
  }
  return { quality: 'Low', adaptiveStep: adaptive === 2 ? 2 : 0,
    reason: adaptive === 2 ? 'load' : transitioning ? 'transition' : costlyWorld ? 'world' : 'load' };
}

export function graphicsBudget(quality, cssWidth, cssHeight, pixelRatio = 1,
  world = 0, destination = world, transitioning = false, adaptiveStep = 0) {
  const decision = graphicsDecision(quality, world, destination, transitioning, adaptiveStep);
  const selected = decision.quality;
  const width = Math.max(1, Number.isFinite(cssWidth) ? cssWidth : 1);
  const height = Math.max(1, Number.isFinite(cssHeight) ? cssHeight : 1);
  const ratio = Math.max(0.5, Math.min(Number.isFinite(pixelRatio) ? pixelRatio : 1, 1.35));
  const naturalWidth = width * ratio;
  const naturalHeight = height * ratio;
  if (selected === 'High') {
    return { width: Math.max(1, Math.min(1600, Math.round(naturalWidth))),
      height: Math.max(1, Math.min(1000, Math.round(naturalHeight))), fps: 60 };
  }
  let limits;
  if (selected === 'Low') {
    const costlyWorld = EFFICIENT_HEAVY_WORLDS.has(world) || EFFICIENT_HEAVY_WORLDS.has(destination);
    const baseStep = transitioning || costlyWorld ? 1 : 0;
    const adaptive = decision.adaptiveStep;
    limits = EFFICIENT_LIMITS[Math.min(2, baseStep + adaptive)];
  } else {
    limits = BALANCED_LIMITS;
  }
  let scale = Math.min(1, limits.width / naturalWidth, limits.height / naturalHeight);
  let targetWidth = Math.max(1, Math.round(naturalWidth * scale));
  let targetHeight = Math.max(1, Math.round(naturalHeight * scale));
  if (targetWidth * targetHeight > limits.pixels) {
    scale = Math.sqrt(limits.pixels / (targetWidth * targetHeight));
    targetWidth = Math.max(1, Math.round(targetWidth * scale));
    targetHeight = Math.max(1, Math.round(targetHeight * scale));
  }
  return { width: targetWidth, height: targetHeight, fps: limits.fps };
}

export function createEfficientGovernor(options = {}) {
  const sampleInterval = options.sampleInterval ?? 2500;
  const cooldown = options.cooldown ?? 8000;
  const recoveryCooldown = options.recoveryCooldown ?? 12_000;
  const heavyLatency = options.heavyLatency ?? 30;
  const headroomLatency = options.headroomLatency ?? 12;
  const heavySamples = options.heavySamples ?? 2;
  const recoverySamples = options.recoverySamples ?? 6;
  let step = 0;
  let pending = false;
  let nextSample = 0;
  let heavyCount = 0;
  let headroomCount = 0;

  function reset() {
    step = 0;
    pending = false;
    nextSample = 0;
    heavyCount = 0;
    headroomCount = 0;
  }
  function shouldSample(now) {
    if (pending || !Number.isFinite(now) || now < nextSample) return false;
    pending = true;
    nextSample = now + sampleInterval;
    return true;
  }
  function observe(latency, now) {
    pending = false;
    if (!Number.isFinite(latency) || !Number.isFinite(now) || latency < 0) {
      heavyCount = 0;
      headroomCount = 0;
      return false;
    }
    if (latency >= heavyLatency) {
      heavyCount++;
      headroomCount = 0;
    } else if (latency <= headroomLatency) {
      headroomCount++;
      heavyCount = 0;
    } else {
      heavyCount = 0;
      headroomCount = 0;
    }
    if (heavyCount >= heavySamples && step < 2) {
      step++;
      heavyCount = 0;
      headroomCount = 0;
      nextSample = Math.max(nextSample, now + cooldown);
      return true;
    }
    if (headroomCount >= recoverySamples && step > 0) {
      step--;
      heavyCount = 0;
      headroomCount = 0;
      nextSample = Math.max(nextSample, now + recoveryCooldown);
      return true;
    }
    return false;
  }
  function cancel(now = 0) {
    pending = false;
    heavyCount = 0;
    headroomCount = 0;
    if (Number.isFinite(now)) nextSample = Math.max(nextSample, now + sampleInterval);
  }
  return { reset, shouldSample, observe, cancel, get step() { return step; } };
}

export function graphicsFrame(timestamp, deadline, fps) {
  if (!deadline || timestamp + 1 >= deadline) {
    return { due: true, deadline: Math.max(timestamp, deadline || timestamp) + 1000 / fps };
  }
  return { due: false, deadline };
}

// Return a fresh, complete profile. Never silently coerce malformed input.
export function validateProfile(input) {
  const profile = object(input, 'profile', PROFILE_KEYS);
  const visual = object(profile.visual, 'visual', VISUAL_KEYS);
  const mappings = object(profile.mappings, 'mappings', MAPPING_KEYS);
  if (profile.format_version !== 1) throw new Error('Unsupported profile format; expected version 1');
  if (typeof profile.director_enabled !== 'boolean') throw new Error('director_enabled must be true or false');
  return {
    format_version: 1, name: nameValue(profile.name),
    visual: {
      scene: number(visual.scene, 'visual.scene', 0, 16, true),
      intensity: number(visual.intensity, 'visual.intensity', 0.15, 1.6),
      motion: number(visual.motion, 'visual.motion', 0.05, 1.8),
      complexity: number(visual.complexity, 'visual.complexity', 0.15, 1.5),
      hue: number(visual.hue, 'visual.hue', 0, 1),
      glow: number(visual.glow, 'visual.glow', 0, 1.6),
      warp: number(visual.warp, 'visual.warp', 0, 1.6),
      feedback: number(visual.feedback === undefined ? 0.22 : visual.feedback, 'visual.feedback', 0, 0.92),
      palette: choice(visual.palette === undefined ? 'Cosmic' : visual.palette, 'visual.palette', PALETTES),
      background: choice(visual.background === undefined ? 'Auto' : visual.background, 'visual.background', BACKGROUNDS),
      math_form: choice(visual.math_form === undefined ? 'Journey' : visual.math_form, 'visual.math_form', MATH_FORMS),
      look: number(visual.look === undefined ? 0 : visual.look, 'visual.look', 0, 9, true),
      graphics_quality: choice(visual.graphics_quality === undefined ? 'Balanced' : visual.graphics_quality,
        'visual.graphics_quality', GRAPHICS_QUALITIES),
    },
    mappings: {
      bass_to_pulse: number(mappings.bass_to_pulse, 'mappings.bass_to_pulse', 0, 1.5),
      mids_to_geometry: number(mappings.mids_to_geometry, 'mappings.mids_to_geometry', 0, 1.5),
      treble_to_sparkle: number(mappings.treble_to_sparkle, 'mappings.treble_to_sparkle', 0, 1.5),
      beat_to_flash: number(mappings.beat_to_flash, 'mappings.beat_to_flash', 0, 1.5),
    },
    audio_mode: choice(profile.audio_mode, 'audio_mode', ['Demo', 'System', 'Microphone']),
    director_enabled: profile.director_enabled,
    director_freedom: number(profile.director_freedom, 'director_freedom', 0, 1),
  };
}

export function parseProfile(text) {
  if (typeof text !== 'string' || new TextEncoder().encode(text).byteLength > MAX_BYTES) throw new Error('Profile must be a JSON text file no larger than 64 KiB');
  let value;
  try { value = JSON.parse(text); } catch { throw new Error('Invalid profile JSON'); }
  return validateProfile(value);
}

export function createProfileStore(storage) {
  if (storage === undefined) {
    try { storage = globalThis.localStorage; } catch { throw new Error('Browser storage is unavailable'); }
  }
  if (!storage) throw new Error('Browser storage is unavailable');
  const keyFor = name => PREFIX + encodeURIComponent(nameValue(name));
  function read(key) {
    try { return storage.getItem(key); } catch { throw new Error('Browser storage is unavailable'); }
  }
  function list() {
    const names = [];
    try {
      for (let i = 0; i < storage.length; i++) {
        const key = storage.key(i);
        if (typeof key !== 'string' || !key.startsWith(PREFIX)) continue;
        try {
          const profile = parseProfile(storage.getItem(key));
          if (keyFor(profile.name) === key) names.push(profile.name);
        } catch { /* Keep invalid entries untouched for recovery. */ }
      }
    } catch { throw new Error('Browser storage is unavailable'); }
    return names.sort((a, b) => a.localeCompare(b));
  }
  function save(input, { overwrite = false } = {}) {
    const profile = validateProfile(input);
    const key = keyFor(profile.name);
    if (!overwrite && read(key) !== null) throw new Error('A profile with this name already exists');
    const text = JSON.stringify(profile, null, 2);
    if (new TextEncoder().encode(text).byteLength > MAX_BYTES) throw new Error('Profile exceeds 64 KiB');
    try { storage.setItem(key, text); } catch { throw new Error('Could not save profile; browser storage may be full or unavailable'); }
    return profile;
  }
  function load(name) {
    const key = keyFor(name);
    const value = read(key);
    if (value === null) throw new Error('Profile not found');
    const profile = parseProfile(value);
    if (profile.name !== name) throw new Error('Stored profile name does not match its key');
    return profile;
  }
  function remove(name) {
    const key = keyFor(name);
    if (read(key) === null) throw new Error('Profile not found');
    try { storage.removeItem(key); } catch { throw new Error('Could not remove profile; browser storage is unavailable'); }
  }
  return { list, save, load, remove, importText: (text, options) => save(parseProfile(text), options), exportText: name => `${JSON.stringify(load(name), null, 2)}\n` };
}
