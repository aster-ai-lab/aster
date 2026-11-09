import { readFile, writeFile, mkdir, readdir, unlink, lstat } from 'node:fs/promises';
import { fileURLToPath, pathToFileURL } from 'node:url';
import path from 'node:path';
import { evaluate } from '@mdx-js/mdx';
import { createElement as h } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import * as runtime from 'react/jsx-runtime';
import rehypeShiki from '@shikijs/rehype';
import remarkGfm from 'remark-gfm';
import { transformerStyleToClass } from '@shikijs/transformers';
import aclGrammar from './acl.tmLanguage.json' with { type: 'json' };
import { buildNavigation } from './build_navigation.mjs';
import { DiagramScope, PeleDiagram, diagramThemeCss } from './docs_diagrams.mjs';
import { createSourceReferences } from './docs_sources.mjs';
import { PeleAtlasDiagram, SourceWalk, prepareWalks, buildAtlasAssets } from './docs_atlas.mjs';
import { createSearchEntry, serializeSearchIndex } from './docs_search.mjs';
import * as components from './docs_components.mjs';
import { projects, documentPages } from './docs_catalog.mjs';

const webRoot = fileURLToPath(new URL('../', import.meta.url));
const { sourceRepository } = components;
export { validateProjects } from './docs_catalog.mjs';
export const pages = documentPages(projects);

function headingPlugin(toc) {
  return () => (tree) => {
    const ids = new Set();
    const text = (node) => node.value ?? (node.children ?? []).map(text).join('');
    let section = '文档表格';
    for (const node of tree.children) {
      if (node.type === 'table') {
        node.data = { ...node.data, hProperties: { ...node.data?.hProperties, 'aria-label': section } };
      }
      if (node.type !== 'heading') continue;
      if (node.depth === 1) throw new Error('MDX uses metadata for the page h1; content headings start at h2');
      const title = text(node);
      section = title;
      const base = title.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, '-').replace(/^-|-$/g, '');
      let id = base;
      for (let suffix = 2; ids.has(id); suffix += 1) id = `${base}-${suffix}`;
      ids.add(id);
      node.data = { ...node.data, hProperties: { ...node.data?.hProperties, id } };
      if (node.depth === 2) toc.push({ id, title });
    }
  };
}

function chapterNav(docsRoot, chapter, entries) {
  if (!chapter.project) return entries.map((project) => h('a', { key: project.id, href: `${docsRoot}${project.id}/${project.defaultVersion}/` }, project.title));
  const { project, version } = chapter;
  const home = h('a', { key: 'home', className: 'nav-home', href: docsRoot }, h('span', { className: 'nav-home-icon', 'aria-hidden': true }, '←'), '返回文档首页');
  let section;
  return [home, h('div', { key: project.id, className: 'nav-project' }, h('p', { className: 'nav-label' }, `${project.title} · ${version.label}`),
    ...version.pages.flatMap((page, index) => {
      const heading = page.section && page.section !== section ? h('p', { key: `${page.file}-section`, className: 'nav-section' }, page.section) : null;
      section = page.section;
      return [heading, h('a', { key: page.file, href: `${docsRoot}${project.id}/${version.id}/${page.slug}`, 'aria-current': chapter.slug === page.slug ? 'page' : undefined },
        !page.section && h('span', { className: 'nav-number' }, String(index + 1).padStart(2, '0')), page.label)];
    }))];
}

function switchers(chapter, docsRoot, entries) {
  if (!chapter.project) return null;
  const { project, version } = chapter;
  const menu = (label, current, links) => {
    const menuId = `docs-switcher-${label === '产品' ? 'product' : 'version'}-menu`;
    return h('details', { className: 'docs-switcher' },
      h('summary', { 'aria-label': `选择${label}，当前${current}`, 'aria-expanded': 'false', 'aria-controls': menuId }, h('span', { className: 'switcher-label' }, label), h('span', { className: 'switcher-current' }, current), h('span', { className: 'switcher-chevron', 'aria-hidden': true })),
      h('div', { className: 'switcher-menu', id: menuId }, ...links));
  };
  return h('div', { className: 'docs-switchers', role: 'group', 'aria-label': '文档产品与版本' },
    menu('产品', project.title, entries.map((entry) => h('a', { key: entry.id, href: `${docsRoot}${entry.id}/${entry.defaultVersion}/`, 'aria-current': entry.id === project.id ? 'true' : undefined }, entry.title))),
    menu('版本', version.label, project.versions.map((entry) => {
      const samePage = entry.pages.some((page) => page.slug === chapter.slug);
      return h('a', { key: entry.id, href: `${docsRoot}${project.id}/${entry.id}/${samePage ? chapter.slug : ''}`, 'aria-current': entry.id === version.id ? 'true' : undefined },
        entry.label, entry.id === project.defaultVersion && h('small', null, '默认'), !samePage && h('small', null, '此版本无本篇，打开版本首页'));
    })));
}

