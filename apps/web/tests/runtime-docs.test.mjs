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
const expected = [
  ['memory', '0.1.5', ['', 'repository/', 'retrieval/']],
  ['event', '0.4.0', ['', 'integration/', 'operations/']],
  ['lane', '0.5.1', ['', 'integration/', 'operations/']],
];

test('runtime products keep independent versions and guides', () => {
  for (const [id, version, slugs] of expected) {
    const project = projects.find((entry) => entry.id === id);
    assert.ok(project, id);
    assert.equal(project.defaultVersion, version);
    assert.deepEqual(project.versions[0].pages.map((page) => page.slug), slugs);
    assert.equal(project.versions[0].sourceRef, '8729d7c84a2d887c61dc206ba47bf8a63425e768');
  }
});

test('runtime pages preserve storage, delivery and scheduling boundaries', async (t) => {
  const expectations = [
    ['memory', '0.1.5', '', 'Aster Memory 产品概览', 'Memory 的证据、存储与派生索引边界'],
    ['memory', '0.1.5', 'repository/', 'Memory 存储与 Repository', 'MemoryNamespace'],
    ['memory', '0.1.5', 'retrieval/', 'Memory 向量检索与验证', 'index_revision_cas'],
    ['event', '0.4.0', '', 'Aster Event 产品概览', 'Event 的 Provider 中立发布订阅边界'],
    ['event', '0.4.0', 'integration/', 'Event 发布订阅接入', 'next_manual_ack'],
    ['event', '0.4.0', 'operations/', 'Event 交付与运行验证', 'at-least-once'],
    ['lane', '0.5.1', '', 'Aster Lane 产品概览', 'Lane 的优先级准入与执行边界'],
    ['lane', '0.5.1', 'integration/', 'Lane 队列与调度接入', 'QueueManagerBuilder'],
    ['lane', '0.5.1', 'operations/', 'Lane 可靠性与运行验证', 'ensure_lease'],
  ];
  for (const [product, version, route, title, text] of expectations) {
    const html = await readFile(path.join(root, 'apps/web/dist/docs', product, version, route, 'index.html'), 'utf8');
    const dom = new JSDOM(html);
    t.after(() => dom.window.close());
    assert.ok(dom.window.document.documentElement.textContent.includes(title), `${product}/${route}`);
    assert.ok(dom.window.document.documentElement.textContent.includes(text), `${product}/${route}`);
    assert.doesNotMatch(html, /data-atlas-action|atlas-controls/);
  }
  for (const [product] of expected) {
    const alias = await readFile(path.join(root, 'apps/web/dist/docs', product, 'index.html'), 'utf8');
    assert.ok(alias.includes('产品概览'), product);
  }
});
