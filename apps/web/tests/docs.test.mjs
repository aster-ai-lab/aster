import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFile, writeFile, mkdtemp, rm, mkdir } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import { createProcessor } from '@mdx-js/mdx';
import remarkGfm from 'remark-gfm';
import { buildDocs, pages, validateProjects } from '../scripts/build_docs.mjs';
import { projects } from '../scripts/docs_components.mjs';
import { documentPages } from '../scripts/docs_catalog.mjs';

const require = createRequire(import.meta.url);
const root = fileURLToPath(new URL('../../../', import.meta.url));
const publicRoot = path.join(root, 'apps/web/public');
const { JSDOM } = require(process.env.JS_DOM_MODULE || path.join(root, 'packages/ui/node_modules/jsdom'));

test('the product catalog refuses unsafe paths, duplicate versions and invalid defaults', () => {
  const project = projects[0];
  const version = project.versions[0];
  const withVersion = (entry) => [{ ...project, versions: [entry] }];
  assert.doesNotThrow(() => validateProjects(projects));
  assert.throws(() => validateProjects([project, project]), /duplicate project/);
  for (const id of ['../private', undefined]) assert.throws(() => validateProjects([{ ...project, id }]), /Invalid/);
  assert.throws(() => validateProjects([{ ...project, unexpected: true }]), /Unknown/);
  assert.throws(() => validateProjects([{ ...project, defaultVersion: 'missing' }]), /default version/);
  assert.throws(() => validateProjects([{ ...project, versions: [version, version] }]), /duplicate version/);
  assert.throws(() => validateProjects(withVersion({ ...version, id: '../' })), /Invalid/);
  assert.throws(() => validateProjects(withVersion({ ...version, id: 'index.html' })), /Invalid/);
  assert.throws(() => validateProjects(withVersion({ ...version, sourceRef: '../secret' })), /source ref/);
  assert.throws(() => validateProjects(withVersion({ ...version, components: '../shared' })), /components/);
  assert.throws(() => validateProjects(withVersion({ ...version, pages: version.pages.slice(1) })), /landing page/);
  assert.throws(() => validateProjects(withVersion({ ...version, pages: [{ ...version.pages[0], section: '' }] })), /Invalid page section/);
  assert.throws(() => validateProjects(withVersion({ ...version, pages: [{ ...version.pages[0], unexpected: true }] })), /Unknown/);
  for (const page of [{ ...version.pages[0], slug: '../' }, { ...version.pages[0], file: '../private' }, { ...version.pages[0], file: undefined }, version.pages[0]]) {
    assert.throws(() => validateProjects(withVersion({ ...version, pages: [...version.pages, page] })), /Invalid or duplicate document/);
  }
  assert.throws(() => validateProjects(withVersion({ ...version, pages: [...version.pages, { ...version.pages[0], slug: 'another/' }] })), /Duplicate document file/);
  assert.throws(() => validateProjects([{ ...project, versions: [...project.versions, { ...version, id: 'tutorial', pages: [version.pages[0]] }] }]), /Duplicate documentation route/);
  assert.doesNotThrow(() => validateProjects([project, { ...project, id: 'second-product' }]));
});