function sourceLink(href, ref) {
  if (!href || !ref) return href;
  for (const type of ['blob', 'tree']) {
    const prefix = `${sourceRepository}/${type}/main/`;
    if (href.startsWith(prefix)) return `${sourceRepository}/${type}/${ref}/${href.slice(prefix.length)}`;
  }
  return href;
}

function contentLink(href, chapter, docsRoot) {
  if (!href?.startsWith('./') && !href?.startsWith('../')) return href;
  // Authored links belong to the pinned document, even when rendered at its default-version alias.
  const target = new URL(href, `https://docs.invalid/${chapter.canonicalRoute}`);
  return docsRoot + target.pathname.slice(1) + target.search + target.hash;
}

function searchDialog(chapter, entries, docsRoot) {
  const { project, version } = chapter;
  const versions = [...new Map(entries.flatMap((entry) => entry.versions.map((item) => [item.id, item.label]))).entries()];
  const filterRow = (label, key, options) => h('div', { className: 'docs-search-filter-row' },
    h('span', { className: 'docs-search-filter-label' }, label),
    h('div', { className: 'docs-search-filter-options', role: 'group', 'aria-label': `按${label}筛选` },
      ...options.map(({ value, text }) => h('button', { key: `${key}-${value}`, type: 'button', className: 'docs-search-filter', [`data-docs-search-${key}`]: value, 'aria-pressed': 'false' }, text))));
  return h('dialog', {
    className: 'docs-search-dialog', 'aria-labelledby': 'docs-search-title', 'data-docs-search-dialog': true,
    'data-current-project': project?.id ?? '', 'data-current-version': version?.id ?? '',
    'data-docs-root': docsRoot, 'data-index-url': `${docsRoot}search-index.json`,
  },
    h('div', { className: 'docs-search-panel' },
      h('div', { className: 'docs-search-top' },
        h('strong', { id: 'docs-search-title' }, '搜索 Aster 文档'),
        h('button', { type: 'button', className: 'docs-search-close', 'data-docs-search-close': true, 'aria-label': '关闭文档搜索' }, '×')),
      h('div', { className: 'docs-search-query' },
        h('input', { type: 'search', placeholder: '搜索标题、正文或代码…', autoComplete: 'off', 'data-docs-search-input': true, 'aria-label': '搜索文档', 'aria-controls': 'docs-search-results' }),
        h('kbd', null, 'ESC')),
      h('div', { className: 'docs-search-filters' },
        filterRow('产品', 'project', [{ value: '', text: '全部产品' }, ...entries.map((entry) => ({ value: entry.id, text: entry.title }))]),
        filterRow('版本', 'version', [{ value: '', text: '全部版本' }, ...versions.map(([value, text]) => ({ value, text }))])),
      h('p', { className: 'docs-search-status', role: 'status', 'aria-live': 'polite', 'data-docs-search-status': true }, '输入关键词开始搜索。'),
      h('ol', { id: 'docs-search-results', className: 'docs-search-results', 'aria-label': '搜索结果', 'data-docs-search-results': true })));
}

