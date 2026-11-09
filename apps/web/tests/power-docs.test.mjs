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
const project = projects.find((entry) => entry.id === 'power');

test('Power registration keeps independent version and execution boundaries', () => {
  assert.ok(project);
  assert.equal(project.defaultVersion, '0.10.0');
  assert.deepEqual(project.versions[0].pages.map((page) => page.slug), ['', 'serving/', 'distributed-inference/', 'operations/']);
  assert.equal(project.versions[0].sourceRef, '8729d7c84a2d887c61dc206ba47bf8a63425e768');
});

test('Power pages preserve serving, distributed and release contracts', async (t) => {
  const expectations = [
    ['', 'Aster Power 产品概览', 'Power 的模型中立执行边界'],
    ['serving/', 'Power 服务与嵌入式接入', 'EmbeddedRuntime'],
    ['distributed-inference/', 'Power 分布式推理与缓存', 'prompt_cache_unsupported'],
    ['operations/', 'Power 运行与发布验证', 'strict v1'],
  ];
  for (const [route, title, text] of expectations) {
    const html = await readFile(path.join(root, 'apps/web/dist/docs/power/0.10.0', route, 'index.html'), 'utf8');
    const dom = new JSDOM(html);
    t.after(() => dom.window.close());
    assert.ok(dom.window.document.documentElement.textContent.includes(title), route);
    assert.ok(dom.window.document.documentElement.textContent.includes(text), route);
    assert.doesNotMatch(html, /data-atlas-action|atlas-controls/);
  }
  const alias = await readFile(path.join(root, 'apps/web/dist/docs/power/index.html'), 'utf8');
  assert.ok(alias.includes('Aster Power 产品概览'));
});
