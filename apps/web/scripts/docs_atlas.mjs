import { readFileSync } from 'node:fs';
import { readFile, readdir, mkdir, writeFile, unlink } from 'node:fs/promises';
import path from 'node:path';
import { createElement as h, useContext } from 'react';
import { highlight, Pre, InnerLine, InnerToken } from 'codehike/code';
import { renderToStaticMarkup } from 'react-dom/server';
import { fromHtml } from 'hast-util-from-html';
import { toHtml } from 'hast-util-to-html';
import { extractStyles } from './docs_markup.mjs';
import highContrast from '@shikijs/themes/github-light-high-contrast';
import { DiagramScope, renderPele } from './docs_diagrams.mjs';

const safeName = (name) => typeof name === 'string' && /^[a-z][a-z0-9-]*$/.test(name);

export function renderAtlas(source, definition, identity, styles = new Map()) {
  if (!definition.title || !definition.description || !definition.steps?.length || definition.steps.some((step) => !step.title || !step.body)) throw new Error('Atlas needs a title, description and explanatory steps');
  const result = renderPele(source, identity, styles, definition);
  for (const step of definition.steps) for (const id of step.focus ?? []) {
    if (!result.nodeIds.has(id)) throw new Error(`Unknown Pele focus node: ${id}`);
  }
  return result;
}

export function PeleAtlasDiagram({ name, label, description }) {
  const scope = useContext(DiagramScope);
  if (!scope?.chapter.version || !safeName(name)) throw new Error('Atlas needs a version and safe name');
  const { chapter, contentRoot, styles, sources } = scope;
  const root = path.join(contentRoot, chapter.project.id, chapter.version.id);
  const definition = JSON.parse(readFileSync(path.join(root, '_atlas', `${name}.json`), 'utf8'));
  const source = readFileSync(path.join(root, '_diagrams', `${name}.mmd`), 'utf8');
  const { markup } = renderAtlas(source, definition, `${chapter.project.id}/${chapter.version.id}/${name}`, styles);
  const depth = chapter.route.split('/').filter(Boolean).length;
  const docsRoot = '../'.repeat(depth) || './';
  return h('figure', { className: 'diagram pele-diagram atlas-figure', 'data-diagram': name },
    h('figcaption', null, h('span', { className: 'diagram-kicker' }, '架构图'), h('span', { className: 'figure-label' }, label || definition.title), description || definition.description),
    h('div', { className: 'diagram-hint', 'aria-hidden': true }, '左右滑动查看完整图示 ↔'),
    h('div', { className: 'diagram-scroll', tabIndex: 0, role: 'region', 'aria-label': `${definition.title}，可横向滚动`, dangerouslySetInnerHTML: { __html: markup } }),
    h('ol', { className: 'atlas-steps' }, ...definition.steps.map((step, index) => h('li', { key: index },
      h('div', { className: 'atlas-step' }, h('span', { className: 'step-number' }, String(index + 1).padStart(2, '0')), h('span', { className: 'lab-step-label' }, step.title)),
      h('p', null, step.body), step.source && h(sources.SourceRef, step.source)))),
    h('p', { className: 'atlas-credit' }, h('a', { href: `${docsRoot}atlas/${chapter.project.id}-${name}.svg`, download: true }, '下载 SVG ↗')));
}

export async function prepareWalks(directory, sources, styles) {
  let files;
  try { files = await readdir(directory); } catch (error) { if (error.code === 'ENOENT') return new Map(); throw error; }
  const walks = new Map();
  for (const file of files.filter((name) => name.endsWith('.json')).sort()) {
    const walk = JSON.parse(await readFile(path.join(directory, file), 'utf8'));
    for (const step of walk.steps) {
      step.excerpt = sources.resolve(step.source);
      const code = await highlight({ value: step.excerpt.value, lang: step.lang || (step.source.path.endsWith('.rs') ? 'rust' : 'markdown'), meta: '' }, highContrast);
      code.annotations.push({ name: 'focus', query: '', fromLineNumber: step.excerpt.focus, toLineNumber: step.excerpt.focusEnd });
      const handlers = [
        { name: 'source-lines', Line: (props) => h(InnerLine, { merge: props, className: 'walk-line', 'data-line-number': step.excerpt.first + props.lineNumber - 1 }) },
        { name: 'focus', AnnotatedLine: (props) => h(InnerLine, { merge: props, className: 'walk-focus', 'data-focus': true }) },
        { name: 'tokens', Token: (props) => h(InnerToken, { merge: props, className: 'walk-token' }) },
      ];
      const markup = renderToStaticMarkup(h(Pre, { code, handlers, tabIndex: 0, 'aria-label': `${step.title}的真实源码`, 'data-codehike': '1.1.0' }));
      const tree = fromHtml(markup, { fragment: true });
      extractStyles(tree, styles, 'walk-style');
      step.markup = toHtml(tree);
      step.language = code.lang;
    }
    walks.set(file.slice(0, -5), walk);
  }
  return walks;
}

