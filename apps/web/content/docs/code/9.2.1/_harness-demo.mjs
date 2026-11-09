import { createElement as h } from 'react';
import { realCases, attachGroups } from './_rsi-cases.mjs';

const core = 'https://github.com/aster-ai-lab/aster/blob/main/crates/code/core/src/';
const child = (label, detail) => ({ label, detail });
const node = (id, label, kind, point, owner, detail, children, source = 'meta_harness.rs') => ({ id, label, kind, point, owner, detail, children, source: core + source });
const nodes = [
  node('facts', '事实日志', 'evidence', [0, 0, .9], 'Core 提供', '组件读取同一事实日志重建状态。日志驱动下一步与恢复；未决的外部副作用仍需核实，不能靠重放盲目重试。', [child('动作 key', '识别同一运行中的动作，保留因果和重放身份。'), child('Run ID', '将结果、取消和恢复绑定到准确的原运行。'), child('事实重建', 'step / output 可再次执行，不能在其中直接发起外部副作用。'), child('未决副作用', '结果未知的外部操作先核实，不当成成功或自动重复。')], 'harness_loop.rs'),
  node('system', 'system', 'component', [-.7, .55, -.45], '默认组件', '提供系统指令列表。候选可以提出调整这个槽，但权限提示与权限决定仍由 Core 管理。', [child('initial', '建立组件初始状态。'), child('step', '读取一条事实，重建状态。'), child('output', '输出指令视图及当前可执行动作；不直接做网络或文件副作用。')]),
  node('tools', 'tools', 'component', [-.8, -.2, -.35], '默认组件', '向视图提供工具目录。工具实现通过原有会话接口注册；目录不是权限凭证。', [child('工具目录', '列出模型可选择的工具说明。'), child('执行器', '由既有程序、动态工具或 MCP 接口承接实际调用。'), child('权限', '每次执行仍经过 Core 权限检查。')]),
  node('budget', 'budget', 'component', [-.15, .75, -.45], '默认组件 + 调度器限制', '组件显示工具调用预算，实际限制由 HarnessConfig 与调度器执行。删掉 budget 展示槽不等于移除上限。', [child('工具次数', 'tool_budget 控制工具调用次数，不是费用或总运行时间。'), child('状态转换', 'step_limit 与工具调用次数是不同计数。'), child('配置继承', 'None 继承基础配置；显式零额度与不存在上限不同。')]),
  node('compact', 'compact', 'component', [.4, .7, -.4], '默认组件 + 调度器动作', '提供压缩阈值视图，由调度器触发压缩。当前 compact_after_chars 按 UTF-8 字节累计；token 压缩依赖适配器上报。', [child('长度阈值', '名称虽然含 chars，当前实现使用 Rust 字符串字节长度。'), child('token 观测', 'token 压缩需要真实输入 token 数据。'), child('限制仍在', '删除 compact 槽不能据此认定自动压缩已关闭。')]),
  node('infer', 'infer', 'component', [.75, .45, -.3], '默认调度器', '根据事实安排模型调用、工具执行、确认或压缩。装配顺序不是固定调用顺序；运行时根据日志状态和动作 key 选择下一步。', [child('读状态', '依据当前事实选择可执行转换。'), child('动作 key', '保留准确动作身份，支持恢复与去重判断。'), child('调度', '读取 HarnessConfig，不绕开权限和完成门禁。')]),
  node('host', 'host:<id>', 'component', [.7, -.1, -.55], 'Rust 宿主注册', 'HostHarnessRegistry 只挂载宿主已注册的 Rust 组件。更大调整可以由 HostHarnessAssembler 提供完整组件树。', [child('注册表', '未知 ID 或缺少注册表时拒绝装配。'), child('完整组件树', 'Assembler 提供树；仍要核对传入的执行限制。'), child('受控接纳', '模型写出 host:<id> 不会下载或装载任意代码。')]),
  node('permission', '权限判定', 'guard', [-.75, -.3, -.5], 'Core 不可关闭', '工具执行前必须有权限决定。系统提示、Hook、候选配方和宿主组件都不能代替授权。', [child('拒绝', '拒绝必须实际阻止执行。'), child('询问', '绑定准确调用与回答，超时不能变成批准。'), child('固定边界', '候选和 Assembler 不能清除 permission_overlay。')]),
  node('completion', '完成门禁', 'guard', [.5, -.5, -.5], 'Core 不可关闭', '修改后的完成状态需要与当前修改摘要对应的验证证据或宿主明确豁免，并等待资源收尾。', [child('当前摘要', '验证报告与本次 effect_digest 对齐。'), child('明确豁免', '宿主显式记录豁免；不是模型自称通过。'), child('收尾完成', 'Run 资源仍未关闭时，不能据此报告成功。')], 'harness_loop.rs'),
];
attachGroups(nodes, {
  facts:[{label:'执行身份',kind:'evidence',records:[0,1]},{label:'恢复边界',kind:'guard',records:[2,3]}],
  system:[{label:'组件生命周期',kind:'component',records:[0,1,2]}],
  tools:[{label:'目录与执行',kind:'component',records:[0,1]},{label:'权限边界',kind:'guard',records:[2]}],
  budget:[{label:'计数限制',kind:'component',records:[0,1]},{label:'配置继承',kind:'guard',records:[2]}],
  compact:[{label:'压缩观测',kind:'evidence',records:[0,1]},{label:'调度限制',kind:'guard',records:[2]}],
  infer:[{label:'动作调度',kind:'component',records:[0,1,2]}],
  host:[{label:'组件装配',kind:'component',records:[0,1]},{label:'接纳边界',kind:'guard',records:[2]}],
  permission:[{label:'权限决定',kind:'guard',records:[0,1]},{label:'固定门禁',kind:'guard',records:[2]}],
  completion:[{label:'完成证据',kind:'evidence',records:[0,1]},{label:'资源寿命',kind:'guard',records:[2]}],
});
const composition = {
  id:'core', title:'Core · 组件协作', type:'实现机制 · 非实时运行数据',
  scope:'连线表示共享事实与执行协作，不表示固定串行调用。光点是关系方向示意，不是实时请求。', source:core+'meta_harness.rs', nodes,
  links:nodes.filter(n=>n.id!=='facts').map(n=>[n.id,'facts']),
  steps:[{title:'同一事实日志',focus:'facts',active:['facts','system','tools','budget','compact','infer'],body:'五个默认组件读取同一事实日志，输出视图与动作。运行时由调度器选择下一步。',result:'装配顺序与运行时调用顺序分别理解。'},
    ...nodes.filter(n=>n.id!=='facts').map(n=>({title:n.label,focus:n.id,active:[n.id,'facts','permission','completion'],body:n.detail,result:n.owner}))],
};
function historyRecord(item,id) {
  return h('div',{id:`${id}-${item.id}-record`,'data-record-case':item.id,key:item.id},
    h('h3',{className:'hd-history-title'},item.title),h('p',{className:'hd-history-type'},item.type),
    h('details',{className:'hd-provenance'},h('summary',null,'案例与证据边界'),h('p',null,item.scope),item.scenario&&h('dl',{className:'hd-scenario-facts'},h('dt',null,'初始条件'),h('dd',null,item.scenario.baseline),h('dt',null,'判断条件'),h('dd',null,item.scenario.decision),h('dt',null,'结果'),h('dd',null,item.scenario.outcome)),h('a',{href:item.source},'原始依据 ↗')),
    ...item.steps.map((step,i)=>h('section',{className:'hd-timeline-step',id:`${id}-${item.id}-step-${i}`,key:i},
      h('details',{className:'hd-event',open:i===0},h('summary',{'data-timeline-select':i},h('span',{className:'hd-event-marker','aria-hidden':true},String(i+1).padStart(2,'0')),h('span',{className:'hd-event-label'},step.title),h('span',{className:'hd-event-chevron','aria-hidden':true},'⌄')),
        h('div',{className:'hd-event-content'},h('p',null,step.body),h('p',{className:'hd-event-result'},step.result),h('div',{className:'hd-event-actions'},h('a',{href:`#${id}-${item.id}-node-${step.focus}`,'data-event-node':step.focus},'钻取节点 ↗'),h('a',{href:item.nodes.find(n=>n.id===step.focus).source},'来源证据 ↗')))))),
    h('details',{className:'hd-node-records'},h('summary',null,'节点说明与全部来源'),...item.nodes.map(n=>h('section',{id:`${id}-${item.id}-node-${n.id}`,key:n.id},h('h4',null,n.label),h('p',null,`${n.owner}：${n.detail}`),h('ul',null,...n.children.map(c=>h('li',{key:c.label},h('strong',null,c.label+'：'),c.detail))),h('a',{href:n.source},'节点来源 ↗')))));
}
function demo(mode) {
  const cases=mode==='rsi'?realCases:[composition], firstCase=cases[0];
  const {nodes,steps}=firstCase, first=steps[0], id=`harness-demo-${mode}`;
  const prefix=`${id}-${firstCase.id}`;
  return h('figure', {className:'harness-demo','data-harness-demo':'','data-mode':mode,'data-example':JSON.stringify({cases}),id},
    h('figcaption',null,h('strong',null,mode==='rsi'?'真实案例 · 执行程序如何演进':'打开 Meta Harness，查看内部如何协作'),h('span',null,'可钻取图谱 · 非实时运行数据')),
    cases.length>1&&h('details',{className:'hd-case-picker',open:true},h('summary',null,h('span',{className:'hd-case-name'},firstCase.title),h('span',{'aria-hidden':true},'⌄')),h('nav',{className:'hd-cases','aria-label':'选择演进案例'},...cases.map((item,i)=>h('a',{href:`#${id}-${item.id}-record`,'data-case':item.id,'aria-current':i===0?'true':undefined,key:item.id},item.title)))),
    h('div',{className:'hd-case-note'},h('strong',{className:'hd-case-type'},firstCase.type),h('p',{className:'hd-case-scope'},firstCase.scope)),
    firstCase.scenario&&h('div',{className:'hd-scenario'},h('span',{className:'hd-scenario-caption'},'当前场景'),h('strong',{className:'hd-scenario-title'},firstCase.scenario.title),h('p',{className:'hd-scenario-goal'},`目标：${firstCase.scenario.goal}`)),
    h('div',{className:'hd-layout'},
      h('nav',{className:'hd-stages','aria-label':mode==='rsi'?'RSI 演进阶段':'Meta Harness 组件'},...steps.map((step,i)=>h('a',{href:`#${prefix}-step-${i}`,'data-step':i,'aria-current':i===0?'step':undefined,key:i},h('span',null,String(i+1).padStart(2,'0')),step.title))),
      h('div',{className:'hd-stage'},
        h('div',{className:'hd-legend'},...[['session','会话'],['component','组件'],['candidate','候选'],['evidence','证据'],['guard','门禁'],['failure','失败']].map(([kind,label])=>h('span',{'data-kind':kind,key:kind},label)),h('span',{className:'hd-flow-label'},'光点 · 关系方向示意')),
        h('div',{className:'hd-world'},h('canvas',{'aria-hidden':true})),h('div',{className:'hd-nodes'},...nodes.map(item=>h('a',{href:`#${prefix}-node-${item.id}`,className:'hd-node','data-node':item.id,'aria-label':item.label,'data-kind':item.kind,'data-role':item.role,key:item.id,'aria-controls':`${id}-lens`,'aria-expanded':'false'},h('i',{'aria-hidden':true}),item.roleLabel&&h('span',{className:'hd-node-role'},item.roleLabel),h('span',null,item.label)))),
        h('canvas',{className:'hd-tether','aria-hidden':true}),
        h('section',{className:'hd-lens',id:`${id}-lens`,hidden:true,'aria-label':'节点内部细节'},
          h('div',{className:'hd-lens-circle'},h('svg',{viewBox:'0 0 400 400',role:'img','aria-labelledby':`${id}-lens-title ${id}-lens-desc`},h('title',{id:`${id}-lens-title`},'当前节点的内部关系'),h('desc',{id:`${id}-lens-desc`},'从当前对象连接到关系类别，再连接具体记录；使用节点按钮阅读或回放讲解。'),h('circle',{cx:200,cy:200,r:188}),h('g',{className:'hd-lens-edges'}),h('g',{className:'hd-lens-marks'})),h('strong',{className:'hd-lens-title'}),h('div',{className:'hd-lens-nodes'}),h('div',{className:'hd-beat-pill',hidden:true,'aria-live':'polite'})),
          h('button',{type:'button',className:'hd-close','aria-label':'返回全局图'},'↙ 返回全局图')),
        h('div',{className:'hd-node-detail','aria-live':'polite'},h('span',{className:'hd-detail-owner'},firstCase.type),h('strong',{className:'hd-detail-title'},first.title),h('p',{className:'hd-detail-body'},first.body),h('a',{className:'hd-detail-source',href:firstCase.source},'查看来源依据 ↗')))),
    h('div',{className:'hd-controls',hidden:true},...['previous','play','next','reset','motion',...(mode==='rsi'?['focus']:[])].map((action,i)=>h('button',{type:'button','data-action':action,key:action,...(action==='motion'?{'aria-pressed':'true'}:{}),...(action==='focus'?{'aria-label':'专注查看图谱',title:'专注查看图谱'}:{})},['← 上一步','开始讲解','下一步 →','复位','暂停动效','⤢'][i])),h('span',{className:'hd-count','aria-live':'polite'},`1 / ${steps.length}`)),
    h('p',{className:'hd-result'},first.result),
    h('details',{className:'hd-transcript',id:`${id}-history`},h('summary',null,h('span',null,'演进历程'),h('span',{className:'hd-history-count'},`${steps.length} 个阶段`),h('span',{className:'hd-history-chevron','aria-hidden':true},'⌃')),...cases.map(item=>historyRecord(item,id))));
}
export function RsiEvolutionDemo() { return demo('rsi'); }
export function MetaHarnessExplorer() { return demo('composition'); }
