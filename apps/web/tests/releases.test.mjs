import assert from 'node:assert/strict';
import { readFile, writeFile, mkdir, mkdtemp, rm } from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import test from 'node:test';
import { selectDownloads, renderDownloadDocument, repository } from '../scripts/build_releases.mjs';

function release(tag, names, extra = {}) {
  return { tag_name: tag, draft: false, prerelease: false, published_at: '2026-10-28T00:00:00Z', assets: names.map((name) => ({ name, state: 'uploaded', size: 123, browser_download_url: `${repository}/releases/download/${tag}/${name}` })), ...extra };
}
const base = () => [
  release('v0.9.0', ['aster_0.9.0_aarch64.dmg','aster_0.9.0_x64-setup.exe','aster_0.9.0_kylin-v10_x86_64.run']),
  release('aster-cli-v0.9.0', ['aster-cli-v0.9.0-macos.tar.gz','aster-cli-v0.9.0-linux.tar.gz','aster-cli-v0.9.0-windows.zip']),
];

test('latest downloads follow numeric versions per product/platform and exclude SDK, draft and prerelease assets', () => {
  const data = [release('v0.10.0', ['aster_0.10.0_aarch64.dmg']), ...base(),
    release('v9.1.6', ['a3s-code-acp-v9.1.6-macos.tar.gz']),
    release('v0.99.0', ['aster_0.99.0_aarch64.dmg'], { draft: true }),
    release('v0.98.0', ['aster_0.98.0_x64-setup.exe'], { prerelease: true })];
  const selected = selectDownloads(data);
  assert.deepEqual(selected.map(({ version }) => version), ['0.10.0','0.9.0','0.9.0','0.9.0','0.9.0','0.9.0']);
  assert.equal(selected[0].url, `${repository}/releases/download/v0.10.0/aster_0.10.0_aarch64.dmg`);
  assert.equal(selectDownloads([...base(), release(undefined, ['aster_0.99.0_aarch64.dmg'])])[0].version, '0.9.0');
});

test('the existing desktop-stable ZIP publisher is supported only after publication', () => {
  const stable = release('desktop-stable', ['Aster-darwin-aarch64-v0.11.0.zip']);
  assert.equal(selectDownloads([...base(), { ...stable, draft: true }])[0].version, '0.9.0');
  const selected = selectDownloads([...base(), stable])[0];
  assert.equal(selected.version, '0.11.0');
  assert.equal(selected.format, 'ZIP');
  assert.equal(selected.tag, 'desktop-stable');
  assert.equal(selectDownloads([...base(), { ...stable, tag_name: 'unrelated-channel' }])[0].version, '0.9.0');
});

test('missing or malformed installation assets refuse rather than selecting another product', () => {
  assert.throws(() => selectDownloads(base().slice(1)), /desktop.macos/);
  const data = base();
  data[0].assets[0].browser_download_url = 'https://github.com.evil.example/installer.dmg';
  assert.throws(() => selectDownloads(data), /Invalid published asset/);
  data[0].assets[0].browser_download_url = `${repository}/releases/download/v0.9.0/aster_0.9.0_aarch64.dmg`;
  data[0].assets[0].state = 'uploading';
  assert.throws(() => selectDownloads(data), /Invalid published asset/);
});

test('real template rendering keeps versions, formats and direct URLs aligned', async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'aster-download-build-'));
  t.after(() => rm(root, { recursive: true, force: true }));
  await mkdir(path.join(root, 'content'));
  const downloads = selectDownloads([...base(), release('desktop-stable', ['Aster-darwin-aarch64-v0.11.0.zip'])]);
  await writeFile(path.join(root, 'content/releases.json'), JSON.stringify({ schema: 'aster.web.downloads.v1', downloads }));
  for (const name of ['home.html', 'download.html']) await writeFile(path.join(root, 'content', name), '<!doctype html><a href="{{download.desktop.macos.url}}">v{{download.desktop.macos.version}} {{download.desktop.macos.format}}</a>');
  const html = await renderDownloadDocument('home.html', { root });
  assert.ok(html.includes(`${repository}/releases/download/desktop-stable/Aster-darwin-aarch64-v0.11.0.zip`));
  assert.ok(html.includes('v0.11.0 ZIP'));
  downloads[3].version = '0.10.0';
  await writeFile(path.join(root, 'content/releases.json'), JSON.stringify({ schema: 'aster.web.downloads.v1', downloads }));
  await assert.rejects(renderDownloadDocument('home.html', { root }), /missing matching platform archives/);
});

test('published page snapshots expose exactly the six selected stable URLs without browser API requests', async () => {
  const snapshot = JSON.parse(await readFile(new URL('../content/releases.json', import.meta.url), 'utf8'));
  for (const name of ['index.html','download/index.html']) {
    const html = await readFile(new URL(`../dist/${name}`, import.meta.url), 'utf8');
    const urls = new Set([...html.matchAll(/href="(https:\/\/github.com\/aster-ai-lab\/aster\/releases\/download\/[^" ]+)"/g)].map((match) => match[1]));
    assert.deepEqual([...urls].sort(), snapshot.downloads.map((item) => item.url).sort());
    assert.ok(!html.includes('{{download.'));
  }
});
