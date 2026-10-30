import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { projects } from '../scripts/docs_catalog.mjs';

const require = createRequire(import.meta.url);
const root = fileURLToPath(new URL('../../../', import.meta.url));
const { JSDOM } = require(process.env.JS_DOM_MODULE || path.join(root, 'packages/ui/node_modules/jsdom'));

test('Vec and Apofasi registration keeps independent versions and guides', () => {
  for (const [id, version] of [['vec', '0.1.8'], ['apofasi', '0.1.1']]) {
    const project = projects.find((entry) => entry.id === id);
    assert.ok(project, id);
    assert.equal(project.defaultVersion, version);
    assert.deepEqual(project.versions[0].pages.map((page) => page.slug), ['', 'integration/', 'operations/']);
    assert.equal(project.versions[0].sourceRef, '8729d7c84a2d887c61dc206ba47bf8a63425e768');
  }
});

test('Vec and Apofasi pages preserve retrieval and decision boundaries', async (t) => {
  const expectations = [
    ['vec', '0.1.8', '', 'Aster Vec 产品概览', '精确重排'],
    ['vec', '0.1.8', 'integration/', 'Vec 接入', 'Collection'],
    ['vec', '0.1.8', 'operations/', 'Vec 运行与验证', 'StorageCeilings'],
    ['apofasi', '0.1.1', '', 'Aster Apofasi 产品概览', '类型化 System-1'],
    ['apofasi', '0.1.1', 'integration/', 'Apofasi 接入', 'SystemOneRequest'],
    ['apofasi', '0.1.1', 'operations/', 'Apofasi 推理与验证', 'APOFASI_CHECKPOINT'],
  ];
  for (const [product, version, route, title, text] of expectations) {
    const html = await readFile(path.join(root, 'apps/web/dist/docs', product, version, route, 'index.html'), 'utf8');
    const dom = new JSDOM(html);
    t.after(() => dom.window.close());
    assert.ok(dom.window.document.documentElement.textContent.includes(title), `${product}/${route}`);
    assert.ok(dom.window.document.documentElement.textContent.includes(text), `${product}/${route}`);
    assert.doesNotMatch(html, /data-atlas-action|atlas-controls/);
  }
});
