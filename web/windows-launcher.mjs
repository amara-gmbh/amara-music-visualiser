#!/usr/bin/env node
/** One-click, loopback-only Windows host for the browser visualiser. */
import { spawn } from 'node:child_process';
import { createReadStream, existsSync, realpathSync, statSync } from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createWindowsAudioBridge } from './windows-audio-bridge.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const ROOT = realpathSync(path.join(HERE, '..'));
const DEFAULT_URL = 'http://127.0.0.1:8768/web/';
const MIME = new Map([
  ['.css', 'text/css; charset=utf-8'], ['.html', 'text/html; charset=utf-8'],
  ['.js', 'text/javascript; charset=utf-8'], ['.json', 'application/json; charset=utf-8'],
  ['.mjs', 'text/javascript; charset=utf-8'], ['.svg', 'image/svg+xml'],
  ['.wgsl', 'text/plain; charset=utf-8'],
]);

function text(res, status, body, extra = {}) {
  const data = Buffer.from(body);
  res.writeHead(status, { 'Content-Type': 'text/plain; charset=utf-8', 'Content-Length': data.length,
    'Cache-Control': 'no-store', 'X-Content-Type-Options': 'nosniff', ...extra });
  res.end(data);
}

export function resolveAsset(urlPath, root = ROOT) {
  const canonicalRoot = realpathSync(root);
  let decoded;
  try { decoded = decodeURIComponent(urlPath); } catch { return undefined; }
  if (decoded.includes('\0') || decoded.includes('\\')) return undefined;
  if (decoded === '/') return { redirect: '/web/' };
  if (decoded === '/web/') decoded = '/web/index.html';
  const relative = decoded.replace(/^\/+/, '');
  const candidate = path.resolve(canonicalRoot, relative);
  const webRoot = path.join(canonicalRoot, 'web');
  const srcRoot = path.join(canonicalRoot, 'src');
  const inWeb = candidate.startsWith(`${webRoot}${path.sep}`);
  const inShaderSource = candidate.startsWith(`${srcRoot}${path.sep}`) && path.extname(candidate) === '.wgsl';
  if (!inWeb && !inShaderSource) return undefined;
  const extension = path.extname(candidate).toLowerCase();
  if (!MIME.has(extension) || !existsSync(candidate) || !statSync(candidate).isFile()) return undefined;
  let real;
  try { real = realpathSync(candidate); } catch { return undefined; }
  const realInWeb = real.startsWith(`${webRoot}${path.sep}`);
  const realInShaderSource = real.startsWith(`${srcRoot}${path.sep}`) && path.extname(real) === '.wgsl';
  if (!realInWeb && !realInShaderSource) return undefined;
  return { file: real, type: MIME.get(extension), size: statSync(real).size };
}

export function createVisualiserServer({ port = 8768, root = ROOT } = {}) {
  const server = http.createServer((req, res) => {
    const authority = `127.0.0.1:${server.address().port}`;
    if (req.headers.host !== authority) return text(res, 403, 'Forbidden');
    if (req.method !== 'GET' && req.method !== 'HEAD') return text(res, 405, 'Method not allowed', { Allow: 'GET, HEAD' });
    const asset = resolveAsset(new URL(req.url, `http://${authority}`).pathname, root);
    if (!asset) return text(res, 404, 'Not found');
    if (asset.redirect) {
      res.writeHead(302, { Location: asset.redirect, 'Cache-Control': 'no-store' });
      return res.end();
    }
    res.writeHead(200, { 'Content-Type': asset.type, 'Content-Length': asset.size, 'Cache-Control': 'no-store',
      'X-Content-Type-Options': 'nosniff', 'X-Frame-Options': 'DENY', 'Content-Security-Policy': "frame-ancestors 'none'" });
    if (req.method === 'HEAD') return res.end();
    const stream = createReadStream(asset.file);
    stream.on('error', () => { if (!res.headersSent) text(res, 500, 'Read error'); else res.destroy(); });
    stream.pipe(res);
  });
  server.listen(port, '127.0.0.1');
  return server;
}

export function openBrowser(url = DEFAULT_URL, spawnProcess = spawn) {
  const child = spawnProcess('cmd.exe', ['/d', '/s', '/c', 'start', '', url], {
    windowsHide: true, detached: true, stdio: 'ignore',
  });
  child.unref?.();
  return child;
}

export async function startWindowsVisualiser({ webPort = 8768, bridgePort = 8769, open = openBrowser } = {}) {
  if (process.platform !== 'win32') throw new Error('The one-click launcher runs on Windows only.');
  const web = createVisualiserServer({ port: webPort });
  try {
    await new Promise((resolve, reject) => { web.once('listening', resolve); web.once('error', reject); });
    const bridge = createWindowsAudioBridge({ port: bridgePort, visualiserPort: webPort });
    try {
      await new Promise((resolve, reject) => { bridge.once('listening', resolve); bridge.once('error', reject); });
      open(`http://127.0.0.1:${webPort}/web/`);
      return { web, bridge };
    } catch (error) {
      await new Promise(resolve => web.close(resolve));
      throw error;
    }
  } catch (error) {
    if (web.listening) await new Promise(resolve => web.close(resolve));
    throw error;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  startWindowsVisualiser().then(({ web, bridge }) => {
    const stop = () => bridge.close(() => web.close(() => process.exit(0)));
    for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, stop);
  }).catch(error => {
    console.error(`amaraMusicVisualiser could not start: ${error.message}`);
    process.exitCode = 1;
  });
}
