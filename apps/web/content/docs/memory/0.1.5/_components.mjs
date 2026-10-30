import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function MemoryArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'memory-architecture',
    label: 'Memory 的证据、存储与派生索引边界',
    description: '宿主负责语义策略。MemoryStore 和 Repository 保存记忆与证据，VectorIndex 只保存调用方提供的派生向量。',
  });
}
