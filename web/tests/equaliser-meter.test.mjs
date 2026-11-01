import assert from 'node:assert/strict';
import { meterLevel, meterPeakBottom } from '../equaliser-meter.js';

assert.equal(meterLevel(0), 0.025);
assert.equal(meterLevel(0.42), 0.42);
assert.equal(meterLevel(1.8), 1);
assert.equal(meterLevel(Number.NaN), 0.025);
assert.equal(meterPeakBottom(0.42), 'calc(42% - 1px)');
assert.equal(meterPeakBottom(1), 'calc(100% - 1px)');
assert.equal(meterPeakBottom(4), 'calc(100% - 1px)');

console.log('Equaliser fill and peak display bounds passed');
