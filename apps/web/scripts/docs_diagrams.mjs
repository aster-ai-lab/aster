import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import { createContext, createElement as h, useContext } from 'react';
import { render } from 'pele';
import { fromHtml } from 'hast-util-from-html';
import { toHtml } from 'hast-util-to-html';
import { extractStyles } from './docs_markup.mjs';

export const DiagramScope = createContext(null);
const theme = { bg: '#f6f8f5', fg: '#243029', muted: '#56645b', line: '#738078', surface: '#ffffff', 'surface-alt': '#edf3ed', border: '#86958a', accent: '#347653', radius: '8px', font: "Ubuntu, 'PingFang SC', 'Microsoft YaHei UI', 'Microsoft YaHei', sans-serif" };
const themeDeclarations = Object.entries(theme).map(([key, value]) => `--pele-${key}:${value}`).join(';');
export const diagramThemeCss = `.diagram .pele{${themeDeclarations}}`;

export function PeleDiagram({ name, label, description }) {
  const scope = useContext(DiagramScope);
  if (!scope?.chapter.version) throw new Error('Diagrams require an explicit product and version');
  if (!/^[a-z][a-z0-9-]*$/.test(name ?? '') || typeof label !== 'string' || !label.trim() || typeof description !== 'string' || !description.trim()) throw new Error('Diagram needs a safe name, label and description');
  const { chapter, contentRoot, styles } = scope;
  const identity = `${chapter.project.id}/${chapter.version.id}/${name}`;
  const source = readFileSync(path.join(contentRoot, chapter.project.id, chapter.version.id, '_diagrams', `${name}.mmd`), 'utf8');
  const { markup } = renderPele(source, identity, styles);
  return h('figure', { className: 'diagram pele-diagram' },
    h('figcaption', null, h('span', { className: 'diagram-kicker' }, '架构图'), h('span', { className: 'figure-label' }, label), description),
    h('div', { className: 'diagram-tools', hidden: true }, h('span', { className: 'diagram-tool-note' }, '保持原始字号，可横向浏览'), h('button', { type: 'button', 'data-lab-expand': true }, '展开图示 ↗')),
    h('div', { className: 'diagram-hint', 'aria-hidden': true }, '左右滑动查看完整图示 ↔'),
    h('div', { className: 'diagram-scroll', tabIndex: 0, role: 'region', 'aria-label': `${label}，可横向滚动`, dangerouslySetInnerHTML: { __html: markup } }));
}

export function renderPele(source, identity, styles, { title, description } = {}) {
  const prefix = 'diagram-' + createHash('sha256').update(identity).digest('hex').slice(0, 12);
  const { svg } = render(source, { idPrefix: prefix, fontFamily: 'Ubuntu, sans-serif', fontSize: 14, padding: 24, autoDirection: false, links: false, images: false });
  const tree = fromHtml(svg, { fragment: true });
  const element = tree.children.find((node) => node.tagName === 'svg');
  element.properties.role = 'img';
  element.properties.style += `;${themeDeclarations}`;
  if (title) {
    element.children = element.children.filter((node) => !['title', 'desc'].includes(node.tagName));
    element.children.unshift(
      { type: 'element', tagName: 'title', properties: { id: `${prefix}-title` }, children: [{ type: 'text', value: title }] },
      { type: 'element', tagName: 'desc', properties: { id: `${prefix}-description` }, children: [{ type: 'text', value: description }] },
      { type: 'element', tagName: 'rect', properties: { width: '100%', height: '100%', fill: 'var(--_bg)' }, children: [] });
    element.properties.ariaLabelledBy = `${prefix}-title ${prefix}-description`;
  }
  const standalone = toHtml(tree);
  const nodeIds = new Set();
  const collectNodes = (node) => {
    if (node.properties?.className?.includes('pele-node')) nodeIds.add(node.properties.dataId);
    for (const child of node.children ?? []) collectNodes(child);
  };
  collectNodes(tree);
  extractStyles(tree, styles, 'pele-style');
  return { markup: toHtml(tree), svg: standalone, nodeIds };
}
