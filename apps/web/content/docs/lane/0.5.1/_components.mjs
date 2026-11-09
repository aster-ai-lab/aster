import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function LaneArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'lane-architecture',
    label: 'Lane 的优先级准入与执行边界',
    description: '等待项按 Priority 和 FIFO 进入 Lane。Scheduler 只决定准入，活动工作需要显式取消和结算。持久 Job Backend 另外管理状态与租约。',
  });
}
