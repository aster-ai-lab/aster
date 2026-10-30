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
const sourceRef = '8729d7c84a2d887c61dc206ba47bf8a63425e768';

test('ACL and Flow retain their own versions and all three guide routes', () => {
  for (const [id, version] of [['acl', '0.3.0'], ['flow', '2.0.0']]) {
    const project = projects.find((entry) => entry.id === id);
    assert.ok(project, id);
    assert.equal(project.defaultVersion, version);
    assert.equal(project.versions[0].sourceRef, sourceRef);
    assert.deepEqual(project.versions[0].pages.map((page) => page.slug), ['', 'integration/', 'operations/']);
  }
});

test('all ACL and Flow pages render readable content, pinned sources and working version aliases', async (t) => {
  for (const id of ['acl', 'flow']) {
    const project = projects.find((entry) => entry.id === id);
    const version = project.versions[0];
    for (const page of version.pages) {
      const route = `${id}/${version.id}/${page.slug}`;
      const html = await readFile(path.join(root, 'apps/web/dist/docs', route, 'index.html'), 'utf8');
      const dom = new JSDOM(html, { url: `https://aster-ai-lab.github.io/aster/docs/${route}` });
      t.after(() => dom.window.close());
      const document = dom.window.document;
      assert.equal(document.querySelector('h1').textContent, page.title);
      assert.equal(document.querySelector('link[rel="canonical"]').href, dom.window.location.href);
      const sources = [...document.querySelectorAll('a.source-ref')];
      assert.ok(sources.length >= 2, route);
      for (const link of sources) assert.ok(link.href.includes(`/blob/${sourceRef}/crates/${id}/`) && /#L\d+/.test(link.href), link.href);
      assert.ok(document.querySelector('.chapter-nav a[aria-current="page"]'), route);
      if (!page.slug) assert.ok(document.querySelector('.diagram svg'), `missing ${id} architecture diagram`);
      const alias = await readFile(path.join(root, 'apps/web/dist/docs', id, page.slug, 'index.html'), 'utf8');
      const aliasDom = new JSDOM(alias, { url: `https://aster-ai-lab.github.io/aster/docs/${id}/${page.slug}` });
      t.after(() => aliasDom.window.close());
      assert.equal(aliasDom.window.document.querySelector('link[rel="canonical"]').href, dom.window.location.href);
      assert.equal(aliasDom.window.document.querySelector('h1').textContent, page.title);
    }
  }
});

test('the public documentation directory exposes both new products and their indexed chapters', async (t) => {
  const html = await readFile(path.join(root, 'apps/web/dist/docs/index.html'), 'utf8');
  const dom = new JSDOM(html, { url: 'https://aster-ai-lab.github.io/aster/docs/' });
  t.after(() => dom.window.close());
  const search = JSON.parse(await readFile(path.join(root, 'apps/web/dist/docs/search-index.json'), 'utf8'));
  for (const id of ['acl', 'flow']) {
    const project = projects.find((entry) => entry.id === id);
    const target = `https://aster-ai-lab.github.io/aster/docs/${id}/${project.defaultVersion}/`;
    assert.ok([...dom.window.document.querySelectorAll('a')].some((link) => link.href === target && link.textContent.includes(project.title)), id);
    const entries = search.documents.filter((entry) => entry.project === id);
    assert.deepEqual(entries.map((entry) => entry.route).sort(), project.versions[0].pages.map((page) => `${id}/${project.defaultVersion}/${page.slug}`).sort());
    for (const page of project.versions[0].pages) {
      const entry = entries.find((record) => record.route === `${id}/${project.defaultVersion}/${page.slug}`);
      assert.equal(entry.title, page.title);
      assert.equal(entry.version, project.defaultVersion);
      assert.ok(entry.text.length > 100, `${id}/${page.slug}`);
    }
  }
});

test('the sidebar, product cards and product switcher use the same functional reading order', async (t) => {
  const order = ['code', 'acp', 'acl', 'flow', 'effect', 'lane', 'event', 'memory', 'search', 'vec', 'apofasi', 'use', 'office', 'sandbox', 'boot', 'orm', 'gateway', 'power'];
  assert.deepEqual(projects.map((project) => project.id), order);
  const titles = order.map((id) => projects.find((project) => project.id === id).title);
  const html = await readFile(path.join(root, 'apps/web/dist/docs/index.html'), 'utf8');
  const dom = new JSDOM(html);
  t.after(() => dom.window.close());
  assert.deepEqual([...dom.window.document.querySelectorAll('.chapter-nav a')].map((link) => link.textContent.trim()), titles);
  const article = await readFile(path.join(root, 'apps/web/dist/docs/acl/0.3.0/index.html'), 'utf8');
  const page = new JSDOM(article);
  t.after(() => page.window.close());
  assert.deepEqual([...page.window.document.querySelectorAll('#docs-switcher-product-menu a')].map((link) => link.textContent.trim()), titles);
  const cards = [...dom.window.document.querySelectorAll('.project-cards > a')];
  assert.deepEqual(cards.map((link) => link.querySelector('strong').textContent.trim()), titles);
});