test('two products and two versions keep content, navigation, source refs and defaults isolated', async (t) => {
  const temporary = await mkdtemp(path.join(os.tmpdir(), 'aster-doc-versions-'));
  t.after(() => rm(temporary, { recursive: true, force: true }));
  const contentRoot = path.join(temporary, 'content');
  const outputRoot = path.join(temporary, 'public');
  const page = (slug) => ({ slug, file: slug ? slug.slice(0, -1) : 'index', label: slug || 'Overview', title: slug || 'Overview', description: 'Fixture article' });
  const entries = [
    { id: 'alpha', title: 'Alpha', label: 'Alpha SDK', description: 'Alpha product', defaultVersion: '2.0', versions: [
      { id: '1.0', label: '1.0', sourceRef: 'alpha-v1', components: '_components', pages: [page(''), page('guide/')] },
      { id: '2.0', label: '2.0', sourceRef: 'main', components: '_components', pages: [page(''), page('guide/'), page('new-only/')] },
    ] },
    { id: 'beta', title: 'Beta', label: 'Beta SDK', description: 'Beta product', defaultVersion: '1.0', versions: [
      { id: '1.0', label: '1.0', sourceRef: 'beta-v1', pages: [page('')] },
    ] },
  ];
  await mkdir(contentRoot, { recursive: true });
  await writeFile(path.join(contentRoot, 'index.mdx'), 'export const meta = {title:"Products",label:"Products",description:"Products",updated:"2026-10-26"};\n\n## Products\n\n<ProjectCards />\n');
  for (const project of entries) {
    for (const version of project.versions) {
      const directory = path.join(contentRoot, project.id, version.id);
      await mkdir(directory, { recursive: true });
      if (version.components) {
        await writeFile(path.join(directory, '_components.mjs'), `export function VersionNote() { return ${JSON.stringify('helper-' + version.id)}; }`);
        await mkdir(path.join(directory, '_diagrams'));
        await writeFile(path.join(directory, '_diagrams/same.mmd'), `flowchart TB\naccTitle: Version diagram\naccDescr: Scoped source\nA[Diagram ${version.id}] --> B[End node]\n`);
      }
      for (const chapter of version.pages) await writeFile(path.join(directory, `${chapter.file}.mdx`), `export const meta = {title:${JSON.stringify(project.title + ' ' + version.id)},label:"Page",description:"Fixture",updated:"2026-10-26"};\n\n## Content\n\n${project.id}-${version.id}-${chapter.file}\n\n[Source](https://github.com/aster-ai-lab/aster/blob/main/crates/code/core/src/lib.rs)\n${version.components ? '\n<VersionNote />\n\n<PeleDiagram name="same" label="Scoped diagram" description="Versioned diagram source" />\n' : ''}`);
    }
  }
  await buildDocs({ entries, contentRoot, outputRoot });
  const read = async (route) => {
    const dom = new JSDOM(await readFile(path.join(outputRoot, route, 'index.html'), 'utf8'), { url: `https://aster-ai-lab.github.io/aster/docs/${route}` });
    t.after(() => dom.window.close());
    return dom.window.document;
  };
  const old = await read('alpha/1.0/guide/');
  assert.ok(old.querySelector('.prose').textContent.includes('alpha-1.0-guide'));
  assert.ok([...old.querySelectorAll('.chapter-nav a')].slice(1).every((node) => node.href.includes('/docs/alpha/1.0/')));
  assert.ok(old.querySelector('.prose a').href.includes('/blob/alpha-v1/'));
  assert.ok(old.querySelector('.resource-links a').href.includes('/tree/alpha-v1/'));
  assert.ok(old.querySelector('.docs-version').textContent.includes('Alpha 1.0'));
  assert.ok(old.querySelector('.prose').textContent.includes('helper-1.0'));
  assert.ok(old.querySelector('.pele').textContent.includes('Diagram 1.0'));
  const current = await read('alpha/2.0/new-only/');
  const oldChoice = current.querySelector('.docs-switcher:last-child a[href$="alpha/1.0/"]');
  assert.ok(oldChoice.textContent.includes('此版本无本篇'));
  assert.ok(current.querySelector('.docs-switcher:first-child a[href$="beta/1.0/"]'));
  assert.ok(current.querySelector('.prose').textContent.includes('helper-2.0'));
  assert.ok(current.querySelector('.pele').textContent.includes('Diagram 2.0'));
  const alias = await read('alpha/guide/');
  assert.ok(alias.querySelector('.prose').textContent.includes('alpha-2.0-guide'));
  assert.ok(alias.querySelector('link[rel=canonical]').href.endsWith('/docs/alpha/2.0/guide/'));
  const previous = await readFile(path.join(outputRoot, 'alpha/1.0/guide/index.html'), 'utf8');
  await writeFile(path.join(contentRoot, 'alpha/2.0/guide.mdx'), (await readFile(path.join(contentRoot, 'alpha/2.0/guide.mdx'), 'utf8')) + '\nOnly version two changed.\n');
  await writeFile(path.join(contentRoot, 'alpha/2.0/_diagrams/same.mmd'), 'flowchart TB\naccTitle: Version diagram\naccDescr: Scoped source\nA[Changed version two] --> B[End node]\n');
  await buildDocs({ entries, contentRoot, outputRoot });
  assert.equal(await readFile(path.join(outputRoot, 'alpha/1.0/guide/index.html'), 'utf8'), previous);
  assert.equal(documentPages(entries).filter((entry) => entry.alias).length, 4);
  await mkdir(path.join(outputRoot, 'handwritten'));
  await writeFile(path.join(outputRoot, 'handwritten/index.html'), '<h1>Handwritten</h1>');
  entries[0].versions[1].pages.pop();
  await assert.rejects(buildDocs({ entries, contentRoot, outputRoot, check: true }), /Stale MDX|Obsolete generated/);
  await buildDocs({ entries, contentRoot, outputRoot });
  await assert.rejects(readFile(path.join(outputRoot, 'alpha/2.0/new-only/index.html')), /ENOENT/);
  await assert.rejects(readFile(path.join(outputRoot, 'alpha/new-only/index.html')), /ENOENT/);
  assert.equal(await readFile(path.join(outputRoot, 'handwritten/index.html'), 'utf8'), '<h1>Handwritten</h1>');
});

