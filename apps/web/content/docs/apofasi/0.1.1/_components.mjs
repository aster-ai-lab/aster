import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function ApofasiArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'apofasi-architecture',
    label: 'Apofasi 的类型化决策路径',
    description: '状态与问题先经过路由，再由词法或神经引擎产生类型化答案。宿主根据置信度决定自动执行或人工复核。',
  });
}
