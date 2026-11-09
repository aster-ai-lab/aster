import { readFileSync } from 'node:fs';
import { createElement as h } from 'react';
import { table } from '../../../../scripts/docs_components.mjs';

export function ExtensionIndex() {
  const records = JSON.parse(readFileSync(new URL('./_coverage.json', import.meta.url), 'utf8'));
  return table('客户端可调用的宿主扩展', ['method 或已支持的前缀族', '用法'], records.map((record) => [h('code', null, record.method), h('a', { href: record.page }, '参数与处理步骤 ↗')]));
}
