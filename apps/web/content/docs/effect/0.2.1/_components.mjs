import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function EffectArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'effect-architecture',
    label: 'Effect 的程序值与事实日志循环',
    description: 'Effect 只在 run 边界执行。Actor 折叠不可变事实日志，执行允许的 Transition，再追加事实。恢复使用同一循环。',
  });
}
