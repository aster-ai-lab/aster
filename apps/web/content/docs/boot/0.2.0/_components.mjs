import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function BootArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'boot-architecture',
    label: 'Boot 的协议中立应用边界',
    description: '模块和 Provider 组成应用，Controller 与消息处理器进入协议中立管线，再由 HTTP 或消息适配器连接外部协议。',
  });
}
