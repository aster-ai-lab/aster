import { createElement as h } from 'react';
import { projects } from './docs_catalog.mjs';
export { projects } from './docs_catalog.mjs';

const repository = 'https://github.com/aster-ai-lab/aster';

export function Callout({ title, children }) {
  return h('aside', { className: 'callout' }, h('strong', null, title), h('div', null, children));
}

export function ChapterCards({ docsRoot, project, version, section }) {
  return h('div', { className: 'chapter-cards' }, ...version.pages.filter((page) => page.section === section).slice(1).map((chapter, index) => h('a', { key: chapter.file, href: docsRoot + project.id + '/' + version.id + '/' + chapter.slug },
    h('span', { className: 'step-number' }, String(index + (section ? 1 : 2)).padStart(2, '0')), h('strong', null, chapter.title), h('span', null, chapter.description), h('span', { className: 'chapter-arrow', 'aria-hidden': true }, '↗'))));
}

export function PortalIntro() {
  return h('div', { className: 'portal-intro' }, h('span', { className: 'portal-orbit', 'aria-hidden': true }, '✳'),
    h('div', null, h('span', { className: 'mini-label' }, 'ASTER DOCS'), h('strong', null, '架构说明与开发指南'), h('p', null, '按子项目查阅工作流程和接入示例。')));
}

export function ProjectCards({ docsRoot, entries = projects }) {
  return h('div', { className: 'project-cards' }, ...entries.map((project) => {
    const version = project.versions.find((entry) => entry.id === project.defaultVersion);
    return h('a', { key: project.id, href: `${docsRoot}${project.id}/${version.id}/` },
      h('div', { className: 'project-card-top' }, h('span', { className: 'mini-label' }, project.label), h('span', { className: 'result-pill' }, version.label)),
      h('strong', null, project.title), h('p', null, project.description),
      h('div', { className: 'project-card-bottom' }, h('span', null, `${project.versions.length} 个版本 · ${version.pages.length} 篇文档`), h('span', null, '阅读产品文档 ↗')));
  }));
}

export function ReadingPaths() {
  return h('div', { className: 'reading-paths' }, ...[
    ['了解项目', '架构概览', '项目负责什么，主要组件如何协作。'],
    ['查看过程', '执行流程', '一次任务从接收到结束会经过哪些步骤。'],
    ['开始开发', '宿主接入', '如何调用接口、配置工具并检查运行结果。'],
  ].map(([label, title, text]) => h('div', { key: label }, h('span', { className: 'mini-label' }, label), h('strong', null, title), h('p', null, text))));
}

export function table(label, headers, rows) {
  return h('div', { className: 'table-scroll', role: 'region', 'aria-label': label, tabIndex: 0 }, h('table', null,
    h('caption', null, label), h('thead', null, h('tr', null, ...headers.map((cell) => h('th', { scope: 'col', key: cell }, cell)))),
    h('tbody', null, ...rows.map(([first, ...cells]) => h('tr', { key: first }, h('th', { scope: 'row' }, first), ...cells.map((cell, i) => h('td', { key: i }, cell)))))));
}

export function DocTable({ caption, headers, rows }) {
  return table(caption, headers, rows);
}

export function MarkdownTable({ children, ...props }) {
  return h('div', { className: 'table-scroll', role: 'region', 'aria-label': props['aria-label'] || '文档表格', tabIndex: 0 }, h('table', props, children));
}

export const sourceRepository = repository;
