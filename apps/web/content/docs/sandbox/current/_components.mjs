import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function SandboxArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'sandbox-architecture',
    label: 'a3s-sandbox 与 a3s-box 的职责边界',
    description: 'a3s-sandbox 将命令交给原生操作系统边界。a3s-box 通过统一生命周期管理选择 MicroVM 或共享内核 Sandbox。',
  });
}