test('section navigation and chapter cards keep introductory and Meta Harness guides separate', async (t) => {
  const project = projects.find((entry) => entry.id === 'code');
  const version = project.versions.find((entry) => entry.id === project.defaultVersion);
  const sectionPages = version.pages.filter((page) => page.section === 'Meta Harness');
  assert.ok(sectionPages.length > 1);
  const source = await readFile(path.join(publicRoot, `docs/code/${version.id}/meta-harness/index.html`), 'utf8');
  const dom = new JSDOM(source, { url: `https://aster-ai-lab.github.io/aster/docs/code/${version.id}/meta-harness/` });
  t.after(() => dom.window.close());
  const { document } = dom.window;
  assert.deepEqual([...document.querySelectorAll('.chapter-nav .nav-section')].map((node) => node.textContent), ['设计原理', '构建终端智能体', 'Meta Harness', '功能参考']);
  assert.equal(document.querySelectorAll('.chapter-cards > a').length, sectionPages.length - 1);
  assert.ok(document.querySelector('.breadcrumb').textContent.includes('Meta Harness'));
  assert.equal(document.querySelector('.chapter-nav a[aria-current]').href, dom.window.location.href);
  for (const card of document.querySelectorAll('.chapter-cards > a')) assert.ok(new URL(card.href).pathname.startsWith(`/aster/docs/code/${version.id}/meta-harness/`));
  const overview = new JSDOM(await readFile(path.join(publicRoot, 'docs/code/index.html'), 'utf8'));
  t.after(() => overview.window.close());
  assert.equal(overview.window.document.querySelectorAll('.chapter-cards > a').length, version.pages.filter((page) => !page.section).length - 1);
  const labels = [...document.querySelectorAll('.chapter-nav .nav-number')].map((node) => node.textContent);
  assert.deepEqual(labels, ['01', '02', '03', '04']);
});

test('MDX builds deterministically and refuses stale generated pages', async (t) => {
  const outputRoot = await mkdtemp(path.join(os.tmpdir(), 'aster-mdx-'));
  t.after(() => rm(outputRoot, { recursive: true, force: true }));
  const files = await buildDocs({ outputRoot });
  assert.equal(files.length, pages.length + 1);
  await buildDocs({ check: true, outputRoot });
  const original = await readFile(files[0], 'utf8');
  await writeFile(files[0], '<h1>stale</h1>');
  await assert.rejects(buildDocs({ check: true, outputRoot }), /Stale MDX output/);
  await writeFile(files[0], original);
  await writeFile(files.at(-1), '/* stale */');
  await assert.rejects(buildDocs({ check: true, outputRoot }), /Stale syntax CSS/);
});

