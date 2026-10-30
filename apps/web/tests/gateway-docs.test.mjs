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
const project = projects.find((entry) => entry.id === 'gateway');

test('Gateway registration keeps version and product boundaries', () => {
  assert.ok(project);
  assert.equal(project.defaultVersion, '1.1.1');
  assert.deepEqual(project.versions[0].pages.map((page) => page.slug), ['', 'quickstart/', 'model-routing/', 'traffic/', 'operations/']);
  assert.equal(project.versions[0].sourceRef, '8729d7c84a2d887c61dc206ba47bf8a63425e768');
});

test('Gateway pages preserve quickstart, routing, snapshot and security contracts', async (t) => {
  const expectations = [
    ['', 'Aster Gateway 产品概览', 'Gateway 的流量职责与期望状态'],
    ['quickstart/', 'standalone 快速开始', 'a3s-gateway validate --config gateway.acl'],
    ['model-routing/', '模型路由、P/D 分离与缓存亲和', 'decode/prepare'],
    ['traffic/', '流量与快照', 'drain_managed_service'],
    ['operations/', '部署与安全', 'local_digest_store'],
  ];
  for (const [route, title, text] of expectations) {
    const html = await readFile(path.join(root, 'apps/web/dist/docs/gateway/1.1.1', route, 'index.html'), 'utf8');
    const dom = new JSDOM(html);
    t.after(() => dom.window.close());
    assert.ok(dom.window.document.documentElement.textContent.includes(title), route);
    assert.ok(dom.window.document.documentElement.textContent.includes(text), route);
    assert.doesNotMatch(html, /data-atlas-action|atlas-controls/);
  }
  const alias = await readFile(path.join(root, 'apps/web/dist/docs/gateway/index.html'), 'utf8');
  assert.ok(alias.includes('Aster Gateway 产品概览'));
});
