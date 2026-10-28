import { createElement as h } from 'react';
import { table } from '../../../../scripts/docs_components.mjs';
import { PeleDiagram } from '../../../../scripts/docs_diagrams.mjs';

const definitions = [
  ['architecture', 'FIG. 01 · 运行时架构', '终端与桌面通过 ACP 接入 Core，Rust 应用直接使用 SDK。会话调用模型和受控工具，并把结果写入事实日志。'],
  ['workflow', 'FIG. 02 · 执行循环', '模型根据工具结果决定下一步。工具调用先经过权限检查；返回结果前，还需要检查完成条件并等待资源清理。'],
  ['meta-harness', 'FIG. 05 · Meta Harness 的接入方式', '配方与宿主注册表一起装配组件；完整 Assembler 提供组件树时优先。选中的树共用原有会话、事实日志和 Core 检查。'],
  ['tutorial-architecture', 'FIG. 06 · 终端智能体的模块分工', '界面消费键盘与运行事件；宿主持有会话、确认管理器和存储，Core 负责受控执行与完成检查。'],
  ['tutorial-permissions', 'FIG. 07 · 权限与确认的分工', '权限策略先决定拒绝、允许或询问。确认面板回答的是准确的一次工具调用，超时不会产生批准。'],
  ['rsi-cycle', 'FIG. 08 · 递归改进的实验闭环', '宿主固定评估依据，检查并运行候选，根据完整证据决定采纳或回退。Meta Harness 提供可替换的执行过程。'],
];

function codeDiagram(name) {
  const entry = definitions.find(([id]) => id === name);
  if (!entry) throw new Error(`Unknown Code diagram: ${name}`);
  return h(PeleDiagram, { name, label: entry[1], description: entry[2] });
}

export function ArchitectureDiagram() {
  return codeDiagram('architecture');
}

export function WorkflowDiagram() {
  return codeDiagram('workflow');
}

export function VerificationDiagram() {
  return h('figure', { className: 'verification-figure' },
    h('div', { className: 'verification-row' }, h('span', { className: 'version' }, '版本 A'), h('div', null, h('strong', null, '修改摘要 A'), h('span', null, '测试通过，报告对应 A')), h('span', { className: 'result-pill' }, 'Verified')),
    h('div', { className: 'version-change' }, '↓ 再次修改，产生新的 effect_digest'),
    h('div', { className: 'verification-row' }, h('span', { className: 'version' }, '版本 B'), h('div', null, h('strong', null, '修改摘要 B'), h('span', null, '需要重新运行相应检查')), h('span', { className: 'result-pill result-open' }, '待验证')),
    h('figcaption', null, 'FIG. 03 · A 的测试结果不能证明 B 也通过了测试。'));
}

export function Principles() {
  return h('div', { className: 'principles' }, ...[
    ['01', '执行工具', '检查模型请求的调用是否获准，再执行工具。'],
    ['02', '保存记录', '记录执行结果，供下一轮使用或中断后恢复。'],
    ['03', '检查完成条件', '检查修改是否通过验证，或已获准免除验证。'],
  ].map(([n, title, text]) => h('div', { key: n }, h('span', { className: 'step-number' }, n), h('h3', null, title), h('p', null, text))));
}

export function ComponentStrip() {
  return h('figure', { className: 'component-figure' }, h('div', { className: 'component-strip' }, ...[
    ['system', '系统指令'], ['tools', '工具目录'], ['budget', '执行预算'], ['compact', '上下文压缩'], ['infer', '调度组件'],
  ].map(([name, label]) => h('div', { key: name }, h('code', null, name), h('span', null, label)))), h('figcaption', null, '默认组件。宿主可以调整组合，权限和完成检查仍由 Core 执行。'));
}

