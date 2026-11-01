import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import http from 'node:http';
import { createVisualiserServer, resolveAsset } from '../windows-launcher.mjs';

assert.equal(resolveAsset('/Cargo.toml'), undefined, 'repository files are not public');
assert.equal(resolveAsset('/src/shader.wgsl')?.type, 'text/plain; charset=utf-8');
assert.equal(resolveAsset('/web/mac-audio-server.py'), undefined, 'server source is not public');
assert.equal(resolveAsset('/web/windows-audio-helper/AmaraWindowsAudioCapture.cs'), undefined, 'helper source is not public');
assert.equal(resolveAsset('/web/%2e%2e/Cargo.toml'), undefined, 'encoded traversal is blocked');
assert.deepEqual(resolveAsset('/'), { redirect: '/web/' });

const server = createVisualiserServer({ port: 0 });
await new Promise(resolve => server.on('listening', resolve));
const port = server.address().port;
const base = `http://127.0.0.1:${port}`;
try {
  let response = await fetch(`${base}/`);
  assert.equal(response.status, 200);
  assert.equal(response.url, `${base}/web/`);
  assert.match(await response.text(), /amaraMusicVisualiser/);
  assert.equal(response.headers.get('x-content-type-options'), 'nosniff');
  assert.equal(response.headers.get('x-frame-options'), 'DENY');

  response = await fetch(`${base}/src/shader.wgsl`, { method: 'HEAD' });
  assert.equal(response.status, 200);
  assert.ok(Number(response.headers.get('content-length')) > 100);
  assert.equal(await response.text(), '');

  response = await fetch(`${base}/Cargo.toml`);
  assert.equal(response.status, 404);
  response = await fetch(`${base}/web/mac-audio-server.py`);
  assert.equal(response.status, 404);
  response = await fetch(`${base}/web/%2e%2e/Cargo.toml`, { redirect: 'manual' });
  assert.equal(response.status, 404);

  response = await fetch(`${base}/web/`, { method: 'POST' });
  assert.equal(response.status, 405);
  assert.equal(response.headers.get('allow'), 'GET, HEAD');

  const raw = await new Promise((resolve, reject) => {
    const req = http.request({ host: '127.0.0.1', port, path: '/web/', headers: { Host: 'evil.example' } }, res => {
      res.resume(); res.on('end', () => resolve(res.statusCode));
    });
    req.on('error', reject); req.end();
  });
  assert.equal(raw, 403, 'Host header is pinned to the loopback authority');

  const vbs = await readFile(new URL('../Start amaraMusicVisualiser.vbs', import.meta.url), 'utf8');
  assert.match(vbs, /shell\.Run command, 0, False/, 'wrapper starts the host hidden');
  assert.match(vbs, /command = "node\.exe/, 'wrapper launches Node directly without an intermediate shell');
} finally {
  await new Promise(resolve => server.close(resolve));
}
console.log('Windows one-click static host security and hidden-wrapper checks passed');