test('highlighted code preserves every authored character and remains labelled and keyboard-scrollable', async (t) => {
  const languages = new Set();
  for (const page of pages) {
    const mdx = await readFile(path.join(root, 'apps/web/content/docs', `${page.sourceFile}.mdx`), 'utf8');
    const fences = createProcessor().parse(mdx).children.filter((node) => node.type === 'code');
    const dom = new JSDOM(await readFile(path.join(publicRoot, 'docs', page.route, 'index.html'), 'utf8'));
    t.after(() => dom.window.close());
    const blocks = [...dom.window.document.querySelectorAll('pre')];
    assert.equal(blocks.length, fences.length);
    for (const [index, block] of blocks.entries()) {
      const fence = fences[index];
      const language = fence.lang || 'text';
      languages.add(language);
      assert.equal(block.querySelector('code').textContent, fence.value + '\n', `${page.file}: ${language} source changed`);
      assert.equal(block.dataset.language, language);
      assert.equal(block.getAttribute('tabindex'), '0');
      assert.ok(block.getAttribute('aria-label').includes(language));
      assert.equal(block.querySelectorAll('[style], script, button').length, 0);
      if (language !== 'text') assert.ok(block.querySelectorAll('code span[class^="syntax-"]').length > 1);
    }
  }
  assert.deepEqual([...languages].sort(), ['acl', 'bash', 'javascript', 'json', 'rust', 'sh', 'text', 'toml']);
});

test('the generated syntax palette meets 4.5:1 text contrast against its light background', async (t) => {
  const css = await readFile(path.join(publicRoot, 'docs/generated.css'), 'utf8');
  const dom = new JSDOM(`<style>${css}</style>`);
  t.after(() => dom.window.close());
  const rules = [...dom.window.document.styleSheets[0].cssRules].filter((rule) => rule.selectorText?.startsWith('.syntax-'));
  const luminance = (hex) => {
    const digits = hex.replace('#', '');
    const normalized = digits.length === 3 ? [...digits].map((digit) => digit + digit).join('') : digits;
    return [0.2126, 0.7152, 0.0722].reduce((sum, weight, index) => {
      const channel = parseInt(normalized.slice(index * 2, index * 2 + 2), 16) / 255;
      return sum + weight * (channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4);
    }, 0);
  };
  const background = rules.map((rule) => rule.style.getPropertyValue('--shiki-light-bg')).find(Boolean);
  assert.ok(background);
  const colors = new Set(rules.map((rule) => rule.style.getPropertyValue('--shiki-light')).filter(Boolean));
  assert.ok(colors.size >= 5);
  for (const color of colors) {
    const [dark, light] = [luminance(color), luminance(background)].sort((a, b) => a - b);
    assert.ok((light + 0.05) / (dark + 0.05) >= 4.5, `${color} fails code-text contrast on ${background}`);
  }
});

test('Pele renders the six architecture diagrams as labelled static SVG with external styling', async (t) => {
  for (const [route, type] of [['code/', 'flowchart'], ['code/workflow/', 'sequence'], ['code/meta-harness/', 'flowchart'], ['code/tutorial/', 'flowchart'], ['code/tutorial/permissions/', 'flowchart'], ['code/design/rsi/', 'flowchart']]) {
    const dom = new JSDOM(await readFile(path.join(publicRoot, 'docs', route, 'index.html'), 'utf8'));
    t.after(() => dom.window.close());
    const svg = dom.window.document.querySelector(`.pele-diagram svg.pele-${type}`);
    assert.ok(svg, route);
    assert.equal(svg.getAttribute('role'), 'img');
    assert.ok(svg.querySelector('title').textContent.trim());
    assert.ok(svg.querySelector('desc').textContent.trim());
    assert.equal(svg.querySelectorAll('[style], style, script, foreignObject, image, a').length, 0);
    assert.equal(svg.hasAttribute('style'), false);
    assert.ok(svg.getAttribute('class').includes('pele-style-'));
  }
});

