// Keep these revisions aligned with the main.js URL in index.html when
// releasing browser changes. A stale module can otherwise prevent startup.
import { createAudioEngine, localSystemAudioAvailable } from './audio.js?v=20261031.19';
import { createSystemAudioIntent } from './audio-intent.js?v=20261031.19';
import { createGeometryPass } from './geometry-pass.js?v=20261031.19';
import { createDotPass } from './dot-pass.js?v=20261031.19';
import { meterLevel, meterPeakBottom } from './equaliser-meter.js?v=20261031.19';
import { createWorldBag, createMandelbrotJourney, createSelectionGate, createMathJourney, createLookJourney } from './choreography.js?v=20261031.19';
import { createJourneyController, createJourneySelectionGuard, describeJourney, journeyDefinition, journeyDwell, journeyShouldAdvance, journeyWorlds } from './journeys.js?v=20261031.19';
import { LOOK_NAMES } from './look-names.js?v=20261031.19';
import { createMusicInsight, describeMusicInsight, musicVisualAdjustments } from './music-insight.js?v=20261031.19';
import { createEfficientGovernor, createProfileStore, defaultProfile, graphicsBudget, graphicsDecision, graphicsFrame, paletteAmount } from './profiles.js?v=20261031.19';
import { snapshotProfile } from './profile-adapter.js?v=20261031.19';
import { createFavoriteStore, createRecipe, parseRecipeText, recipeApplication, recipeFromHash, recipeLink, recipeText } from './recipes.js?v=20261031.19';
import { createVariationMode } from './variation-mode.js?v=20261031.19';
import { AUTO_WORLD_TRANSITION_SECONDS, createSelectionEventGuard, manualPipelineMasks,
  manualSwitchPlan, wallClockProgress } from './world-transition.js?v=20261031.19';

// Procedural worlds are available immediately; geometry worlds are added only
// after their shared native GPU pass compiles successfully.
const WORLDS = [
  [0, 'Deep Flight'], [1, 'Fractal Bloom'], [2, 'Glass Tide'],
  [3, 'Feedback Reverie'], [8, 'Water Ripples'], [9, 'Northern Lights'],
  [10, 'Mandelbrot'], [11, 'Liquid Stained Glass'], [13, 'Neon Memory'],
  [14, 'Neon Coast'], [15, 'Mocca & Cream'], [16, 'Aquarelle'],
];
const SHADERS = [
  'params.wgsl', 'shader.wgsl', 'worlds/flight.wgsl',
  'worlds/mathematics.wgsl', 'worlds/tide.wgsl', 'worlds/feedback.wgsl',
  'worlds/chromatic_space.wgsl', 'worlds/water.wgsl', 'worlds/aurora.wgsl',
  'worlds/mandelbrot.wgsl', 'worlds/liquid_stained_glass.wgsl',
  'worlds/neon_memory.wgsl', 'worlds/neon_coast.wgsl',
  'worlds/mocha_world.wgsl', 'worlds/aquarelle_world.wgsl',
];
const PROCEDURAL_IDS = new Set([0, 1, 2, 3, 8, 9, 10, 11, 13, 14, 15, 16]);
const WORLD_COUNT = 17;
const worldMask = id => PROCEDURAL_IDS.has(id) ? 1 << id : 0;
const canvas = document.querySelector('#visual');
const selector = document.querySelector('#world');
const variationSelector = document.querySelector('#variation');
const journeyInput = document.querySelector('#journey');
const journeyStatus = document.querySelector('#journey-status');
const status = document.querySelector('#status');
const fileInput = document.querySelector('#music-file');
const filePlayer = document.querySelector('#file-player');
const playButton = document.querySelector('#play');
const stopFileButton = document.querySelector('#stop-file');
const rewindFileButton = document.querySelector('#rewind-file');
const forwardFileButton = document.querySelector('#forward-file');
const fileSeek = document.querySelector('#file-seek');
const fileElapsed = document.querySelector('#file-elapsed');
const fileDuration = document.querySelector('#file-duration');
const fullscreen = document.querySelector('#fullscreen');
const spectrumView = document.querySelector('#spectrum');
const microphone = document.querySelector('#microphone');
const systemAudio = document.querySelector('#system-audio');
const systemAudioStatus = document.querySelector('#system-audio-status');
const musicInsightStatus = document.querySelector('#music-insight');
const shareAudio = document.querySelector('#share-audio');
const stopCapture = document.querySelector('#stop-capture');
const controls = document.querySelector('#controls');
const equaliser = document.querySelector('#equaliser');
const controlsToggle = document.querySelector('#toggle-controls');
const equaliserToggle = document.querySelector('#toggle-equaliser');
const sceneState = document.querySelector('#scene-state');
const sceneProgress = document.querySelector('#scene-progress');
const paceInput = document.querySelector('#pace');
const intensityInput = document.querySelector('#intensity');
const detailInput = document.querySelector('#detail');
const bassInput = document.querySelector('#bass-gain');
const midsInput = document.querySelector('#mids-gain');
const highsInput = document.querySelector('#highs-gain');
const paletteInput = document.querySelector('#palette');
const graphicsInput = document.querySelector('#graphics-quality');
const graphicsStatus = document.querySelector('#graphics-status');
const profileName = document.querySelector('#profile-name');
const profileList = document.querySelector('#profile-list');
const profileImport = document.querySelector('#profile-import');
const favoriteName = document.querySelector('#favorite-name');
const favoriteList = document.querySelector('#favorite-list');
const recipeImport = document.querySelector('#recipe-import');
const recipeStatus = document.querySelector('#recipe-status');
const audio = createAudioEngine();
const systemIntent = createSystemAudioIntent(() => window.sessionStorage);
const systemAudioAvailable = localSystemAudioAvailable(window.location.hostname);
if (!systemAudioAvailable) {
  systemIntent.clear();
  systemAudio.hidden = true;
  systemAudioStatus.hidden = true;
}

