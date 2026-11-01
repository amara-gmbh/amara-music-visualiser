import assert from 'node:assert/strict';
import { createMusicInsight, describeMusicInsight, musicVisualAdjustments } from '../music-insight.js';

const insight = createMusicInsight();
let state = insight.update({ active: false, source: 'none', energy: 0 }, 0);
assert.equal(state.section, 'quiet');
assert.equal(state.bpm, 0);
assert.equal(state.tempoConfidence, 0);
assert.match(describeMusicInsight(state), /waiting for a clear signal/);
assert.deepEqual(musicVisualAdjustments(state), { paceScale: 1, intensityScale: 1, glowOffset: 0 });
assert.deepEqual(musicVisualAdjustments({ buildup: 1, drop: 1 }),
  { paceScale: 1.025, intensityScale: 1.085, glowOffset: 0.08 });

function pulseTrain({ bpm, seconds, source = 'file' }) {
  insight.reset(source);
  const step = 0.05;
  const period = 60 / bpm;
  let onsets = 0;
  let last = insight.state;
  for (let time = 0; time <= seconds + 1e-6; time += step) {
    const phase = time % period;
    const pulse = phase < step * 0.8 || period - phase < 1e-6;
    last = insight.update({ active: true, source, energy: pulse ? 0.72 : 0.08,
      bass: pulse ? 0.85 : 0.07 }, time);
    if (last.onset > 0) onsets++;
  }
  return { last, onsets };
}

let train = pulseTrain({ bpm: 120, seconds: 8 });
assert.ok(train.onsets >= 12, 'steady pulses produce stable onset events');
assert.ok(train.last.bpm >= 118 && train.last.bpm <= 122, `expected 120 BPM, got ${train.last.bpm}`);
assert.ok(train.last.tempoConfidence >= 0.75, 'regular pulse train reaches useful confidence');
assert.match(describeMusicInsight(train.last), /120 BPM/);

train = pulseTrain({ bpm: 90, seconds: 12, source: 'shared' });
assert.ok(train.last.bpm >= 88 && train.last.bpm <= 92, `expected 90 BPM, got ${train.last.bpm}`);
assert.ok(train.last.tempoConfidence >= 0.7);

insight.reset('system');
let sawBuildup = false;
let sawDrop = false;
for (let i = 0; i < 24; i++) {
  const time = i * 0.1;
  const energy = 0.08 + i * 0.014;
  state = insight.update({ active: true, source: 'system', energy, bass: energy * 0.8 }, time);
  sawBuildup ||= state.event === 'buildup';
}
state = insight.update({ active: true, source: 'system', energy: 0.76, bass: 0.82 }, 2.4);
sawDrop ||= state.event === 'drop';
assert.equal(sawBuildup, true, 'sustained rise emits one buildup event');
assert.equal(sawDrop, true, 'strong onset after buildup emits a drop event');
assert.ok(state.drop > 0.9);

train = pulseTrain({ bpm: 120, seconds: 5, source: 'file' });
assert.ok(train.last.tempoConfidence > 0.5);
state = insight.update({ active: true, source: 'microphone', energy: 0.12, bass: 0.1 }, 5.1);
assert.equal(state.bpm, 0, 'source change clears tempo history');
assert.equal(state.tempoConfidence, 0);
insight.reset('shared');
assert.equal(insight.state.bpm, 0, 'explicit reset clears tempo history');
for (let time = 0; time <= 1.5; time += 0.1) {
  state = insight.update({ active: true, source: 'shared', energy: 0, bass: 0 }, time);
}
assert.equal(state.section, 'quiet');
assert.equal(state.tempoConfidence, 0, 'sustained silence clears confidence');

console.log('Music insight silence, onset, tempo, buildup/drop and reset checks passed');