test('the terminal-agent tutorial has twelve ordered chapters and project-local neighboring pages', async (t) => {
  const project = projects.find((entry) => entry.id === 'code');
  const version = project.versions.find((entry) => entry.id === project.defaultVersion);
  const tutorial = version.pages.filter((page) => page.section === '构建终端智能体');
  assert.equal(tutorial.length, 13);
  assert.equal(tutorial[0].slug, 'tutorial/');
  const landing = new JSDOM(await readFile(path.join(publicRoot, `docs/code/${version.id}/tutorial/index.html`), 'utf8'), { url: `https://aster-ai-lab.github.io/aster/docs/code/${version.id}/tutorial/` });
  t.after(() => landing.window.close());
  const cards = [...landing.window.document.querySelectorAll('.chapter-cards > a')];
  assert.equal(cards.length, 12);
  assert.deepEqual(cards.map((card) => card.querySelector('.step-number').textContent), Array.from({ length: 12 }, (_, index) => String(index + 1).padStart(2, '0')));
  for (const [index, page] of tutorial.slice(1).entries()) {
    assert.ok(cards[index].href.endsWith(`/docs/code/${version.id}/${page.slug}`));
    const dom = new JSDOM(await readFile(path.join(publicRoot, 'docs/code', version.id, page.slug, 'index.html'), 'utf8'));
    t.after(() => dom.window.close());
    assert.ok(dom.window.document.querySelector('h1').textContent.startsWith(String(index + 1).padStart(2, '0') + ' ·'));
    assert.ok(dom.window.document.querySelector('.chapter-nav a[aria-current]').getAttribute('href').endsWith(`code/${version.id}/${page.slug}`));
    const neighbors = [...dom.window.document.querySelectorAll('.page-navigation a')].map((node) => node.getAttribute('href'));
    assert.ok(neighbors.every((href) => href.includes(`code/${version.id}/`)));
  }
});

test('generated documents preserve project URLs, active navigation and accessible diagrams without JavaScript', async (t) => {
  await buildDocs({ check: true });
  for (const page of pages) {
    const source = await readFile(path.join(publicRoot, 'docs', page.route, 'index.html'), 'utf8');
    const dom = new JSDOM(source, { url: `https://aster-ai-lab.github.io/aster/docs/${page.route}` });
    t.after(() => dom.window.close());
    const { document } = dom.window;
    assert.equal(document.querySelectorAll('main h1').length, 1);
    assert.equal(document.querySelectorAll('.docs-resources').length, 1);
    assert.equal(document.querySelector('.docs-sidebar .docs-resources'), null);
    assert.ok(document.querySelector('.docs-toc .docs-resources .docs-version'));
    assert.equal(document.querySelector('link[rel=canonical]').href, `https://aster-ai-lab.github.io/aster/docs/${page.canonicalRoute}`);
    assert.equal(document.querySelectorAll('select, datalist, iframe').length, 0);
    const scripts = [...document.querySelectorAll('script')];
    assert.equal(scripts.length, 1);
    assert.ok(scripts[0].src.endsWith('/docs/navigation.js'));
    assert.ok(scripts[0].defer);
    assert.equal(scripts[0].textContent, '');
    assert.equal(document.querySelector('.chapter-nav a[aria-current]').href, `https://aster-ai-lab.github.io/aster/docs/${page.canonicalRoute}`);
    assert.equal(document.querySelector('.mobile-navigation a[aria-current]').href, `https://aster-ai-lab.github.io/aster/docs/${page.canonicalRoute}`);
    assert.equal(document.querySelector('.docs-brand').href, 'https://aster-ai-lab.github.io/aster/');
    const ids = [...document.querySelectorAll('[id]')].map((node) => node.id);
    assert.equal(ids.length, new Set(ids).size);
    for (const link of document.querySelectorAll('.toc-content nav a')) {
      assert.ok(document.getElementById(decodeURIComponent(new URL(link.href).hash.slice(1))));
    }
    for (const svg of document.querySelectorAll('svg')) {
      assert.equal(svg.getAttribute('role'), 'img');
      for (const id of svg.getAttribute('aria-labelledby').split(' ')) assert.ok(document.getElementById(id).textContent.trim());
    }
    for (const node of document.querySelectorAll('[href], img[src]')) {
      const url = new URL(node.getAttribute('href') || node.getAttribute('src'), dom.window.location.href);
      if (url.origin !== dom.window.location.origin || node.getAttribute('rel') === 'canonical') continue;
      assert.ok(url.pathname.startsWith('/aster/'), `project prefix lost: ${url}`);
      let filename = path.join(publicRoot, decodeURIComponent(url.pathname.slice('/aster/'.length)));
      if (url.pathname.endsWith('/')) filename = path.join(filename, 'index.html');
      assert.ok((await readFile(filename)).length > 0, `missing page or asset: ${url}`);
    }
    if (page.project) {
      assert.ok(document.querySelector('.breadcrumb').textContent.includes(page.project.title));
      assert.equal(document.querySelector('.page-navigation') !== null, true);
    } else {
      assert.equal(document.querySelectorAll('.project-cards > a').length, projects.length);
      assert.equal(document.querySelector('.page-navigation'), null);
    }
  }
});


