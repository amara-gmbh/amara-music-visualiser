import assert from 'node:assert/strict';
import { createAudioEngine } from '../audio.js';

const graphs = [];
let contexts = [];
let testAudio;
let streamRequest;
let displayRequest;
let lastMediaConstraints;
let lastDisplayConstraints;
let clock = 100;
Object.defineProperty(globalThis, 'performance', { value: { now: () => clock }, configurable: true });
class NodeMock {
  constructor(kind) { this.kind = kind; this.connections = []; }
  connect(node) { this.connections.push(node); graphs.push([this.kind, node.kind]); }
  disconnect() { this.connections.length = 0; }
}
class AudioMock {
  constructor() { this.paused = true; this.ended = false; this.listeners = {}; this.currentTime = 0; this.duration = 120; testAudio = this; }
  addEventListener(name, callback) { this.listeners[name] = callback; }
  pause() { this.paused = true; }
  play() { this.paused = false; this.ended = false; return Promise.resolve(); }
  load() {}
  removeAttribute() {}
}
class AudioContextMock {
  constructor() { this.state = 'running'; this.sampleRate = 48000; this.destination = new NodeMock('speaker'); contexts.push(this); }
  createAnalyser() {
    const analyser = new NodeMock('analyser');
    analyser.frequencyBinCount = 2048;
    analyser.getFloatFrequencyData = data => data.fill(-35);
    return analyser;
  }
  createMediaElementSource() { return new NodeMock('file'); }
  createMediaStreamSource() { return new NodeMock('capture'); }
  close() { this.state = 'closed'; return Promise.resolve(); }
}
class Track {
  constructor(kind) { this.kind = kind; this.readyState = 'live'; this.handlers = []; this.stops = 0; }
  addEventListener(name, handler) { if (name === 'ended') this.handlers.push(handler); }
  stop() { this.stops++; this.readyState = 'ended'; }
  end() { this.readyState = 'ended'; this.handlers.forEach(handler => handler()); }
}
function makeStream(kinds) {
  const tracks = kinds.map(kind => new Track(kind));
  return { tracks, getTracks: () => tracks, getAudioTracks: () => tracks.filter(track => track.kind === 'audio') };
}
globalThis.Audio = AudioMock;
globalThis.window = { AudioContext: AudioContextMock };
Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { mediaDevices: {
  getUserMedia: constraints => { lastMediaConstraints = constraints; return streamRequest(); },
  getDisplayMedia: constraints => { lastDisplayConstraints = constraints; return displayRequest(); },
} } });
URL.createObjectURL = () => 'blob:synthetic';
URL.revokeObjectURL = () => {};

const engine = createAudioEngine();
await engine.loadFile({ name: 'test.wav' });
await engine.togglePlayback();
assert.equal(engine.getState().source, 'file');
assert.equal(engine.getState().playing, true);
assert.ok(graphs.some(edge => edge.join('>') === 'file>speaker'));
assert.equal(engine.getState().duration, 120);
testAudio.duration = NaN;
assert.equal(engine.getState().duration, null, 'metadata may not be available immediately');
engine.seekFile(42);
assert.equal(engine.getState().currentTime, 0, 'seeking waits for a known duration');
testAudio.duration = 120;
engine.seekFile(42.5);
assert.equal(engine.getState().currentTime, 42.5, 'seeking changes the file position');
engine.seekFile(-10);
assert.equal(engine.getState().currentTime, 0, 'rewind is clamped to the start');
engine.seekFile(999);
assert.equal(engine.getState().currentTime, 120, 'forward seeking is clamped to the end');
engine.seekFile(NaN);
assert.equal(engine.getState().currentTime, 120, 'invalid positions are ignored');
engine.stopFile();
assert.equal(engine.getState().playing, false);
assert.equal(engine.getState().source, 'file', 'stop retains the selected file');
assert.equal(engine.getState().currentTime, 0, 'stop rewinds to the start');
await engine.togglePlayback();
assert.equal(engine.getState().playing, true, 'stopped file can be played again');

streamRequest = () => Promise.reject(Object.assign(new Error('denied'), { name: 'NotAllowedError' }));
await assert.rejects(engine.startMicrophone(), /not granted/);
assert.equal(engine.getState().source, 'file');
assert.equal(engine.getState().playing, true, 'denial must not stop current file');

const microphone = makeStream(['audio']);
streamRequest = () => Promise.resolve(microphone);
await engine.startMicrophone();
assert.deepEqual(lastMediaConstraints, { audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false }, video: false });
assert.equal(engine.getState().source, 'microphone');
assert.equal(engine.getState().playing, true);
engine.seekFile(30);
engine.stopFile();
assert.equal(engine.getState().source, 'microphone', 'file controls do not alter live input');
assert.equal(engine.getState().playing, true);
clock += 100;
assert.ok(engine.getFeatures().energy > 0, 'capture must drive measured features');
assert.ok(!graphs.some(edge => edge.join('>') === 'capture>speaker'), 'capture must never reach speakers');

const videoOnly = makeStream(['video']);
displayRequest = () => Promise.resolve(videoOnly);
await assert.rejects(engine.startSharedAudio(), /No audio was shared/);
assert.equal(videoOnly.tracks[0].readyState, 'ended');
assert.equal(engine.getState().source, 'microphone', 'failed sharing must preserve prior stream');
assert.equal(microphone.tracks[0].readyState, 'live');
assert.deepEqual(lastDisplayConstraints, { video: true, audio: true });

const shared = makeStream(['audio', 'video']);
displayRequest = () => Promise.resolve(shared);
await engine.startSharedAudio();
assert.equal(microphone.tracks[0].readyState, 'ended');
assert.equal(engine.getState().source, 'shared');
assert.ok(shared.tracks.every(track => track.readyState === 'live'));
shared.tracks[1].end();
assert.equal(engine.getState().source, 'none');
assert.equal(engine.getState().playing, false);
assert.match(engine.getState().error, /ended/);
assert.ok(shared.tracks.every(track => track.readyState === 'ended'));

let release;
streamRequest = () => new Promise(resolve => { release = resolve; });
const pending = engine.startMicrophone();
engine.stopCapture();
const abandoned = makeStream(['audio']);
release(abandoned);
await pending;
assert.equal(abandoned.tracks[0].readyState, 'ended', 'late capture must not leak after cancellation');
assert.equal(engine.getState().source, 'none');

await engine.togglePlayback();
assert.equal(engine.getState().source, 'file');
assert.equal(engine.getState().playing, true);
const finalMic = makeStream(['audio']);
streamRequest = () => Promise.resolve(finalMic);
await engine.startMicrophone();
engine.destroy();
assert.equal(finalMic.tracks[0].readyState, 'ended');
assert.equal(contexts[0].state, 'closed');
console.log('capture focused checks passed');
