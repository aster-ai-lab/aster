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
const project = projects.find((entry) => entry.id === 'office');

test('Office registration keeps browser and native guides on current source', () => {
  assert.ok(project);
  assert.equal(project.defaultVersion, 'current');
  assert.deepEqual(project.versions[0].pages.map((page) => page.slug), ['', 'browser/', 'automation/']);
  assert.equal(project.versions[0].sourceRef, '8729d7c84a2d887c61dc206ba47bf8a63425e768');
});

test('Office pages preserve editor, automation and collaboration boundaries', async (t) => {
  const expectations = [
    ['', 'Aster Office 产品概览', 'Aster Office 的双执行平面'],
    ['browser/', 'Office 浏览器编辑器接入', 'DocumentEditor'],
    ['automation/', 'Office 原生自动化', 'office_apply_batch'],
  ];
  for (const [route, title, text] of expectations) {
    const html = await readFile(path.join(root, 'apps/web/dist/docs/office/current', route, 'index.html'), 'utf8');
    const dom = new JSDOM(html);
    t.after(() => dom.window.close());
    assert.ok(dom.window.document.documentElement.textContent.includes(title), route);
    assert.ok(dom.window.document.documentElement.textContent.includes(text), route);
    assert.doesNotMatch(html, /data-atlas-action|atlas-controls/);
  }
  const alias = await readFile(path.join(root, 'apps/web/dist/docs/office/index.html'), 'utf8');
  assert.ok(alias.includes('Aster Office 产品概览'));
});