test('current Core and ACP coverage indexes match the authoritative public contract sets', async () => {
  const core = projects.find((entry) => entry.id === 'code');
  const acp = projects.find((entry) => entry.id === 'acp');
  assert.equal(core.versions.length, 2);
  for (const [project, crate] of [[core, 'core'], [acp, 'acp']]) {
    const cargo = await readFile(path.join(root, `crates/code/${crate}/Cargo.toml`), 'utf8');
    assert.equal(cargo.match(/^version = "([^"]+)"/m)[1], project.defaultVersion);
  }
  const source = await readFile(path.join(root, 'crates/code/core/src/sdk_capabilities.rs'), 'utf8');
  const specs = source.split('const CAPABILITY_SPECS:')[1].split('\n];')[0].split('CapabilitySpec {').slice(1);
  const actual = specs.map((spec) => ({
    id: spec.match(/id: "([^"]+)"/)[1],
    operations: [...spec.split('operations: &[')[1].split(']')[0].matchAll(/"([^"]+)"/g)].map((match) => match[1]),
    hostOwned: spec.includes('host_owned: true'),
    tier: spec.includes('CapabilityTier::Advanced') ? 'advanced' : 'baseline',
  }));
  const coreCoverage = JSON.parse(await readFile(path.join(root, `apps/web/content/docs/code/${core.defaultVersion}/_coverage.json`), 'utf8'));
  assert.deepEqual(coreCoverage.map(({ id, operations, hostOwned, tier }) => ({ id, operations, hostOwned, tier })), actual);
  const methodsSource = await readFile(path.join(root, 'crates/code/acp/src/agent/host_methods.rs'), 'utf8');
  const parser = methodsSource.split('fn parse(method: &str)')[1].split('fn as_str')[0];
  const methods = [...parser.matchAll(/"((?:a3s|x\.ai)\/[^"\n]+)"/g)].map((match) => match[1]);
  const acpCoverage = JSON.parse(await readFile(path.join(root, `apps/web/content/docs/acp/${acp.defaultVersion}/_coverage.json`), 'utf8'));
  assert.deepEqual(acpCoverage.map((record) => record.method).sort(), methods.sort());
  for (const [project, records] of [[core, coreCoverage], [acp, acpCoverage]]) {
    for (const record of records) {
      const base = new URL(`https://docs.invalid/${project.id}/${project.defaultVersion}/reference/`);
      const target = new URL(record.page, base);
      const page = pages.find((entry) => entry.canonicalRoute === target.pathname.slice(1));
      assert.ok(page, `${project.id}: missing coverage target ${target.pathname}`);
      const content = await readFile(path.join(root, 'apps/web/content/docs', page.sourceFile + '.mdx'), 'utf8');
      assert.ok(content.includes('## 实现依据'), `${page.sourceFile}: implementation references missing`);
    }
  }
});