export function SourceWalk({ name }) {
  const { chapter, walks, sources, styles } = useContext(DiagramScope);
  if (!safeName(name) || !walks.has(name)) throw new Error(`Unknown source walk: ${name}`);
  const walk = walks.get(name);
  const countClass = `walk-count-${walk.steps.length}`;
  styles.set(countClass, `--walk-count:${walk.steps.length}`);
  const identity = `walk-${chapter.project.id}-${name}`;
  const stops = walk.steps.map((step, index) => {
    const id = `${identity}-${index}`;
    const path = step.source.path.split('/');
    const filename = path.pop();
    const narrative = h('div', { className: 'walk-narrative' },
      h('button', { type: 'button', id: `${id}-step`, disabled: true, 'data-walk-step': index, className: 'walk-title', 'aria-controls': `${id}-detail ${id}-code` },
        h('span', { className: 'step-number' }, String(index + 1).padStart(2, '0')), h('span', { className: 'lab-step-label' }, step.title), h('span', { className: 'walk-step-arrow', 'aria-hidden': true }, '↗')),
      h('div', { className: 'walk-detail', id: `${id}-detail` }, h('p', null, step.body),
        h('p', { className: 'walk-takeaway' }, step.takeaway), h(sources.SourceRef, step.source)));
    const code = h('div', { className: `walk-code is-context${step.language === 'markdown' ? ' is-wrapped' : ''}`, id: `${id}-code`, 'data-source-file': step.source.path, role: 'region', 'aria-labelledby': `${id}-step` },
      h('div', { className: 'walk-file' }, h('span', { className: 'walk-language' }, step.language === 'rust' ? 'RS' : 'MD'),
        h('div', { className: 'walk-file-location', title: step.source.path }, h('span', null, `${path.join('/')}/`), h('strong', null, filename)),
        h('a', { className: 'walk-file-source', href: step.excerpt.href, title: '在 GitHub 查看这段原文' }, step.excerpt.lines, ' ↗')),
      h('div', { className: 'walk-code-tools', hidden: true },
        h('button', { type: 'button', 'data-code-action': 'focus', 'aria-pressed': false }, '聚焦关键行'),
        h('button', { type: 'button', 'data-code-action': 'wrap', 'aria-pressed': step.language === 'markdown' }, '自动换行'),
        h('span', { className: 'walk-copy-status', 'data-copy-status': true, role: 'status' }),
        h('button', { type: 'button', 'data-code-action': 'copy' }, '复制代码'),
        h('button', { type: 'button', 'data-lab-expand': true }, '展开 ↗')),
      h('div', { className: 'walk-code-content', dangerouslySetInnerHTML: { __html: step.markup } }));
    return h('section', { key: index, className: 'walk-stop', 'data-walk-stop': index }, narrative, code);
  });
  return h('section', { className: `source-walk reading-lab ${countClass}`, 'data-source-walk': name, 'aria-label': walk.title },
    h('div', { className: 'walk-heading' }, h('div', null, h('p', { className: 'diagram-kicker' }, '源码漫游'), h('h3', null, walk.title)), h('span', { className: 'walk-position', 'data-walk-position': true, 'aria-hidden': true }, `${stops.length} 个步骤`)),
    h('p', { className: 'walk-intro' }, walk.intro),
    h('div', { className: 'walk-controls', hidden: true },
      h('div', { className: 'walk-arrows' }, h('button', { type: 'button', 'data-walk-action': 'previous', 'aria-label': '上一站' }, '←'), h('button', { type: 'button', 'data-walk-action': 'next', 'aria-label': '下一站' }, '→')),
      h('span', { className: 'walk-inline-position', 'data-walk-inline-position': true, 'aria-hidden': true }, ''), h('span', { className: 'walk-hint' }, '选择步骤，对照关键代码'), h('button', { type: 'button', 'data-walk-action': 'all', 'aria-pressed': false }, '连续阅读')),
    h('div', { className: 'walk-stops' }, ...stops),
    h('p', { className: 'walk-status sr-only', role: 'status', 'aria-live': 'polite' }),
    h('p', { className: 'walk-footnote' }, '源码与本文版本一致，保留原始行号。'));
}

export async function buildAtlasAssets({ contentRoot, outputRoot, check, repositoryRoot }) {
  let catalog;
  try { catalog = JSON.parse(await readFile(path.join(contentRoot, 'atlas.json'), 'utf8')); } catch (error) { if (error.code === 'ENOENT') return []; throw error; }
  const files = [];
  const emit = async (target, value) => {
    if (check) { if (await readFile(target, 'utf8') !== value) throw new Error(`Stale atlas asset: ${target}`); }
    else { await mkdir(path.dirname(target), { recursive: true }); await writeFile(target, value); }
    files.push(target);
  };
  for (const entry of catalog.diagrams) {
    const root = path.join(contentRoot, entry.product, entry.version);
    const definition = JSON.parse(await readFile(path.join(root, '_atlas', `${entry.name}.json`), 'utf8'));
    const source = await readFile(path.join(root, '_diagrams', `${entry.name}.mmd`), 'utf8');
    const { svg } = renderAtlas(source, definition, `${entry.product}/${entry.version}/${entry.name}`);
    await emit(path.join(outputRoot, 'atlas', `${entry.product}-${entry.name}.svg`), svg);
    if (entry.readme) await emit(path.join(repositoryRoot, 'assets/readme', `${entry.readme}.svg`), svg);
  }
  for (const name of ['seecode-symbol.png', 'seecode-word.png']) {
    const obsolete = path.join(outputRoot, 'atlas', name);
    try { await readFile(obsolete); } catch (error) { if (error.code === 'ENOENT') continue; throw error; }
    if (check) throw new Error(`Obsolete diagram watermark: ${obsolete}`);
    await unlink(obsolete);
  }
  return files;
}
