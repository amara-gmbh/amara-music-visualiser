import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { createWindowsAudioBridge } from '../windows-audio-bridge.mjs';

const origin = 'http://127.0.0.1:8768';
const helper = fileURLToPath(import.meta.url);
const helperSource = await readFile(new URL('../windows-audio-helper/AmaraWindowsAudioCapture.cs', import.meta.url), 'utf8');
const multimediaSelection = helperSource.indexOf('GetDefaultAudioEndpoint(EDataFlow.Render, ERole.Multimedia');
const consoleFallback = helperSource.indexOf('GetDefaultAudioEndpoint(EDataFlow.Render, ERole.Console');
assert.ok(multimediaSelection >= 0, 'WASAPI loopback must prefer the multimedia render endpoint');
assert.ok(consoleFallback > multimediaSelection, 'console render endpoint is only the fallback');
const spawnCapture = () => spawn(process.execPath, ['-e',
  "const block=Buffer.alloc(4096); process.stdout.write(block); setInterval(()=>process.stdout.write(block),10)"],
{ stdio: ['ignore', 'pipe', 'pipe'] });
const server = createWindowsAudioBridge({ port: 0, visualiserPort: 8768, helper, spawnCapture, startTimeoutMs: 2000 });
await new Promise(resolve => server.on('listening', resolve));
const base = `http://127.0.0.1:${server.address().port}`;
const post = (path, requestOrigin = origin) => fetch(base + path, { method: 'POST', headers: { Origin: requestOrigin } });

try {
  let response = await post('/api/system-audio/status', 'https://evil.example');
  assert.equal(response.status, 403);
  response = await post('/api/system-audio/status');
  assert.equal(response.status, 200);
  assert.deepEqual(await response.json(), { available: true, active: false, sampleRate: 48000,
    channels: 1, format: 'f32le', platform: 'windows' });

  const stream = await post('/api/system-audio/stream');
  assert.equal(stream.status, 200);
  assert.equal(stream.headers.get('access-control-allow-origin'), origin);
  const reader = stream.body.getReader();
  const first = await reader.read();
  assert.ok(first.value.length > 0);
  await new Promise(resolve => setTimeout(resolve, 2200));
  assert.equal(server.listening, true, 'startup timeout cannot crash an established stream');
  response = await post('/api/system-audio/stream');
  assert.equal(response.status, 409, 'only one browser may receive loopback audio');
  response = await post('/api/system-audio/status');
  assert.equal((await response.json()).active, true);
  response = await post('/api/system-audio/stop');
  assert.deepEqual(await response.json(), { stopped: true });
  await reader.cancel();
} finally {
  await new Promise(resolve => server.close(resolve));
}
console.log('Windows loopback bridge origin, stream, exclusivity and stop checks passed');