function layout(chapter, meta, Content, toc, entries, versionComponents, diagramScope, sources) {
  const docsRoot = '../'.repeat(chapter.route.split('/').filter(Boolean).length) || './';
  const siteRoot = docsRoot + '../';
  const { project, version } = chapter;
  const index = version?.pages.findIndex((page) => page.file === chapter.file) ?? -1;
  const projectRoot = project && `${docsRoot}${project.id}/${version.id}/`;
  const versionLabel = project ? `${project.title} ${version.label}` : '全部产品文档';
  const sourceRef = version?.sourceRef ?? 'main';
  const neighbor = (entry, label) => entry && h('a', { href: projectRoot + entry.slug }, h('span', null, label), h('strong', null, entry.label, label === '下一篇' ? ' →' : ''));
  return h('html', { lang: 'zh-CN' },
    h('head', null,
      h('meta', { charSet: 'utf-8' }), h('meta', { name: 'viewport', content: 'width=device-width, initial-scale=1' }),
      h('meta', { name: 'description', content: meta.description }), h('meta', { name: 'theme-color', content: '#ffffff' }),
      h('meta', { property: 'og:title', content: `${meta.title} · Aster 文档` }), h('meta', { property: 'og:description', content: meta.description }),
      h('title', null, `${meta.title} · Aster 文档`),
      h('link', { rel: 'canonical', href: `https://aster-ai-lab.github.io/aster/docs/${chapter.canonicalRoute}` }),
      h('link', { rel: 'icon', type: 'image/png', href: `${siteRoot}tenant/aster/favicon.png` }),
      h('link', { rel: 'stylesheet', href: `${siteRoot}transitions.css` }),
      h('link', { rel: 'stylesheet', href: `${docsRoot}styles.css` }), h('link', { rel: 'stylesheet', href: `${docsRoot}harness-demo.css` }), h('link', { rel: 'stylesheet', href: `${docsRoot}generated.css` }),
      h('script', { src: `${docsRoot}navigation.js`, defer: true }), h('script', { src: `${docsRoot}search.js`, defer: true })),
    h('body', null,
      h('a', { className: 'skip-link', href: '#content' }, '跳转到正文'),
      h('header', { className: 'docs-header' },
        h('a', { className: 'docs-brand', href: siteRoot, 'aria-label': 'Aster 首页' }, h('img', { src: `${siteRoot}tenant/aster/logo.svg`, alt: '', width: 30, height: 30 }), 'Aster'),
        h('span', { className: 'header-divider', 'aria-hidden': true }, '/'), h('a', { className: 'docs-wordmark', href: docsRoot }, '文档'),
        h('nav', { className: 'header-links', 'aria-label': '网站导航' },
          h('button', { type: 'button', className: 'docs-search-trigger', 'data-docs-search-open': true, 'aria-keyshortcuts': 'Meta+K Control+K' }, '搜索文档', h('kbd', null, '⌘ K')),
          h('a', { href: siteRoot }, '官网'), h('a', { href: sourceRepository }, 'GitHub ↗'), h('a', { className: 'download-link', href: `${siteRoot}download/` }, '下载 Aster', h('span', { 'aria-hidden': true }, ' ↗')))),
      searchDialog(chapter, entries, docsRoot),
      h('div', { className: 'docs-layout' },
        h('aside', { className: 'docs-sidebar' },
          h('div', { className: 'sidebar-heading' }, h('span', { className: 'sidebar-mark', 'aria-hidden': true }, '✳'), h('div', null, h('strong', null, '开发者文档'), h('span', null, '架构与接入说明'))),
          h('nav', { className: 'chapter-nav', 'aria-label': '文档导航', 'data-scroll-scope': JSON.stringify([project?.id ?? 'portal', version?.id ?? '']) }, chapterNav(docsRoot, chapter, entries))),
        h('main', { id: 'content', tabIndex: -1 },
          switchers(chapter, docsRoot, entries),
          h('details', { className: 'mobile-navigation' }, h('summary', null, '文档目录', h('span', { 'aria-hidden': true }, '＋')), h('nav', { 'aria-label': '移动文档导航' }, chapterNav(docsRoot, chapter, entries))),
          h('div', { className: 'breadcrumb' }, h('a', { href: docsRoot }, '文档'), project && h('span', { 'aria-hidden': true }, '/'), project && h('a', { href: projectRoot }, project.title), version && h('span', { 'aria-hidden': true }, '/'), version?.label, chapter.section && h('span', { 'aria-hidden': true }, '/'), chapter.section, h('span', { 'aria-hidden': true }, '/'), meta.label),
          h('article', null,
            h('div', { className: 'article-heading' }, h('p', { className: 'eyebrow' }, project ? `${project.title.toUpperCase()} / ${chapter.section || String(index + 1).padStart(2, '0')}` : 'ASTER DOCUMENTATION'), h('h1', null, meta.title), h('p', { className: 'article-description' }, meta.description), h('div', { className: 'article-meta' }, h('span', null, versionLabel), h('span', { 'aria-hidden': true }, '·'), h('time', { dateTime: meta.updated }, meta.updated))),
            h('div', { className: 'prose' }, h(DiagramScope.Provider, { value: diagramScope }, h(Content, { components: {
              ...components, ...versionComponents, PeleDiagram, PeleAtlasDiagram, SourceWalk, SourceRef: sources.SourceRef,
              a: ({ href, ...props }) => h('a', { ...props, href: contentLink(sourceLink(href, sourceRef), chapter, docsRoot) }),
              table: components.MarkdownTable,
              th: ({ align, style, ...props }) => h('th', { ...props, scope: 'col', 'data-align': align ?? style?.textAlign }),
              td: ({ align, style, ...props }) => h('td', { ...props, 'data-align': align ?? style?.textAlign }),
              ChapterCards: (props) => h(components.ChapterCards, { ...props, docsRoot, project, version }),
              ProjectCards: () => h(components.ProjectCards, { docsRoot, entries }),
            } })))),
          project && h('nav', { className: 'page-navigation', 'aria-label': '相邻文档' }, neighbor(version.pages[index - 1], '上一篇'), neighbor(version.pages[index + 1], '下一篇')),
          h('footer', { className: 'docs-footer' }, h('span', null, 'Aster 技术文档'), h('a', { href: `${sourceRepository}/blob/main/apps/web/content/docs/${chapter.sourceFile}.mdx` }, '编辑此页 ↗'))),
        h('aside', { className: 'docs-toc' },
          h('div', { className: 'toc-content' },
            h('nav', { 'aria-label': '本页目录' }, h('p', { className: 'nav-label' }, '本页内容'), ...toc.map(({ id, title }) => h('a', { key: id, href: `#${id}` }, title))),
            h('div', { className: 'toc-note' }, h('span', { className: 'mini-label' }, '版本说明'), h('p', null, version ? `本文对应 ${versionLabel}，运行时源码引用 ${sourceRef}。` : '先选择产品，再按版本查阅相应文档。'))),
          h('div', { className: 'docs-resources' },
            h(sources.SourceList),
            h('div', { className: 'resource-links' }, h('p', { className: 'nav-label' }, '源码与资料'), h('a', { href: `${sourceRepository}/tree/${sourceRef}/docs` }, '仓库文档 ↗'), h('a', { href: `${sourceRepository}/tree/${sourceRef}/crates` }, '浏览源码 ↗')),
            h('div', { className: 'docs-version' }, h('span', { className: 'version-dot', 'aria-hidden': true }), versionLabel))))));
}