test('default aliases resolve cross-product and version links from their pinned source location', async (t) => {
  for (const route of ['code/', 'code/reference/', 'acp/', 'acp/reference/']) {
    const dom = new JSDOM(await readFile(path.join(publicRoot, 'docs', route, 'index.html'), 'utf8'), { url: `https://aster-ai-lab.github.io/aster/docs/${route}` });
    t.after(() => dom.window.close());
    const opposite = route.startsWith('code/') ? 'acp' : 'code';
    const link = [...dom.window.document.querySelectorAll('.prose a')].find((node) => node.href.includes(`/docs/${opposite}/9.1.6/`));
    assert.ok(link, `${route}: missing pinned cross-product link`);
    assert.ok(!link.href.includes('/aster/acp/') && !link.href.includes('/aster/code/'));
  }
});

test('every Markdown table renders as an accessible table instead of pipe-delimited prose', async (t) => {
  let total = 0;
  const processor = createProcessor({ remarkPlugins: [remarkGfm] });
  const countTables = (node) => (node.type === 'table' ? 1 : 0) + (node.children ?? []).reduce((sum, child) => sum + countTables(child), 0);
  for (const page of pages.filter((entry) => !entry.alias)) {
    const source = await readFile(path.join(root, 'apps/web/content/docs', page.sourceFile + '.mdx'), 'utf8');
    const expected = countTables(processor.parse(source));
    const dom = new JSDOM(await readFile(path.join(publicRoot, 'docs', page.route, 'index.html'), 'utf8'));
    t.after(() => dom.window.close());
    const tables = dom.window.document.querySelectorAll('.prose .table-scroll > table[aria-label]');
    assert.equal(tables.length, expected, `${page.sourceFile}: Markdown table was lost`);
    for (const table of tables) {
      assert.equal(table.parentElement.getAttribute('role'), 'region');
      assert.equal(table.parentElement.tabIndex, 0);
      assert.ok(table.querySelectorAll('thead th[scope="col"]').length > 0);
      assert.ok(table.querySelectorAll('tbody tr').length > 0);
      assert.equal(table.querySelectorAll('[style]').length, 0);
    }
    total += expected;
  }
  assert.ok(total > 0, 'real articles must exercise GFM tables');
});

test('GFM supports table alignment, escaped pipes, task lists, deletion, links and footnotes', async (t) => {
  const temporary = await mkdtemp(path.join(os.tmpdir(), 'aster-gfm-'));
  t.after(() => rm(temporary, { recursive: true, force: true }));
  const contentRoot = path.join(temporary, 'content');
  const outputRoot = path.join(temporary, 'public');
  await mkdir(contentRoot);
  await writeFile(path.join(contentRoot, 'index.mdx'), `export const meta = {title:'Markdown',label:'Markdown',description:'Rendering fixture',updated:'2026-10-26'};

## 表格与行内元素

| 入口 | 状态 | 数量 |
| :--- | :---: | ---: |
| \`task\` | **可用** | 12 |
| 左\\|右 | ~~旧版~~ | 3 |

- [x] 完成检查
- [ ] 继续处理

https://github.com/aster-ai-lab/aster

一条注释[^one]。

[^one]: 注释正文。
`);
  await buildDocs({ entries: [], contentRoot, outputRoot });
  const dom = new JSDOM(await readFile(path.join(outputRoot, 'index.html'), 'utf8'));
  t.after(() => dom.window.close());
  const { document } = dom.window;
  assert.equal(document.querySelector('.table-scroll').getAttribute('aria-label'), '表格与行内元素');
  assert.deepEqual([...document.querySelectorAll('thead th')].map((cell) => cell.dataset.align), ['left', 'center', 'right']);
  assert.equal(document.querySelector('tbody tr:nth-child(2) td').textContent, '左|右');
  assert.equal(document.querySelector('tbody code').textContent, 'task');
  assert.equal(document.querySelector('tbody del').textContent, '旧版');
  const checkboxes = [...document.querySelectorAll('.task-list-item input')];
  assert.equal(checkboxes.length, 2);
  assert.ok(checkboxes.every((input) => input.disabled));
  assert.deepEqual(checkboxes.map((input) => input.checked), [true, false]);
  assert.ok(document.querySelector('.prose a[href="https://github.com/aster-ai-lab/aster"]'));
  for (const link of document.querySelectorAll('[data-footnote-ref], [data-footnote-backref]')) assert.ok(document.getElementById(decodeURIComponent(link.hash.slice(1))));
  assert.equal(document.querySelectorAll('[style], iframe, select, .prose script').length, 0);
});

