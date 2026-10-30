import { createElement as h } from 'react';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

export function FlowArchitectureDiagram() {
  return h(PeleDiagram, {
    name: 'flow-architecture',
    label: 'Flow 的历史、决策与副作用边界',
    description: '引擎从追加式历史投影状态，工作流返回确定性命令。宿主执行步骤并提交结果，等待时释放 Worker，恢复时重新读取历史。',
  });
}