export async function buildDocs({ check = false, outputRoot = path.join(webRoot, 'node_modules/.cache/aster-docs'), entries = projects, contentRoot = path.join(webRoot, 'content/docs') } = {}) {
  const files = [];
  const searchDocuments = [];
  const diagramStyles = new Map();
  const componentModules = new Map();
  const sourceCache = new Map();
  const walkCache = new Map();
  const codeStyles = transformerStyleToClass({ classPrefix: 'syntax-' });
  const highlighting = [rehypeShiki, {
    themes: { light: 'github-light-high-contrast' }, defaultColor: false,
    langs: ['rust', 'javascript', aclGrammar], lazy: true,
    defaultLanguage: 'text', addLanguageClass: true, stripEndNewline: false,
    transformers: [codeStyles, {
      name: 'aster:code-label',
      pre(node) {
        const language = this.options.lang;
        node.properties['data-language'] = language;
        node.properties['aria-label'] = `${language} 代码示例`;
        node.properties.tabIndex = 0;
      },
    }],
  }];
  for (const chapter of documentPages(entries)) {
    const sourcePath = path.join(contentRoot, `${chapter.sourceFile}.mdx`);
    let versionComponents = {};
    if (chapter.version?.components) {
      const modulePath = path.join(contentRoot, chapter.project.id, chapter.version.id, `${chapter.version.components}.mjs`);
      if (!componentModules.has(modulePath)) componentModules.set(modulePath, await import(pathToFileURL(modulePath)));
      versionComponents = componentModules.get(modulePath);
    }
    const toc = [];
    // Only repository-authored MDX is evaluated at build time; never accept remote/user input here.
    const { default: Content, meta } = await evaluate(await readFile(sourcePath, 'utf8'), { ...runtime, baseUrl: pathToFileURL(sourcePath), remarkPlugins: [remarkGfm, headingPlugin(toc)], remarkRehypeOptions: { footnoteLabel: '注释', footnoteBackLabel: '返回引用' }, rehypePlugins: [highlighting] });
    if (!meta?.title || !meta?.description || !meta?.label || !/^\d{4}-\d{2}-\d{2}$/.test(meta.updated ?? '') || toc.length === 0) throw new Error(`Missing document metadata or sections: ${chapter.file}`);
    const sources = createSourceReferences(chapter.version?.sourceRef ?? 'main', path.resolve(webRoot, '../..'), sourceCache);
    const walkDirectory = chapter.version && path.join(contentRoot, chapter.project.id, chapter.version.id, '_walks');
    if (walkDirectory && !walkCache.has(walkDirectory)) walkCache.set(walkDirectory, await prepareWalks(walkDirectory, sources, diagramStyles));
    const html = '<!doctype html>\n<!-- Generated from content/docs/*.mdx by npm run build. -->\n' + renderToStaticMarkup(layout(chapter, meta, Content, toc, entries, versionComponents, { chapter, contentRoot, styles: diagramStyles, sources, walks: walkCache.get(walkDirectory) }, sources)) + '\n';
    if (!chapter.alias) searchDocuments.push(createSearchEntry(chapter, meta, html));
    const target = path.join(outputRoot, chapter.route, 'index.html');
    if (check) {
      if (await readFile(target, 'utf8') !== html) throw new Error(`Stale MDX output: ${target}; run npm --prefix apps/web run build`);
    } else {
      await mkdir(path.dirname(target), { recursive: true });
      await writeFile(target, html);
    }
    files.push(target);
  }
  const searchTarget = path.join(outputRoot, 'search-index.json');
  const searchIndex = serializeSearchIndex(searchDocuments);
  if (check) {
    if (await readFile(searchTarget, 'utf8') !== searchIndex) throw new Error(`Stale search index: ${searchTarget}; run npm --prefix apps/web run build`);
  } else {
    await writeFile(searchTarget, searchIndex);
  }
  files.push(searchTarget);
  const cssTarget = path.join(outputRoot, 'generated.css');
  const css = '/* Generated by Shiki, Pele and Code Hike; run npm --prefix apps/web run build. */\n' + codeStyles.getCSS() + '\n' + diagramThemeCss + '\n' + [...diagramStyles].map(([name, style]) => `.${name}{${style}}`).join('\n') + '\n';
  if (check) {
    if (await readFile(cssTarget, 'utf8') !== css) throw new Error(`Stale syntax CSS: ${cssTarget}; run npm --prefix apps/web run build`);
  } else {
    await writeFile(cssTarget, css);
  }
  files.push(cssTarget);
  files.push(...await buildAtlasAssets({ contentRoot, outputRoot, check, repositoryRoot: path.resolve(webRoot, '../..') }));
  const expected = new Set(files.map((file) => path.resolve(file)));
  for (const relative of await readdir(outputRoot, { recursive: true })) {
    const target = path.join(outputRoot, relative);
    if (path.basename(relative) !== 'index.html' || expected.has(path.resolve(target))) continue;
    const info = await lstat(target);
    if (info.isSymbolicLink()) throw new Error(`Symlink documentation output: ${target}`);
    if (!info.isFile()) continue;
    if (!(await readFile(target, 'utf8')).startsWith('<!doctype html>\n<!-- Generated from content/docs/*.mdx by npm run build. -->')) continue;
    if (check) throw new Error(`Obsolete generated document: ${target}; run npm --prefix apps/web run build`);
    await unlink(target);
  }
  return files;
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  const args = process.argv.slice(2);
  if (args.some((arg) => arg !== '--check')) throw new Error('Usage: node scripts/build_docs.mjs [--check]');
  await buildNavigation({ check: args.includes('--check') });
  await buildDocs({ check: args.includes('--check') });
  console.log(`${args.includes('--check') ? 'Checked' : 'Built'} ${pages.filter((page) => !page.alias).length} MDX documents (${pages.length} routes) and generated CSS.`);
}
