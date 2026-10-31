import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function SandboxArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'sandbox-architecture',
    label: 'aster-sandbox 与 aster-box 的职责边界',
    description: 'aster-sandbox 将命令交给原生操作系统边界。aster-box 通过统一生命周期管理选择 MicroVM 或共享内核 Sandbox。',
  });
}
