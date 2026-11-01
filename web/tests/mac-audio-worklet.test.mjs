import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';

let Processor;
const sandbox = {
  Float32Array, Number, sampleRate: 44100,
  AudioWorkletProcessor: class { constructor() { this.port = { messages: [], postMessage(value) { this.messages.push(value); } }; } },
  registerProcessor(name, cls) { assert.equal(name, 'amara-system-audio'); Processor = cls; },
};
vm.runInNewContext(await readFile(new URL('../system-audio-worklet.js', import.meta.url), 'utf8'), sandbox);
const worklet = new Processor();
worklet.port.onmessage({ data: Float32Array.from([-.02, .02]) });
assert.ok(Math.abs(worklet.ring[0]) > .02 && Math.abs(worklet.ring[1]) > .02,
  'quiet system audio is amplified for analysis');
assert.ok(Math.abs(worklet.ring[0]) <= 1 && Math.abs(worklet.ring[1]) <= 1,
  'analysis gain remains bounded');
worklet.count = 0; worklet.read = 0; worklet.write = 0; worklet.port.messages.length = 0;
worklet.port.onmessage({ data: Float32Array.from({ length: 10000 }, (_, i) => i / 10000) });
assert.equal(worklet.count, 8192, 'the audio ring is bounded even after a burst');
assert.deepEqual(worklet.port.messages, ['ready'], 'each transferred chunk is acknowledged');
const output = new Float32Array(128);
worklet.process([], [[output]]);
assert.ok(output.every(Number.isFinite));
assert.ok(output[1] > output[0], 'audio is rendered and resampled from the retained live samples');
assert.ok(worklet.count < 8192, 'processing consumes the ring');
worklet.count = 0;
output.fill(1);
worklet.process([], [[output]]);
assert.ok(output.every(value => value === 0), 'underflow produces silence rather than stale sound');
console.log('System audio worklet buffering and resampling checks passed');
