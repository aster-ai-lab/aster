import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function SearchArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'search-architecture',
    label: 'Search 的多源检索与结构边界',
    description: '同一查询经过原生 API、HTTP/RSS 或浏览器来源，统一执行 URL 规范化、去重和排名融合。语义判断保留在调用方。',
  });
}
