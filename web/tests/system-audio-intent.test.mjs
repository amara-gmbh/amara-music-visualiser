import assert from 'node:assert/strict';
import { createSystemAudioIntent } from '../audio-intent.js';

const values = new Map();
const storage = {
  getItem: key => values.get(key) ?? null,
  setItem: (key, value) => values.set(key, value),
  removeItem: key => values.delete(key),
};
const intent = createSystemAudioIntent(() => storage);
assert.equal(intent.requested(), false);
intent.request();
assert.equal(intent.requested(), true, 'system-audio intent survives a page reload in session storage');
intent.clear();
assert.equal(intent.requested(), false, 'explicit stop clears reconnect intent');

const unavailable = createSystemAudioIntent(() => { throw new Error('storage disabled'); });
assert.doesNotThrow(() => unavailable.request());
assert.equal(unavailable.requested(), false);
assert.doesNotThrow(() => unavailable.clear());
console.log('System-audio reconnect intent storage checks passed');
