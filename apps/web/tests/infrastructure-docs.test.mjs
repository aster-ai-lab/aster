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
  ['boot', '0.2.0', ['', 'application/', 'operations/']],
  ['orm', '0.3.1', ['', 'integration/', 'operations/']],
  ['search', '3.4.0', ['', 'integration/', 'operations/']],
  ['effect', '0.2.1', ['', 'integration/', 'operations/']],
  ['use', '0.3.3', ['', 'aix/', 'operations/']],
];

test('infrastructure products keep independent versions and guides', () => {
  for (const [id, version, slugs] of expected) {
    const project = projects.find((entry) => entry.id === id);
    assert.ok(project, id);
    assert.equal(project.defaultVersion, version);
    assert.deepEqual(project.versions[0].pages.map((page) => page.slug), slugs);
    assert.equal(project.versions[0].sourceRef, '8729d7c84a2d887c61dc206ba47bf8a63425e768');
  }
});

test('infrastructure pages preserve public contracts and product boundaries', async (t) => {
  const expectations = [
    ['boot', '0.2.0', '', 'Aster Boot 产品概览', 'Boot 的协议中立应用边界'],
    ['boot', '0.2.0', 'application/', 'Boot 应用与管线', 'BootFactory'],
    ['boot', '0.2.0', 'operations/', 'Boot 运行与验证', 'PostgresQueueBackend'],
    ['orm', '0.3.1', '', 'Aster ORM 产品概览', 'ORM 的查询编译与执行边界'],
    ['orm', '0.3.1', 'integration/', 'ORM 查询与接入', 'PostgresDialect'],
    ['orm', '0.3.1', 'operations/', 'ORM 迁移与运行验证', 'verify_required'],
    ['search', '3.4.0', '', 'Aster Search 产品概览', 'Search 的多源检索与结构边界'],
    ['search', '3.4.0', 'integration/', 'Search 检索接入', 'execute_cascade'],
    ['search', '3.4.0', 'operations/', 'Search 排名与运行验证', 'receipt_binding'],
    ['effect', '0.2.1', '', 'Aster Effect 产品概览', 'Effect 的程序值与事实日志循环'],
    ['effect', '0.2.1', 'integration/', 'Effect 与 Harness 接入', 'coding_actor'],
    ['effect', '0.2.1', 'operations/', 'Effect 恢复与运行验证', 'DuplicateTransition'],
    ['use', '0.3.3', '', 'Aster Use 产品概览', 'Use 的 AIX 包与计划应用边界'],
    ['use', '0.3.3', 'aix/', 'AIX 包与接入', 'schema_version = 3'],
    ['use', '0.3.3', 'operations/', 'Use 运行与恢复', 'Plan Digest'],
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