export function CapabilityCards() {
  return h('div', { className: 'capability-cards' }, ...[
    ['构建', '模块已编译', '相应的 Cargo feature 已启用。'], ['接入', '会话已配置', '宿主已提供这项能力需要的配置。'], ['验证', '运行已检查', '在目标环境中检查过实际行为。'],
  ].map(([tag, title, text]) => h('div', { key: tag }, h('span', { className: 'mini-label' }, tag), h('strong', null, title), h('p', null, text))));
}

export function MetaHarnessDiagram() {
  return codeDiagram('meta-harness');
}

export function TutorialArchitectureDiagram() {
  return codeDiagram('tutorial-architecture');
}

export function TutorialPermissionDiagram() {
  return codeDiagram('tutorial-permissions');
}

export function RsiCycleDiagram() {
  return codeDiagram('rsi-cycle');
}

export function CompletionStates() {
  return table('完成状态的含义', ['状态', '何时出现', '含义'], [
    ['Narrative', '没有工作区修改，且无其他未解决事项', '本轮可以用最终答复结束'],
    ['Verified', '检查通过，报告对应当前修改', '声明的检查已通过'],
    ['Waived', '宿主或用户明确接受尚未验证的本次修改', '允许结束，但不代表验证通过'],
    ['Incomplete', '缺少检查结果、操作结果不明或子任务尚未处理完', '任务还不能算完成'],
  ]);
}

export function StackDiagram() {
  return h('figure', { className: 'stack-figure' }, ...[
    ['应用', 'Aster · IDE · 业务应用', '提供界面、用户任务与业务信息'],
    ['Agent Harness', 'A3S Code · Codex · Claude Agent SDK · Deep Agents', '模型循环、工具、上下文、运行规则'],
    ['编排运行时', '例如 LangGraph', '保存状态、安排步骤，处理暂停与恢复'],
  ].map(([label, title, text]) => h('div', { key: label }, h('span', { className: 'mini-label' }, label), h('strong', null, title), h('span', null, text))),
    h('figcaption', null, 'FIG. 04 · 各 harness 采用各自的编排实现，其中 Deep Agents 使用 LangGraph。'));
}

export function FrameworkTable() {
  return table('Harness 与编排运行时对比', ['框架', '提供什么', '如何接入或扩展'], [
    ['A3S Code', '可嵌入的 Rust 编码运行时', '直接创建 Rust 会话；组合执行组件，接入不同模型供应商'],
    ['Codex', '完整 agent harness', 'CLI、SDK、app-server；管理会话、工具、沙箱、审批与事件'],
    ['Claude Agent SDK', '在应用中使用 Claude Code 的执行能力', '用 Python、TypeScript 调用；配置工具、权限和生命周期钩子（hooks）'],
    ['Deep Agents', '面向多步任务的通用 harness', '基于 LangGraph 提供文件系统、子代理和记忆；可替换后端和中间件'],
    ['LangGraph', '有状态的流程编排', '用状态图连接固定步骤、模型调用和人工审批；上层提供具体工具与验收规则'],
  ]);
}

export function OwnershipTable() {
  return table('宿主与运行时的职责', ['职责', '主要负责人'], [
    ['界面、用户交互和业务信息', '宿主'], ['模型、工具权限、验证命令和存储配置', '宿主'],
    ['执行模型与工具的循环，检查权限', 'Core'], ['记录修改，检查完成条件', 'Core'],
    ['记忆可以被谁访问，何时激活和刷新', '宿主配置并接入'],
  ]);
}

export function ProfileTable() {
  return table('Core 构建选项', ['Profile', '主要能力'], [
    ['minimal', '执行循环、工作区工具、身份信息与事件记录'], ['local-code · 默认', '增加本地持久词法索引、grep 候选筛选与 PDF 文本提取'],
    ['scientific', '增加持久 SQLite 记忆、浏览器搜索与 advanced-harness'], ['server', '增加 S3 工作区与 Code OTLP 遥测导出'], ['full', 'scientific 与 server 的并集'],
  ]);
}
