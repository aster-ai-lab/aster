import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function PowerArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'power-architecture',
    label: 'Power 的模型中立执行边界',
    description: '模型 crate 通过嵌入式接口使用 Power，OpenAI 客户端或 Gateway 通过服务接口使用 Power。共享核心管理制品、设备、准入、状态和执行回执。',
  });
}
