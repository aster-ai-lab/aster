import { fromHtml } from 'hast-util-from-html';

function normalizeText(value) {
  return String(value).replace(/\s+/g, ' ').trim();
}

function proseText(html) {
  const root = fromHtml(html);
  let prose;
  const find = (node) => {
    if (prose) return;
    if (node.type === 'element') {
      const className = node.properties?.className ?? node.properties?.class;
      const classes = Array.isArray(className) ? className : String(className ?? '').split(/\s+/);
      if (classes.includes('prose')) {
        prose = node;
        return;
      }
    }
    for (const child of node.children ?? []) find(child);
  };
  find(root);
  if (!prose) throw new Error('Generated document is missing its prose root');
  const text = (node) => {
    if (node.type === 'text') return node.value;
    if (node.type !== 'element' || ['script', 'style', 'svg'].includes(node.tagName)) return '';
    return (node.children ?? []).map(text).join(' ');
  };
  return normalizeText(text(prose));
}

export function createSearchEntry(chapter, meta, html) {
  return {
    route: chapter.canonicalRoute,
    title: meta.title,
    description: meta.description,
    project: chapter.project?.id ?? '',
    projectTitle: chapter.project?.title ?? 'Aster 文档',
    version: chapter.version?.id ?? '',
    versionLabel: chapter.version?.label ?? '',
    section: chapter.section ?? '',
    text: proseText(html),
  };
}

export function serializeSearchIndex(documents) {
  const ordered = [...documents].sort((left, right) => left.route.localeCompare(right.route));
  return `${JSON.stringify({ format: 1, documents: ordered })}\n`;
}
