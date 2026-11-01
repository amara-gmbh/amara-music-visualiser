import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';

const html = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
const main = readFileSync(new URL('../main.js', import.meta.url), 'utf8');
const revision = html.match(/src="\.\/main\.js\?v=([^"&]+)"/)?.[1];
assert.ok(revision, 'main entry needs a cache-busting release revision');
assert.ok(html.includes(`./style.css?v=${revision}`), 'stylesheet revision must match the entry');
assert.ok(html.includes(`./assets/amara-logo.svg?v=${revision}`), 'logo revision must match the entry');
assert.ok(existsSync(new URL('../assets/amara-logo.svg', import.meta.url)), 'official logo asset must exist');
const imports = Array.from(main.matchAll(/from ['"](\.\/[^'"]+)['"]/g), match => match[1]);
assert.ok(imports.length > 0);
for (const specifier of imports) {
  const [path, query] = specifier.split('?');
  assert.equal(query, `v=${revision}`, `browser import ${path} needs the release revision`);
  assert.ok(existsSync(new URL(`../${path}`, import.meta.url)), `missing browser module ${path}`);
}
for (const file of ['main.js', 'geometry-pass.js', 'dot-pass.js']) {
  const code = readFileSync(new URL(`../${file}`, import.meta.url), 'utf8');
  assert.match(code, /new URL\(import\.meta\.url\)\.searchParams\.get\('v'\)/,
    `${file} must take its shader revision from the loaded module`);
  assert.match(code, /if \(revision\) url\.searchParams\.set\('v', revision\)/,
    `${file} must propagate its revision to shader requests`);
  const moduleUrl = new URL(`../${file}?v=${revision}`, import.meta.url);
  const shaderUrl = new URL('../src/params.wgsl', moduleUrl);
  shaderUrl.searchParams.set('v', moduleUrl.searchParams.get('v'));
  assert.equal(shaderUrl.searchParams.get('v'), revision);
  assert.ok(shaderUrl.pathname.endsWith('/src/params.wgsl'));
}
console.log('Browser release asset revisions match');
