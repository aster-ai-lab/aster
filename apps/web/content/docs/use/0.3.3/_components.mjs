import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function UseArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'use-architecture',
    label: 'Use 的 AIX 包与计划应用边界',
    description: 'AIX 包经过结构与信任校验形成可审查计划。应用计划后，Use 按作用域发布 Tool、MCP、Skill、OKF 和 UI 表面。',
  });
}