selector.add(new Option('Auto · all available worlds', 'auto'));
for (const [id, name] of WORLDS) selector.add(new Option(name, String(id)));
const bars = Array.from({ length: 48 }, () => {
  const bar = document.createElement('span');
  const fill = document.createElement('span');
  const peak = document.createElement('i');
  fill.className = 'spectrum-fill';
  peak.className = 'spectrum-peak';
  bar.append(fill, peak);
  spectrumView.append(bar);
  return { fill, peak, value: 0, until: 0 };
});
function setStatus(text) { status.textContent = text; }
function describeError(error) {
  return error instanceof Error ? error.message : String(error);
}
function togglePanel(panel, button, name) {
  panel.hidden = !panel.hidden;
  button.setAttribute('aria-expanded', String(!panel.hidden));
  button.textContent = `${panel.hidden ? 'Show' : 'Hide'} ${name}`;
}
controlsToggle.addEventListener('click', () => togglePanel(controls, controlsToggle, 'controls'));
equaliserToggle.addEventListener('click', () => togglePanel(equaliser, equaliserToggle, 'equaliser'));
let lastSystemSignal = 'off';
function formatTrackTime(seconds) {
  const total = Math.max(0, Math.floor(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const rest = String(total % 60).padStart(2, '0');
  return hours ? `${hours}:${String(minutes).padStart(2, '0')}:${rest}` : `${minutes}:${rest}`;
}
function syncAudioButtons() {
  const state = audio.getState();
  const capturing = state.source === 'microphone' || state.source === 'shared' || state.source === 'system' || state.connecting;
  stopCapture.disabled = !capturing;
  filePlayer.hidden = state.source !== 'file' || capturing;
  playButton.disabled = !state.fileName || capturing;
  playButton.textContent = state.source === 'file' && state.playing ? 'Pause' : 'Play';
  const duration = state.duration;
  const position = Math.max(0, Math.min(state.currentTime, duration ?? state.currentTime));
  fileSeek.disabled = !duration;
  rewindFileButton.disabled = !duration;
  forwardFileButton.disabled = !duration;
  if (duration) {
    fileSeek.max = String(duration);
    fileSeek.value = String(position);
    fileSeek.setAttribute('aria-valuetext', `${formatTrackTime(position)} of ${formatTrackTime(duration)}`);
  } else {
    fileSeek.max = '0';
    fileSeek.value = '0';
    fileSeek.removeAttribute('aria-valuetext');
  }
  fileElapsed.textContent = formatTrackTime(position);
  fileDuration.textContent = duration ? formatTrackTime(duration) : '--:--';
  systemAudio.disabled = !systemAudioAvailable || state.source === 'system' || state.connecting;
  systemAudio.textContent = systemIntent.requested() && state.source !== 'system'
    ? 'Reconnect computer audio' : 'Listen to computer audio';
  const labels = { connecting: 'Connecting to computer audio…', waiting: 'Computer audio connected; waiting for samples…',
    live: 'Computer audio · signal detected', silent: 'Computer audio connected · no sound detected' };
  const systemLabel = labels[state.systemSignal] || '';
  if (systemAudioStatus.textContent !== systemLabel) systemAudioStatus.textContent = systemLabel;
  systemAudioStatus.hidden = !systemLabel;
  if (systemLabel && (state.systemSignal !== lastSystemSignal || status.textContent.startsWith('Audio:'))) setStatus(systemLabel);
  lastSystemSignal = state.systemSignal;
}
async function beginListening(kind) {
  if (kind === 'system') systemIntent.request();
  else systemIntent.clear();
  try {
    const pending = kind === 'system' ? audio.startSystemAudio()
      : kind === 'microphone' ? audio.startMicrophone() : audio.startSharedAudio();
    syncAudioButtons();
    await pending;
    const state = audio.getState();
    if (state.source === kind && kind !== 'system' && state.playing) setStatus(kind === 'microphone' ? 'Listening to microphone' : 'Listening to shared audio');
    syncAudioButtons();
  } catch (error) { setStatus(`Audio: ${describeError(error)}`); syncAudioButtons(); }
}
microphone.addEventListener('click', () => beginListening('microphone'));
systemAudio.addEventListener('click', () => beginListening('system'));
shareAudio.addEventListener('click', () => beginListening('shared'));
stopCapture.addEventListener('click', () => { systemIntent.clear(); audio.stopCapture(); syncAudioButtons(); setStatus('Listening stopped'); });
// Stop private media tracks even when the browser retains this page in BFCache.
window.addEventListener('pagehide', () => audio.stopCapture());
window.addEventListener('pageshow', () => {
  syncAudioButtons();
  if (!systemAudioAvailable || !systemIntent.requested() || audio.getState().source === 'system') return;
  if (navigator.userActivation?.isActive) void beginListening('system');
  else setStatus('Computer audio was active · click Reconnect computer audio');
});
fileInput.addEventListener('change', async () => {
  const file = fileInput.files?.[0];
  if (!file) return;
  // The same file can be chosen again after switching to live audio.
  fileInput.value = '';
  try {
    systemIntent.clear();
    await audio.loadFile(file);
    await audio.togglePlayback();
    syncAudioButtons();
    setStatus(`Playing ${file.name} · music stays on this device`);
  } catch (error) { syncAudioButtons(); setStatus(`Could not play audio: ${describeError(error)}`); }
});
playButton.addEventListener('click', async () => {
  try {
    await audio.togglePlayback();
    const state = audio.getState();
    syncAudioButtons();
    setStatus(state.playing ? `Playing ${state.fileName}` : 'Paused');
  } catch (error) { setStatus(`Audio: ${describeError(error)}`); }
});
stopFileButton.addEventListener('click', () => {
  audio.stopFile();
  syncAudioButtons();
  setStatus('Track stopped');
});
rewindFileButton.addEventListener('click', () => {
  audio.seekFile(audio.getState().currentTime - 10);
  syncAudioButtons();
});
forwardFileButton.addEventListener('click', () => {
  audio.seekFile(audio.getState().currentTime + 10);
  syncAudioButtons();
});
fileSeek.addEventListener('input', () => {
  audio.seekFile(Number(fileSeek.value));
  syncAudioButtons();
});
fullscreen.addEventListener('click', async () => {
  try {
    if (document.fullscreenElement) await document.exitFullscreen();
    else await document.documentElement.requestFullscreen();
  } catch (error) { setStatus(`Fullscreen: ${describeError(error)}`); }
});

async function loadShaders() {
  const revision = new URL(import.meta.url).searchParams.get('v');
  const sources = await Promise.all(SHADERS.map(async path => {
    const url = new URL(`../src/${path}`, import.meta.url);
    if (revision) url.searchParams.set('v', revision);
    const response = await fetch(url);
    if (!response.ok) throw new Error(`Could not load shared shader ${path}`);
    return response.text();
  }));
  return sources.join('\n');
}

function clamp(value, min, max) { return Math.max(min, Math.min(max, value)); }

async function start() {
  if (!navigator.gpu) throw new Error('WebGPU is unavailable in this browser. Try a current desktop browser with WebGPU enabled.');
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
  if (!adapter) throw new Error('No compatible WebGPU graphics device was found.');
  const device = await adapter.requestDevice();
  device.addEventListener('uncapturederror', event => setStatus(`Graphics error: ${event.error.message}`));
  const context = canvas.getContext('webgpu');
  if (!context) throw new Error('The browser could not create a WebGPU canvas.');
  const format = navigator.gpu.getPreferredCanvasFormat();
  context.configure({ device, format, alphaMode: 'opaque' });
  let deviceLost = false;
  device.lost.then(info => {
    deviceLost = true;
    setStatus(`Graphics device lost: ${info.message || info.reason}. Reload the page to restart visuals.`);
  });
  const source = await loadShaders();
  const uniform = device.createBuffer({ size: 1120, usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST });
  const layout = device.createBindGroupLayout({ entries: [
    { binding: 0, visibility: GPUShaderStage.FRAGMENT, buffer: { type: 'uniform' } },
    { binding: 1, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: 'float', viewDimension: '2d' } },
    { binding: 2, visibility: GPUShaderStage.FRAGMENT, sampler: { type: 'filtering' } },
    { binding: 3, visibility: GPUShaderStage.FRAGMENT, texture: { sampleType: 'float', viewDimension: '2d-array' } },
  ] });
  const pipelineLayout = device.createPipelineLayout({ bindGroupLayouts: [layout] });
  const sampler = device.createSampler({ magFilter: 'linear', minFilter: 'linear', addressModeU: 'clamp-to-edge', addressModeV: 'clamp-to-edge' });
  // The native compositor samples a shared five-layer geometry texture.
  let rasterTexture = device.createTexture({ size: [1, 1, 5], format: 'rgba16float', usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.RENDER_ATTACHMENT });
  let rasterView = rasterTexture.createView({ dimension: '2d-array' });
  let geometry = null;
  const pipelines = new Map();
  const failedWorlds = new Set();
  const moduleFor = mask => {
    const bridge = mask === 12 || mask === 260 || mask === 264;
    const code = source
      .replace('const included_worlds: u32 = 126735u;', `const included_worlds: u32 = ${mask}u;`)
      .replace('const implicit_bridges: bool = true;', `const implicit_bridges: bool = ${bridge};`)
      // Water can be reached through a per-pixel world morph. Use a bounded
      // screen-space footprint there: derivatives in that branch are not
      // guaranteed to be uniform under browser WebGPU validation.
      .replace('fwidth(axis) * sharpness', 'pixel() * sharpness')
      // Browser WebGPU enforces uniform derivative control flow. History is
      // a single-level texture, so explicit level zero is equivalent here.
      .replace('textureSample(previous_frame, frame_sampler,\n            clamp(history_uv, vec2<f32>(0.0), vec2<f32>(1.0)))',
        'textureSampleLevel(previous_frame, frame_sampler,\n            clamp(history_uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0)');
    return device.createShaderModule({ label: `Shared amara worlds ${mask}`, code });
  };
  const buildPipeline = mask => {
    if (!pipelines.has(mask)) {
      const module = moduleFor(mask);
      const pending = (async () => {
        const info = await module.getCompilationInfo();
        const errors = info.messages.filter(message => message.type === 'error');
        if (errors.length) throw new Error(errors.map(message => `${message.lineNum}:${message.linePos} ${message.message}`).join('; '));
        return device.createRenderPipelineAsync({
          layout: pipelineLayout,
          vertex: { module, entryPoint: 'vs_main' },
          fragment: { module, entryPoint: 'fs_main', targets: [{ format: 'rgba8unorm' }] },
          primitive: { topology: 'triangle-list' },
        });
      })();
      pipelines.set(mask, pending);
      // A transient failure must not poison this world pair indefinitely.
      pending.catch(() => { if (pipelines.get(mask) === pending) pipelines.delete(mask); });
    }
    return pipelines.get(mask);
  };
  function warmManualPipelines() {
    const masks = manualPipelineMasks(WORLDS.map(([id]) => id), worldMask);
    let index = 0;
    const warmNext = () => {
      if (deviceLost || index >= masks.length) return;
      const mask = masks[index++];
      void buildPipeline(mask).catch(() => {}).finally(() => setTimeout(warmNext, 40));
    };
    setTimeout(warmNext, 250);
  }
  const presentModule = moduleFor(0);
  const presentInfo = await presentModule.getCompilationInfo();
  const presentErrors = presentInfo.messages.filter(message => message.type === 'error');
  if (presentErrors.length) throw new Error(presentErrors.map(message => `${message.lineNum}:${message.linePos} ${message.message}`).join('; '));
  const presentPipeline = await device.createRenderPipelineAsync({
    layout: pipelineLayout,
    vertex: { module: presentModule, entryPoint: 'vs_main' },
    fragment: { module: presentModule, entryPoint: 'fs_present', targets: [{ format }] },
    primitive: { topology: 'triangle-list' },
  });
  let current = WORLDS[Math.floor(Math.random() * WORLDS.length)][0];
  const bag = createWorldBag();
  const mandelbrot = createMandelbrotJourney();
  const mathJourney = createMathJourney();
  let currentPipeline = await buildPipeline(worldMask(current));
  let from = current;
  let next = null;
  let meltStartWall = 0;
  let melt = 1;
  let worldStart = 0;
  let journeyWorldStartWall = performance.now() / 1000;
  const pendingChoice = createSelectionGate();
  const journeyController = createJourneyController();
  const journeySelectionGuard = createJourneySelectionGuard();
  let last = 0;
  let frameDeadline = 0;
  let lastPlaying = false;
  let lastAudioError = '';
  let nextInsightAnnouncement = 0;
  let elapsed = 0;
  let wallElapsed = performance.now() / 1000;
  let travel = 0;
  let history = [];
  let historyViews = [];
  let historyBindings = [];
  let width = 0;
  let height = 0;
  let currentHistory = 0;
  let hasHistory = false;
  const lookJourney = createLookJourney(Array.from({ length: WORLD_COUNT }, () => Math.floor(Math.random() * 10)));
  const mandelbrotRegions = createWorldBag();
  const mandelbrotChoices = Array.from({ length: 10 }, (_, index) => index);
  const variationMode = createVariationMode();
  warmManualPipelines();
  function refreshVariation() {
    variationSelector.replaceChildren();
    if (selector.value === 'auto') {
      variationSelector.add(new Option('Auto chooses variations', ''));
      variationSelector.disabled = true;
      return;
    }
    const id = Number(selector.value);
    if (!LOOK_NAMES[id]) {
      variationSelector.add(new Option('World unavailable', ''));
      variationSelector.disabled = true;
      return;
    }
    variationSelector.add(new Option('Auto · vary this world', 'auto'));
    for (const [index, title] of LOOK_NAMES[id].entries()) variationSelector.add(new Option(title, String(index)));
    const look = lookJourney.state(id, elapsed);
    variationSelector.value = variationMode.isAutomatic() ? 'auto' : String(look.queued ?? look.to);
    variationSelector.disabled = false;
  }
  variationSelector.addEventListener('change', () => {
    if (selector.value === 'auto') return;
    if (journeyController.state.active) pauseJourney('manual variation');
    const id = Number(selector.value);
    variationMode.choose(variationSelector.value, elapsed);
    if (variationMode.isAutomatic()) {
      lookJourney.cancelQueued(); // Preserve the currently visible morph, discard only its queued manual choice.
      setStatus(`Auto variations · ${WORLDS.find(([world]) => world === id)?.[1] || 'selected world'}`);
      return;
    }
    const look = Number(variationSelector.value);
    lookJourney.request(id, look, elapsed);
    const state = lookJourney.state(id, elapsed);
    setStatus(state.queued === null ? `Changing variation to ${LOOK_NAMES[id][look]}` : `Variation queued: ${LOOK_NAMES[id][look]}`);
  });
  const impacts = Array.from({ length: 12 }, () => [0, 0, 0, 0]);
  let impactHead = 0;
  let lastImpact = -10;
  const uniforms = new Float32Array(280);
  const mappedSpectrum = new Float32Array(48);
  let appliedProfile = defaultProfile();
  const efficientGovernor = createEfficientGovernor();
  const musicInsight = createMusicInsight();
  let latestInsight = musicInsight.state;
  // Preserve the existing browser preview's default look, including Glass.
  appliedProfile.visual.glow = 0.75;
  appliedProfile.visual.warp = 0.5;
  appliedProfile.visual.feedback = 0.2;
  appliedProfile.mappings.beat_to_flash = 1;
  paletteInput.addEventListener('change', () => {
    appliedProfile.visual.palette = paletteInput.value;
    if (journeyController.state.active) {
      journeyController.customize('palette');
      syncJourneyStatus();
    }
  });
  paceInput.addEventListener('change', () => {
    if (journeyController.state.active) {
      journeyController.customize('pace');
      syncJourneyStatus();
    }
  });
  graphicsInput.addEventListener('change', () => {
    appliedProfile.visual.graphics_quality = graphicsInput.value;
    efficientGovernor.reset();
    frameDeadline = 0;
  });
  function syncJourneyStatus() {
    const state = journeyController.state;
    journeyInput.value = state.selected;
    journeyStatus.textContent = describeJourney(state, current);
  }
  function pauseJourney(reason) {
    journeyController.pause(reason);
    syncJourneyStatus();
  }
  function activateJourney(id, { applyDefaults = true } = {}) {
    journeySelectionGuard.run(() => {
      const journey = journeyDefinition(id);
      journeyController.choose(id);
      pendingChoice.cancel();
      selector.value = 'auto';
      bag.reset();
      worldStart = elapsed;
      journeyWorldStartWall = performance.now() / 1000;
      if (applyDefaults) {
        appliedProfile.visual.palette = journey.palette;
        paletteInput.value = journey.palette;
        paceInput.value = String(journey.pace);
      }
      refreshVariation();
      syncJourneyStatus();
    });
  }
  function handleJourneySelection() {
    const requestedJourney = journeyInput.value;
    if (requestedJourney === 'Manual') pauseJourney('manual control');
    else if (journeyController.state.selected === requestedJourney && journeyController.state.active) syncJourneyStatus();
    else activateJourney(requestedJourney);
  }
  journeyInput.addEventListener('input', handleJourneySelection);
  journeyInput.addEventListener('change', handleJourneySelection);
  const initialJourney = journeyController.state.selected;
  if (initialJourney === 'Manual') syncJourneyStatus();
  else if (initialJourney === 'Surprise') {
    journeyController.customize('current palette and pace');
    syncJourneyStatus();
  }
  else activateJourney(initialJourney);
  let profileStore = null;
  function refreshProfiles(selected = '') {
    profileList.replaceChildren(new Option('Choose saved profile', ''));
    if (!profileStore) return;
    for (const name of profileStore.list()) profileList.add(new Option(name, name));
    profileList.value = selected;
  }
  try { profileStore = createProfileStore(); refreshProfiles(); }
  catch (error) { setStatus(`Profiles unavailable: ${describeError(error)}`); }
  function applyProfile(profile) {
    if (!WORLDS.some(([id]) => id === profile.visual.scene)) throw new Error('This world is not available on this browser yet');
    appliedProfile = structuredClone(profile);
    paletteInput.value = appliedProfile.visual.palette;
    graphicsInput.value = appliedProfile.visual.graphics_quality;
    efficientGovernor.reset();
    frameDeadline = 0;
    variationMode.choose('manual', elapsed); // The Mac profile stores a specific look, not Auto.
    paceInput.value = String(profile.visual.motion);
    intensityInput.value = String(profile.visual.intensity);
    detailInput.value = String(profile.visual.complexity);
    bassInput.value = String(profile.mappings.bass_to_pulse);
    midsInput.value = String(profile.mappings.mids_to_geometry);
    highsInput.value = String(profile.mappings.treble_to_sparkle);
    const id = profile.visual.scene;
    selector.value = String(id);
    selector.dispatchEvent(new Event('change'));
    lookJourney.request(id, profile.visual.look, elapsed);
    refreshVariation();
    profileName.value = profile.name;
    setStatus(`Loaded ${profile.name} · audio source unchanged`);
  }
  document.querySelector('#profile-save').addEventListener('click', () => {
    try {
      if (!profileStore) throw new Error('Browser storage is unavailable');
      const name = profileName.value.trim();
      profileStore.save(snapshotProfile({ base: appliedProfile, name, selection: selector.value,
        current, next, lookTargets: Array.from({ length: WORLD_COUNT }, (_, id) => {
          const look = lookJourney.state(id, elapsed);
          return look.queued ?? look.to;
        }), controls: { pace: paceInput, intensity: intensityInput,
          detail: detailInput, graphics: graphicsInput, bass: bassInput, mids: midsInput, highs: highsInput } }));
      refreshProfiles(name);
      setStatus(`Saved ${name} on this device`);
    } catch (error) { setStatus(`Profile: ${describeError(error)}`); }
  });
  document.querySelector('#profile-load').addEventListener('click', () => {
    try { if (!profileStore || !profileList.value) throw new Error('Choose a saved profile'); applyProfile(profileStore.load(profileList.value)); }
    catch (error) { setStatus(`Profile: ${describeError(error)}`); }
  });
  const deleteConfirm = document.querySelector('#profile-delete-confirm');
  const deleteQuestion = document.querySelector('#profile-delete-question');
  const deleteConfirmButton = document.querySelector('#profile-delete-confirm-button');
  let pendingProfileDeletion = null;
  function cancelProfileDeletion() {
    pendingProfileDeletion = null;
    deleteConfirm.hidden = true;
  }
  document.querySelector('#profile-delete').addEventListener('click', () => {
    try {
      if (!profileStore || !profileList.value) throw new Error('Choose a saved profile');
      pendingProfileDeletion = profileList.value;
      deleteQuestion.textContent = `Delete “${pendingProfileDeletion}” from this browser?`;
      deleteConfirm.hidden = false;
      deleteConfirmButton.focus();
    } catch (error) { setStatus(`Profile: ${describeError(error)}`); }
  });
  document.querySelector('#profile-delete-cancel').addEventListener('click', cancelProfileDeletion);
  profileList.addEventListener('change', cancelProfileDeletion);
  deleteConfirmButton.addEventListener('click', () => {
    try {
      if (!profileStore || !pendingProfileDeletion) throw new Error('Choose a saved profile');
      const name = pendingProfileDeletion;
      profileStore.remove(name);
      cancelProfileDeletion();
      refreshProfiles();
      setStatus(`Deleted ${name} from this browser`);
    } catch (error) { setStatus(`Profile: ${describeError(error)}`); }
  });
  profileImport.addEventListener('change', async () => {
    const file = profileImport.files?.[0];
    if (!file) return;
    try {
      if (!profileStore) throw new Error('Browser storage is unavailable');
      if (file.size > 64 * 1024) throw new Error('Profile must be no larger than 64 KiB');
      const profile = profileStore.importText(await file.text());
      refreshProfiles(profile.name);
      setStatus(`Imported ${profile.name}; choose Load to apply it`);
    } catch (error) { setStatus(`Profile: ${describeError(error)}`); }
    finally { profileImport.value = ''; }
  });
  document.querySelector('#profile-export').addEventListener('click', () => {
    try {
      if (!profileStore || !profileList.value) throw new Error('Choose a saved profile');
      const text = profileStore.exportText(profileList.value);
      const url = URL.createObjectURL(new Blob([text], { type: 'application/json' }));
      const link = document.createElement('a');
      link.href = url;
      link.download = `${profileList.value}.json`;
      link.click();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
      setStatus(`Exported ${profileList.value}`);
    } catch (error) { setStatus(`Profile: ${describeError(error)}`); }
  });
  let favoriteStore = null;
  function refreshFavorites(selected = '') {
    favoriteList.replaceChildren(new Option('Choose favorite', ''));
    if (!favoriteStore) return;
    for (const name of favoriteStore.list()) favoriteList.add(new Option(name, name));
    favoriteList.value = selected;
  }
  function currentRecipe() {
    const world = next ?? current;
    const look = lookJourney.state(world, elapsed);
    const journey = journeyController.state;
    return createRecipe({
      format_version: 1,
      journey: { id: journey.active ? journey.selected : 'Manual', customized: journey.active && journey.customized },
      visual: { world, selection: selector.value === 'auto' ? 'auto' : 'manual', look: look.queued ?? look.to,
        palette: paletteInput.value, graphics_quality: graphicsInput.value,
        pace: Number(paceInput.value), intensity: Number(intensityInput.value), detail: Number(detailInput.value) },
      mappings: { bass: Number(bassInput.value), mids: Number(midsInput.value),
        highs: Number(highsInput.value), beat: appliedProfile.mappings.beat_to_flash },
    });
  }
  function applyVisualRecipe(input) {
    const plan = recipeApplication(input);
    appliedProfile.visual.palette = plan.visual.palette;
    appliedProfile.visual.graphics_quality = plan.visual.graphics_quality;
    appliedProfile.mappings.bass_to_pulse = plan.mappings.bass;
    appliedProfile.mappings.mids_to_geometry = plan.mappings.mids;
    appliedProfile.mappings.treble_to_sparkle = plan.mappings.highs;
    appliedProfile.mappings.beat_to_flash = plan.mappings.beat;
    paletteInput.value = plan.visual.palette;
    graphicsInput.value = plan.visual.graphics_quality;
    paceInput.value = String(plan.visual.pace);
    intensityInput.value = String(plan.visual.intensity);
    detailInput.value = String(plan.visual.detail);
    bassInput.value = String(plan.mappings.bass);
    midsInput.value = String(plan.mappings.mids);
    highsInput.value = String(plan.mappings.highs);
    efficientGovernor.reset();
    frameDeadline = 0;

    if (plan.journey.id === 'Manual') {
      pauseJourney('visual recipe');
      selector.value = plan.visual.selection === 'auto' ? 'auto' : String(plan.visual.world);
      variationMode.choose(plan.visual.selection === 'manual' ? 'manual' : 'auto', elapsed);
      refreshVariation();
    } else {
      activateJourney(plan.journey.id, { applyDefaults: false });
      if (plan.journey.customized) journeyController.customize('recipe settings');
      variationMode.choose('auto', elapsed);
      syncJourneyStatus();
    }
    lookJourney.request(plan.visual.world, plan.visual.look, elapsed);
    void chooseManualWorld(plan.visual.world);
    refreshVariation();
    recipeStatus.textContent = 'Visual recipe applied · audio source unchanged';
  }
  async function copyRecipe(value, label) {
    if (!navigator.clipboard?.writeText) throw new Error('Clipboard access is unavailable in this browser');
    await navigator.clipboard.writeText(value);
    recipeStatus.textContent = label;
  }
  function importedRecipe(text) {
    const value = text.trim();
    if (!value) throw new Error('Paste a recipe link or recipe text first');
    if (/^https?:\/\//iu.test(value)) {
      const recipe = recipeFromHash(new URL(value).hash);
      if (!recipe) throw new Error('This link does not contain a visual recipe');
      return recipe;
    }
    return parseRecipeText(value);
  }
  try { favoriteStore = createFavoriteStore(); refreshFavorites(); }
  catch (error) { recipeStatus.textContent = `Favorites unavailable: ${describeError(error)}`; }
  document.querySelector('#favorite-save').addEventListener('click', () => {
    try {
      if (!favoriteStore) throw new Error('Browser storage is unavailable');
      const name = favoriteName.value.trim() || `Favorite ${favoriteStore.list().length + 1}`;
      favoriteStore.save(name, currentRecipe());
      favoriteName.value = name;
      refreshFavorites(name);
      recipeStatus.textContent = `Favorited ${name} in this browser`;
    } catch (error) { recipeStatus.textContent = `Favorite: ${describeError(error)}`; }
  });
  document.querySelector('#favorite-load').addEventListener('click', () => {
    try {
      if (!favoriteStore || !favoriteList.value) throw new Error('Choose a favorite');
      applyVisualRecipe(favoriteStore.load(favoriteList.value));
    } catch (error) { recipeStatus.textContent = `Favorite: ${describeError(error)}`; }
  });
  document.querySelector('#favorite-remove').addEventListener('click', () => {
    try {
      if (!favoriteStore || !favoriteList.value) throw new Error('Choose a favorite');
      const name = favoriteList.value;
      favoriteStore.remove(name);
      refreshFavorites();
      recipeStatus.textContent = `Removed ${name} from this browser`;
    } catch (error) { recipeStatus.textContent = `Favorite: ${describeError(error)}`; }
  });
  document.querySelector('#recipe-copy-text').addEventListener('click', () => {
    try {
      void copyRecipe(recipeText(currentRecipe()), 'Recipe text copied · visual settings only').catch(error => {
        recipeStatus.textContent = `Recipe: ${describeError(error)}`;
      });
    } catch (error) { recipeStatus.textContent = `Recipe: ${describeError(error)}`; }
  });
  document.querySelector('#recipe-copy-link').addEventListener('click', () => {
    try {
      void copyRecipe(recipeLink(currentRecipe(), window.location.href),
        'Recipe link copied · it opens only where this localhost app is running').catch(error => {
        recipeStatus.textContent = `Recipe: ${describeError(error)}`;
      });
    } catch (error) { recipeStatus.textContent = `Recipe: ${describeError(error)}`; }
  });
  document.querySelector('#recipe-apply').addEventListener('click', () => {
    try { applyVisualRecipe(importedRecipe(recipeImport.value)); }
    catch (error) { recipeStatus.textContent = `Recipe: ${describeError(error)}`; }
  });
  try {
    const linkedRecipe = recipeFromHash(window.location.hash);
    if (linkedRecipe) {
      recipeImport.value = recipeText(linkedRecipe);
      recipeStatus.textContent = 'Recipe link ready · review it, then choose Review & apply';
    }
  } catch (error) { recipeStatus.textContent = `Recipe link: ${describeError(error)}`; }
  function bindings(view) {
    return device.createBindGroup({ layout, entries: [
      { binding: 0, resource: { buffer: uniform } },
      { binding: 1, resource: view },
      { binding: 2, resource: sampler },
      { binding: 3, resource: rasterView },
    ] });
  }
  function resize() {
    const quality = appliedProfile.visual.graphics_quality;
    const transitioning = next !== null;
    const destination = next ?? current;
    const budget = graphicsBudget(quality,
      canvas.clientWidth, canvas.clientHeight, devicePixelRatio || 1,
      from, destination, transitioning, efficientGovernor.step);
    const decision = graphicsDecision(quality, from, destination, transitioning, efficientGovernor.step);
    let graphicsLabel;
    if (quality === 'Auto') {
      const reason = decision.reason === 'transition' ? 'during transitions'
        : decision.reason === 'world' ? 'for this demanding world'
          : decision.reason === 'load' ? 'after sustained graphics load' : 'as the normal baseline';
      const tier = decision.quality === 'Low' ? 'Efficient' : 'Balanced';
      const floor = decision.quality === 'Low' ? ' · artwork resolution preserved' : '';
      graphicsLabel = `Automatic · ${tier} ${reason} · ${budget.fps} fps target${floor}`;
    } else {
      const names = { Low: 'Efficient', Balanced: 'Balanced', High: 'High Detail' };
      graphicsLabel = `Manual · ${names[quality]} · ${budget.fps} fps target`;
    }
    if (graphicsStatus.textContent !== graphicsLabel) graphicsStatus.textContent = graphicsLabel;
    const { width: w, height: h } = budget;
    if (w === width && h === height) return budget.fps;
    width = canvas.width = w;
    height = canvas.height = h;
    for (const texture of history) texture.destroy();
    history = [0, 1].map(() => device.createTexture({ size: [w, h], format: 'rgba8unorm', usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING }));
    rasterTexture.destroy();
    rasterTexture = device.createTexture({ size: [w, h, 5], format: 'rgba16float', usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.TEXTURE_BINDING });
    rasterView = rasterTexture.createView({ dimension: '2d-array' });
    geometry?.setTexture(rasterTexture, w, h);
    historyViews = history.map(texture => texture.createView());
    historyBindings = historyViews.map(view => bindings(view));
    currentHistory = 0;
    hasHistory = false;
    return budget.fps;
  }
  async function chooseManualWorld(id) {
    if (manualTarget === id && pendingChoice.busy()) return;
    const plan = manualSwitchPlan(current, next, melt, id);
    pendingChoice.cancel();
    current = plan.anchor;
    from = current;
    next = null;
    melt = 1;
    worldStart = elapsed;
    journeyWorldStartWall = performance.now() / 1000;
    if (id === current) {
      syncJourneyStatus();
      return;
    }
    manualTarget = id;
    const token = pendingChoice.begin();
    try {
      // A single-world pipeline is much smaller than a new source/destination
      // pair and is warmed in the background, so explicit choices do not wait
      // for an uncached cinematic morph shader.
      const title = WORLDS.find(([world]) => world === id)?.[1] || 'selected world';
      setStatus(`Preparing ${title}…`);
      const pipeline = await buildPipeline(worldMask(id));
      if (!pendingChoice.current(token) || (selector.value !== 'auto' && Number(selector.value) !== id)) return;
      current = id;
      from = id;
      next = null;
      melt = 1;
      worldStart = elapsed;
      journeyWorldStartWall = performance.now() / 1000;
      currentPipeline = pipeline;
      hasHistory = false;
      syncJourneyStatus();
      setStatus(`${title} · switched`);
    } catch (error) {
      if (pendingChoice.current(token)) {
        failedWorlds.add(id);
        setStatus(`Could not prepare world: ${describeError(error)}`);
      }
    } finally {
      pendingChoice.finish(token);
      if (manualTarget === id) manualTarget = null;
    }
  }
  async function chooseWorld(id) {
    if (id === current || next !== null || pendingChoice.busy()) return;
    const token = pendingChoice.begin();
    try {
      const pipeline = await buildPipeline(worldMask(current) | worldMask(id));
      if (!pendingChoice.current(token) || (selector.value !== 'auto' && Number(selector.value) !== id)) return;
      from = current;
      next = id;
      if (id === 10 && current !== 10) {
        mandelbrot.reset();
        if (variationMode.allowsMandelbrotRegion(selector.value)) {
          const look = lookJourney.state(10, elapsed);
          const region = mandelbrotRegions.next(mandelbrotChoices, look.to);
          if (region !== null) lookJourney.request(10, region, elapsed);
        }
      }
      currentPipeline = pipeline;
      meltStartWall = performance.now() / 1000;
      melt = 0;
      setStatus(`Transforming into ${WORLDS.find(([world]) => world === id)[1]}`);
    } catch (error) {
      if (pendingChoice.current(token)) {
        failedWorlds.add(id);
        setStatus(`Could not prepare world: ${describeError(error)}`);
      }
    } finally {
      pendingChoice.finish(token);
    }
  }
  let manualTarget = null;
  const worldSelectionEvents = createSelectionEventGuard();
  function handleWorldSelection(event) {
    if (!worldSelectionEvents.handle(event.type, selector.value)) return;
    if (journeySelectionGuard.consumeAuto(selector.value)) {
      refreshVariation();
      return;
    }
    // An asynchronous compile can finish after the user's choice changes.
    // Invalidate it without tearing down an already visible transition.
    variationMode.worldChanged(elapsed);
    if (selector.value !== 'auto') {
      if (journeyController.state.active) pauseJourney('manual world');
      const selected = Number(selector.value);
      failedWorlds.delete(selected);
      void chooseManualWorld(selected);
    }
    else {
      if (!journeyController.state.active) {
        activateJourney('Surprise', { applyDefaults: false });
        journeyController.customize('current palette and pace');
        syncJourneyStatus();
      }
      worldStart = elapsed;
      journeyWorldStartWall = performance.now() / 1000;
      bag.reset();
      lookJourney.cancelQueued();
    }
    refreshVariation();
  }
  selector.addEventListener('input', handleWorldSelection);
  selector.addEventListener('change', handleWorldSelection);
  function draw(timestamp) {
    if (deviceLost) return;
    wallElapsed = timestamp / 1000;
    const fps = resize();
    const pacing = graphicsFrame(timestamp, frameDeadline, fps);
    frameDeadline = pacing.deadline;
    if (!pacing.due) { requestAnimationFrame(draw); return; }
    const realDt = last ? clamp((timestamp - last) / 1000, 0, 0.25) : 0;
    const dt = Math.min(realDt, 0.05);
    last = timestamp;
    elapsed += dt;
    const features = audio.getFeatures();
    const audioState = audio.getState();
    if (audioState.error && audioState.error !== lastAudioError) {
      setStatus(`Audio: ${audioState.error}`);
    } else if (lastPlaying && !audioState.playing && audioState.source !== 'mac') {
      setStatus(audioState.source === 'file' ? `Paused or finished · ${audioState.fileName}` : 'Listening stopped');
    }
    lastPlaying = audioState.playing;
    lastAudioError = audioState.error;
    const { bass, mids, treble, energy, beat, spectrum } = features;
    const insight = musicInsight.update({ active: audioState.playing, source: audioState.source,
      energy, bass, mids, treble }, timestamp / 1000);
    latestInsight = insight;
    if (insight.event !== 'none' || timestamp >= nextInsightAnnouncement) {
      const label = describeMusicInsight(insight);
      if (musicInsightStatus.textContent !== label) musicInsightStatus.textContent = label;
      nextInsightAnnouncement = timestamp + 1000;
    }
    const basePace = Number(paceInput.value);
    const baseIntensity = Number(intensityInput.value);
    const adjustments = musicVisualAdjustments(insight);
    const insightPace = basePace * adjustments.paceScale;
    const insightIntensity = baseIntensity * adjustments.intensityScale;
    const insightGlow = clamp(appliedProfile.visual.glow + adjustments.glowOffset, 0, 1.6);
    travel += dt * insightPace * 0.55;
    const destination = next ?? current;
    if (next !== null) {
      melt = wallClockProgress(meltStartWall, wallElapsed, AUTO_WORLD_TRANSITION_SECONDS);
      if (melt >= 1) {
        current = next;
        next = null;
        from = current;
        worldStart = elapsed;
        journeyWorldStartWall = wallElapsed;
        syncJourneyStatus();
        setStatus(`${WORLDS.find(([id]) => id === current)[1]} · shared native shader`);
      }
    } else if (selector.value === 'auto' && journeyController.state.active &&
        journeyShouldAdvance({ id: journeyController.state.selected, world: current,
          age: wallElapsed - journeyWorldStartWall, insight: latestInsight })) {
      if (!pendingChoice.busy()) {
        const available = WORLDS.map(([id]) => id).filter(id => !failedWorlds.has(id));
        const choices = journeyWorlds(journeyController.state.selected, available);
        const selected = bag.next(choices, current);
        if (selected !== null) chooseWorld(selected);
      }
    } else if (selector.value !== 'auto' && Number(selector.value) !== current && !failedWorlds.has(Number(selector.value))) {
      void chooseManualWorld(Number(selector.value));
    }
    // Original uniform layout from src/params.wgsl. Timing remains continuous;
    // only input envelopes and selection are adapted for the browser.
    uniforms.fill(0);
    uniforms.set([width, height, travel, destination], 0);
    uniforms.set([bass, mids, treble, energy], 4);
    uniforms.set([beat, insightIntensity, insightPace, Number(detailInput.value)], 8);
    uniforms.set([appliedProfile.visual.hue, insightGlow, appliedProfile.visual.warp, paletteAmount(appliedProfile.visual.palette)], 12);
    uniforms.set([Number(bassInput.value), Number(midsInput.value), Number(highsInput.value), appliedProfile.mappings.beat_to_flash], 16);
    uniforms.set([treble, energy, from, melt], 20);
    uniforms.set([appliedProfile.visual.feedback, dt, hasHistory ? 1 : 0, 0], 24);
    uniforms.set([0.35 + energy * 0.4, 0.75, treble * 0.45, beat * 0.5], 28);
    uniforms.set([elapsed * 0.15, 1, 0.3, 0.27], 32);
    lookJourney.advance(elapsed);
    const currentLook = lookJourney.state(current, elapsed);
    if (variationMode.shouldAdvance({ selection: selector.value, current, next, worldStart, look: currentLook, now: elapsed })) {
      lookJourney.request(current, (currentLook.to + 1 + Math.floor(Math.random() * 9)) % 10, elapsed);
    }
    const math = mathJourney.update(features, {
      bass: Number(bassInput.value), mids: Number(midsInput.value),
      treble: Number(highsInput.value), beat: appliedProfile.mappings.beat_to_flash,
    }, appliedProfile.visual.math_form, dt);
    const mathNames = ['Julia', 'Inversion Lace', 'Harmonic Sculpture', 'Vortex Weave'];
    const mathLabel = math.blend < 1 && math.from !== math.to
      ? `${mathNames[math.from]} → ${mathNames[math.to]}` : mathNames[math.to];
    const name = id => WORLDS.find(([world]) => world === id)?.[1] || `World ${id}`;
    const variation = id => {
      const look = lookJourney.state(id, elapsed);
      const start = LOOK_NAMES[id]?.[look.from] || `Variation ${look.from + 1}`;
      const target = LOOK_NAMES[id]?.[look.to] || `Variation ${look.to + 1}`;
      const active = look.from !== look.to && look.blend < 1 ? `${start} → ${target}` : target;
      return look.queued === null ? active : `${active} · queued: ${LOOK_NAMES[id][look.queued]}`;
    };
    const journeyActive = selector.value === 'auto' && journeyController.state.active;
    const duration = journeyActive ? journeyDwell(journeyController.state.selected, current) : 1;
    if (next !== null) {
      sceneState.textContent = `${name(from)} → ${name(next)} · ${variation(next)}${next === 1 ? ` · ${mathLabel}` : ''}`;
      sceneProgress.value = melt;
      sceneProgress.setAttribute('aria-label', 'World transformation progress');
    } else {
      sceneState.textContent = `${name(current)} · ${variation(current)}${current === 1 ? ` · ${mathLabel}` : ''}`;
      sceneProgress.value = journeyActive ? clamp((wallElapsed - journeyWorldStartWall) / duration, 0, 1) : 0;
      sceneProgress.hidden = !journeyActive;
      sceneProgress.setAttribute('aria-label', 'Time until next world');
    }
    if (next !== null) sceneProgress.hidden = false;
    uniforms.set([math.from, math.to, math.blend, math.phase], 36);
    for (let i = 0; i < 48; i++) {
      const gain = i < 8 ? Number(bassInput.value) : i < 24 ? Number(midsInput.value) : Number(highsInput.value);
      mappedSpectrum[i] = clamp((spectrum[i] || 0) * gain, 0, 1.5);
      uniforms[40 + i] = mappedSpectrum[i];
    }
    for (let i = 0; i < WORLD_COUNT; i++) {
      const look = lookJourney.state(i, elapsed);
      uniforms.set([look.from, look.to, look.blend, elapsed], 88 + i * 4);
    }
    const playing = audioState.playing;
    const geometryState = geometry?.update(mappedSpectrum, realDt, playing);
    uniforms.set([geometryState?.head || 0, geometryState?.fraction || 0, playing ? 1 : 0, 0], 156);
    uniforms.set([0, 1, 0, 0], 160);
    uniforms.set([energy, beat, bass, treble], 164);
    for (const impact of impacts) if (impact[3] > 0) impact[2] += dt;
    if (playing && beat > 0.2 && elapsed - lastImpact > 0.5) {
      impacts[impactHead] = [(Math.random() - 0.5) * 1.2, (Math.random() - 0.5) * 1.0, 0, beat];
      // Native water reserves the first four impacts for bass onsets.
      impactHead = (impactHead + 1) % 4;
      lastImpact = elapsed;
    }
    for (let i = 0; i < impacts.length; i++) uniforms.set(impacts[i], 168 + i * 4);
    for (let i = 0; i < 8; i++) uniforms[268 + i] = uniforms[40 + i * 6];
    uniforms[276] = Math.sin(elapsed * 0.035) * 0.12;
    if (from === 10 || destination === 10) {
      const exposure = from === 10 && destination === 10 ? 1 : destination === 10 ? melt : 1 - melt;
      const mandelbrotLook = lookJourney.state(10, elapsed);
      const journey = mandelbrot.advance(dt * exposure, mandelbrotLook.from);
      uniforms[277] = journey.age;
      if (journey.newRegion && variationMode.allowsMandelbrotRegion(selector.value)) {
        const region = mandelbrotRegions.next(mandelbrotChoices, mandelbrotLook.to);
        if (region !== null) lookJourney.request(10, region, elapsed);
      }
    }
    uniforms[278] = 1;
    device.queue.writeBuffer(uniform, 0, uniforms);
    const target = 1 - currentHistory;
    const encoder = device.createCommandEncoder();
    geometry?.render(encoder, from, destination, melt);
    const drawPass = encoder.beginRenderPass({ colorAttachments: [{ view: historyViews[target], loadOp: 'clear', storeOp: 'store', clearValue: [0, 0, 0, 1] }] });
    drawPass.setPipeline(currentPipeline);
    drawPass.setBindGroup(0, historyBindings[currentHistory]);
    drawPass.draw(3);
    drawPass.end();
    const present = encoder.beginRenderPass({ colorAttachments: [{ view: context.getCurrentTexture().createView(), loadOp: 'clear', storeOp: 'store', clearValue: [0, 0, 0, 1] }] });
    present.setPipeline(presentPipeline);
    present.setBindGroup(0, historyBindings[target]);
    present.draw(3);
    present.end();
    device.queue.submit([encoder.finish()]);
    if ((appliedProfile.visual.graphics_quality === 'Low' || appliedProfile.visual.graphics_quality === 'Auto') &&
        efficientGovernor.shouldSample(timestamp)) {
      const sampleStart = performance.now();
      try {
        device.queue.onSubmittedWorkDone().then(() => {
          const now = performance.now();
          if (deviceLost || !['Low', 'Auto'].includes(appliedProfile.visual.graphics_quality)) {
            efficientGovernor.cancel(now);
          } else if (efficientGovernor.observe(now - sampleStart, now)) {
            frameDeadline = 0;
          }
        }, () => efficientGovernor.cancel(performance.now()));
      } catch {
        efficientGovernor.cancel(performance.now());
      }
    }
    currentHistory = target;
    hasHistory = true;
    for (let i = 0; i < bars.length; i++) {
      const bar = bars[i];
      const value = meterLevel(spectrum[i] || 0);
      if (value >= bar.value) { bar.value = value; bar.until = elapsed + 0.75; }
      else if (elapsed > bar.until) bar.value = Math.max(value, bar.value - dt * 0.32);
      bar.fill.style.transform = `scaleY(${value})`;
      bar.peak.style.bottom = meterPeakBottom(bar.value);
    }
    syncAudioButtons();
    requestAnimationFrame(draw);
  }
  const initialAudio = audio.getState();
  if (initialAudio.source !== 'mac' && !initialAudio.connecting) {
    setStatus(`${WORLDS.find(([id]) => id === current)[1]} · add music to begin`);
  }
  requestAnimationFrame(draw);
  createGeometryPass({ device, uniformBuffer: uniform, rasterTexture }).then(pass => {
    geometry = pass;
    geometry.setTexture(rasterTexture, width || 1, height || 1);
    const additions = [[4, 'Particle Constellation'], [5, 'Strange Attractor'],
      [6, 'Spectral Landscape'], [7, 'Dot Equaliser'], [12, 'Disco Silk']];
    WORLDS.push(...additions);
    WORLDS.sort((a, b) => a[0] - b[0]);
    for (const [id, title] of additions) selector.add(new Option(title, String(id)));
  }).catch(async error => {
    const reason = describeError(error);
    try {
      const dots = await createDotPass({ device, uniformBuffer: uniform, rasterTexture });
      dots.setTexture(rasterTexture);
      geometry = {
        setTexture: texture => dots.setTexture(texture),
        update: (spectrum, dt, fresh) => dots.update(spectrum, dt, fresh),
        render: (encoder, from, destination, melt) => {
          if (destination === 7 || (melt < 1 && from === 7)) dots.render(encoder);
        },
        destroy: () => dots.destroy?.(),
      };
      WORLDS.push([7, 'Dot Equaliser']);
      WORLDS.sort((a, b) => a[0] - b[0]);
      selector.add(new Option('Dot Equaliser', '7'));
      setStatus(`Some geometry worlds unavailable: ${reason}. Dot Equaliser remains available.`);
    } catch (fallbackError) {
      setStatus(`Geometry worlds unavailable: ${reason}; Dot Equaliser: ${describeError(fallbackError)}`);
    }
  });
}

start().catch(error => setStatus(`Visualiser could not start: ${describeError(error)}`));
