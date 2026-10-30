import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function VecArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'vec-architecture',
    label: 'Vec 的候选检索与精确重排',
    description: '查询固定一个修订。ANN 提供候选，权威文档决定最终分数。缺失或过期索引使用精确扫描回退。',
  });
}
