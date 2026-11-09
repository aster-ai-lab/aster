import { createHash } from 'node:crypto';

export function extractStyles(node, styles, prefix) {
  if (node.properties?.style) {
    const style = node.properties.style;
    const className = `${prefix}-${createHash('sha256').update(style).digest('hex').slice(0, 12)}`;
    styles.set(className, style);
    node.properties.className = [...(node.properties.className ?? []), className];
    delete node.properties.style;
  }
  for (const child of node.children ?? []) extractStyles(child, styles, prefix);
}
