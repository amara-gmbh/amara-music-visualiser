#!/usr/bin/env node
/** Local-only Windows WASAPI loopback bridge for the browser visualiser. */
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const DEFAULT_HELPER = path.join(HERE, 'windows-audio-helper', 'amara-windows-audio-capture.ps1');
const RATE = 48_000;

function json(res, status, value, origin) {
  if (res.headersSent) { res.end(); return; }
  const body = Buffer.from(JSON.stringify(value));
  const headers = { 'Content-Type': 'application/json', 'Content-Length': body.length, 'Cache-Control': 'no-store' };
  if (origin) Object.assign(headers, { 'Access-Control-Allow-Origin': origin, Vary: 'Origin' });
  res.writeHead(status, headers);
  res.end(body);
}

function startHelper(helper) {
  if (path.extname(helper).toLowerCase() === '.ps1') {
    return spawn('powershell.exe', ['-NoLogo', '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', helper], {
      windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
    });
  }
  return spawn(helper, [], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
}

export function createWindowsAudioBridge({ port = 8769, visualiserPort = 8768, helper = DEFAULT_HELPER,
  spawnCapture = () => startHelper(helper), startTimeoutMs = 10_000 } = {}) {
  let active;
  const allowedOrigin = `http://127.0.0.1:${visualiserPort}`;

  function stopCapture() {
    const child = active;
    active = undefined;
    if (child && child.exitCode === null) child.kill();
  }

  const server = http.createServer((req, res) => {
    const authority = `127.0.0.1:${server.address().port}`;
    const origin = req.headers.origin;
    const allowed = req.headers.host === authority && origin === allowedOrigin;
    if (req.method === 'OPTIONS') {
      if (!allowed) return json(res, 403, { error: 'Request origin is not allowed.' });
      res.writeHead(204, {
        'Access-Control-Allow-Origin': allowedOrigin,
        'Access-Control-Allow-Methods': 'POST, OPTIONS',
        'Access-Control-Allow-Headers': 'Content-Type',
        'Access-Control-Allow-Private-Network': 'true',
        'Access-Control-Max-Age': '600', Vary: 'Origin',
      });
      return res.end();
    }
    if (req.method !== 'POST' || !allowed || Number(req.headers['content-length'] || 0) !== 0 || req.headers['transfer-encoding']) {
      return json(res, 403, { error: 'Request origin is not allowed.' }, allowed ? allowedOrigin : undefined);
    }
    const pathname = new URL(req.url, `http://${authority}`).pathname;
    if (pathname === '/api/system-audio/status') {
      return json(res, 200, { available: existsSync(helper), active: Boolean(active), sampleRate: RATE,
        channels: 1, format: 'f32le', platform: 'windows' }, allowedOrigin);
    }
    if (pathname === '/api/system-audio/stop') {
      stopCapture();
      return json(res, 200, { stopped: true }, allowedOrigin);
    }
    if (pathname !== '/api/system-audio/stream') {
      return json(res, 404, { error: 'Unknown endpoint.' }, allowedOrigin);
    }
    if (active) return json(res, 409, { error: 'Computer audio is already in use by this bridge.' }, allowedOrigin);
    if (!existsSync(helper)) return json(res, 503, { error: 'The Windows audio helper is not installed.' }, allowedOrigin);

    let child;
    try { child = spawnCapture(); } catch {
      return json(res, 503, { error: 'The Windows audio helper could not be started.' }, allowedOrigin);
    }
    active = child;
    let started = false;
    let settled = false;
    const timeout = setTimeout(() => {
      if (settled || started) return;
      settled = true;
      stopCapture();
      json(res, 504, { error: 'Windows audio did not start. Check that an output device is active.' }, allowedOrigin);
    }, startTimeoutMs);
    const finish = () => {
      clearTimeout(timeout);
      if (active === child) active = undefined;
    };
    child.stdout.on('data', chunk => {
      if (settled || !chunk.length) return;
      if (!started) {
        started = true;
        clearTimeout(timeout);
        res.writeHead(200, { 'Content-Type': 'application/octet-stream', 'Cache-Control': 'no-store',
          'Access-Control-Allow-Origin': allowedOrigin, Vary: 'Origin' });
      }
      if (!res.write(chunk)) child.stdout.pause();
    });
    res.on('drain', () => child.stdout.resume());
    child.stderr.on('data', () => { /* Never expose device details to the page or logs. */ });
    child.on('error', () => {
      if (settled) return;
      settled = true;
      finish();
      if (!res.headersSent) json(res, 503, { error: 'The Windows audio helper could not be started.' }, allowedOrigin);
      else res.end();
    });
    child.on('close', () => {
      if (settled) return;
      settled = true;
      finish();
      if (!res.headersSent) json(res, 503, { error: 'Windows audio stopped before starting.' }, allowedOrigin);
      else res.end();
    });
    res.on('close', () => {
      if (settled) return;
      settled = true;
      if (!res.writableEnded && active === child) stopCapture();
      finish();
    });
  });
  server.on('close', stopCapture);
  server.listen(port, '127.0.0.1');
  return server;
}

function option(name, fallback) {
  const index = process.argv.indexOf(name);
  return index >= 0 ? process.argv[index + 1] : fallback;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  if (process.platform !== 'win32') {
    console.error('The Windows audio bridge must run on Windows.');
    process.exitCode = 1;
  } else {
    const port = Number(option('--port', '8769'));
    const visualiserPort = Number(option('--visualiser-port', '8768'));
    if (!Number.isInteger(port) || port < 1 || port > 65535 || !Number.isInteger(visualiserPort) || visualiserPort < 1 || visualiserPort > 65535) {
      console.error('Ports must be integers between 1 and 65535.');
      process.exitCode = 1;
    } else {
      const server = createWindowsAudioBridge({ port, visualiserPort });
      server.on('listening', () => console.log(`amaraMusicVisualiser Windows audio: http://127.0.0.1:${port}`));
      for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, () => server.close(() => process.exit(0)));
    }
  }
}
