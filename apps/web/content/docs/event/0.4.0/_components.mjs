import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function EventArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'event-architecture',
    label: 'Event 的 Provider 中立发布订阅边界',
    description: 'EventBus 统一事件、订阅、历史和管理接口。Memory、NATS、Iggy 或自定义 Provider 负责实际传输与持久化。',
  });
}
