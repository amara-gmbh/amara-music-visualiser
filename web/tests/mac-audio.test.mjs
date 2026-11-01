import assert from 'node:assert/strict';
import { createAudioEngine, localSystemAudioAvailable } from '../audio.js';

assert.equal(localSystemAudioAvailable('127.0.0.1'), true);
assert.equal(localSystemAudioAvailable('localhost'), true);
assert.equal(localSystemAudioAvailable('musicvisualiser.amarawebdesign.de'), false,
  'a public host must not advertise the desktop-only system-audio bridge');

let clock = 100;
Object.defineProperty(globalThis, 'performance', { configurable: true, value: { now: () => clock } });
const connections = [];
const nodes = [];
let moduleURL;
let abortSignal;
let fetchedURL;
let streamController;
class MockNode {
  constructor(kind) { this.kind = kind; this.gain = { value: 1 }; }
  connect(other) { connections.push([this, other]); }
  disconnect() {}
}
class MockAudio {
  constructor() { this.paused = true; this.ended = false; }
  addEventListener() {}
  pause() { this.paused = true; }
  play() { this.paused = false; return Promise.resolve(); }
  load() {}
  removeAttribute() {}
}
class MockContext {
  constructor() {
    this.state = 'running'; this.sampleRate = 48000; this.destination = new MockNode('speaker');
    this.audioWorklet = { addModule: async url => { moduleURL = url; } };
  }
  createAnalyser() { const n = new MockNode('analyser'); n.frequencyBinCount = 2048; n.getFloatFrequencyData = a => a.fill(-35); return n; }
  createMediaElementSource() { return new MockNode('file'); }
  createGain() { return new MockNode('gain'); }
  close() { this.state = 'closed'; return Promise.resolve(); }
}
class MockWorklet extends MockNode {
  constructor() { super('worklet'); this.messages = []; this.port = { postMessage: samples => this.messages.push([...samples]) }; nodes.push(this); }
}
globalThis.Audio = MockAudio;
globalThis.AudioWorkletNode = MockWorklet;
globalThis.window = { AudioContext: MockContext };
Object.defineProperty(globalThis, 'navigator', { configurable: true, value: { mediaDevices: {}, platform: 'Win32' } });
globalThis.fetch = async (url, options) => {
  fetchedURL = url;
  abortSignal = options.signal;
  const body = new ReadableStream({ start(controller) { streamController = controller; } });
  return { ok: true, body };
};
const tick = () => new Promise(resolve => setTimeout(resolve, 0));
const bytesFor = values => {
  const bytes = new Uint8Array(values.length * 4);
  const view = new DataView(bytes.buffer);
  values.forEach((value, index) => view.setFloat32(index * 4, value, true));
  return bytes;
};
const engine = createAudioEngine();
await engine.startSystemAudio();
assert.equal(fetchedURL, 'http://127.0.0.1:8769/api/system-audio/stream');
assert.match(moduleURL, /system-audio-worklet\.js/);
assert.equal(engine.getState().source, 'system');
assert.equal(engine.getState().systemSignal, 'waiting');
assert.equal(engine.getState().playing, false, 'connection alone is not a signal');
const gain = connections.find(([from, to]) => from.kind === 'analyser' && to.kind === 'gain')?.[1];
assert.equal(gain.gain.value, 0, 'captured audio must not reach speakers audibly');
streamController.enqueue(bytesFor([0, 0]));
await tick();
assert.equal(engine.getState().systemSignal, 'silent');
nodes[0].port.onmessage();
const samples = bytesFor([.5, -.25]);
streamController.enqueue(samples.slice(0, 3));
streamController.enqueue(samples.slice(3));
await tick();
assert.equal(engine.getState().systemSignal, 'live');
assert.deepEqual(nodes[0].messages.at(-1), [.5, -.25], 'split float bytes must be reassembled');
assert.ok(engine.getFeatures().energy > 0, 'live Mac input drives the analyser');
const count = nodes[0].messages.length;
streamController.enqueue(bytesFor([.25]));
streamController.enqueue(bytesFor([.25]));
await tick();
assert.equal(nodes[0].messages.length, count, 'unacknowledged worklet messages are bounded');
clock += 1600;
assert.equal(engine.getState().systemSignal, 'silent');
const micTrack = { readyState: 'live', addEventListener() {}, stop() { this.readyState = 'ended'; } };
const micStream = { getTracks: () => [micTrack], getAudioTracks: () => [micTrack] };
navigator.mediaDevices.getUserMedia = async () => micStream;
// A successful source switch must stop the local bridge before using the mic.
MockContext.prototype.createMediaStreamSource = () => new MockNode('capture');
await engine.startMicrophone();
assert.equal(abortSignal.aborted, true, 'switching input closes computer audio');
assert.equal(engine.getState().source, 'microphone');
engine.stopCapture();
assert.equal(micTrack.readyState, 'ended');
assert.equal(engine.getState().source, 'none');

// A late bridge response after Stop must never connect or leak a session.
let release;
globalThis.fetch = (_url, options) => {
  abortSignal = options.signal;
  return new Promise(resolve => { release = resolve; });
};
const pending = engine.startSystemAudio();
assert.equal(engine.getState().connecting, true);
engine.stopCapture();
release({ ok: true, body: new ReadableStream() });
await pending;
assert.equal(abortSignal.aborted, true);
assert.equal(engine.getState().source, 'none');
assert.equal(nodes.length, 1, 'late response must not construct a worklet');
engine.destroy();

// A busy/failed Mac bridge must not stop an already-playing file.
URL.createObjectURL = () => 'blob:test';
URL.revokeObjectURL = () => {};
const fallback = createAudioEngine();
await fallback.loadFile({ name: 'test.wav' });
await fallback.togglePlayback();
globalThis.fetch = async () => ({ ok: false, status: 409, json: async () => ({ error: 'Mac audio is already in use.' }) });
await assert.rejects(fallback.startSystemAudio(), /already in use/);
assert.equal(fallback.getState().source, 'file');
assert.equal(fallback.getState().playing, true, 'failed connection keeps the previous input playing');
fallback.destroy();
console.log('System audio streaming and lifecycle checks passed');