test('chapter navigation retains its scroll across page loads and isolates product/version state', async (t) => {
  const code = await readFile(path.join(publicRoot, 'docs/navigation.js'), 'utf8');
  const stored = new Map();
  const open = async (route, height = 320) => {
    const dom = new JSDOM(await readFile(path.join(publicRoot, 'docs', route, 'index.html'), 'utf8'), { url: `https://aster-ai-lab.github.io/aster/docs/${route}`, runScripts: 'outside-only' });
    t.after(() => dom.window.close());
    Object.defineProperty(dom.window, 'sessionStorage', { value: { getItem: (key) => stored.get(key) ?? null, setItem: (key, value) => stored.set(key, value) } });
    const menu = dom.window.document.querySelector('.docs-sidebar .chapter-nav');
    Object.defineProperty(menu, 'clientHeight', { value: height });
    dom.window.eval(code);
    menu.addEventListener('click', (event) => event.preventDefault());
    return { dom, menu };
  };
  const first = await open('code/9.1.6/tutorial/');
  first.menu.scrollTop = 540;
  first.menu.querySelector('a[href$="tutorial/memory/"]').click();
  const second = await open('code/9.1.6/tutorial/memory/');
  assert.equal(second.menu.scrollTop, 540);
  assert.equal(second.dom.window.scrollY, 0);
  assert.equal((await open('code/9.1.5/tutorial/memory/')).menu.scrollTop, 0);
  assert.equal((await open('acp/9.1.6/')).menu.scrollTop, 0);
  second.menu.scrollTop = 610;
  second.dom.window.dispatchEvent(new second.dom.window.Event('pagehide'));
  assert.equal((await open('code/9.1.6/tutorial/')).menu.scrollTop, 610);
  const mobile = await open('code/9.1.6/tutorial/', 0);
  mobile.menu.scrollTop = 0;
  mobile.dom.window.dispatchEvent(new mobile.dom.window.Event('pagehide'));
  assert.equal((await open('code/9.1.6/tutorial/')).menu.scrollTop, 610);
});

test('navigation remains usable when session storage is unavailable or corrupt', async (t) => {
  const code = await readFile(path.join(publicRoot, 'docs/navigation.js'), 'utf8');
  for (const storage of [
    { getItem: () => 'NaN', setItem: () => {} },
    { getItem: () => '-10', setItem: () => {} },
    { getItem: () => { throw new Error('blocked'); }, setItem: () => { throw new Error('blocked'); } },
  ]) {
    const dom = new JSDOM(await readFile(path.join(publicRoot, 'docs/code/index.html'), 'utf8'), { url: 'https://aster-ai-lab.github.io/aster/docs/code/', runScripts: 'outside-only' });
    t.after(() => dom.window.close());
    Object.defineProperty(dom.window, 'sessionStorage', { value: storage });
    const menu = dom.window.document.querySelector('.chapter-nav');
    Object.defineProperty(menu, 'clientHeight', { value: 300 });
    assert.doesNotThrow(() => dom.window.eval(code));
    assert.equal(menu.scrollTop, 0);
    assert.doesNotThrow(() => dom.window.dispatchEvent(new dom.window.Event('pagehide')));
    assert.ok(menu.querySelector('a[aria-current="page"]'));
  }
});
