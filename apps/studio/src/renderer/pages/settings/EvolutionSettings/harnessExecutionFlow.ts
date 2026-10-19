/** Default a3s-code Meta Harness. Parts feed the view. The scheduler and the kernel branch on facts. */

export const HARNESS_STOCK_ORDER = ['system', 'tools', 'budget', 'compact', 'infer'] as const

export type HarnessStockId = (typeof HARNESS_STOCK_ORDER)[number]

export type HarnessFlowKind = 'stock' | 'kernel' | 'scheduler'

export type HarnessDesignerType = 'flow.start' | 'flow.step' | 'flow.condition' | 'flow.complete' | 'flow.fail'

export type HarnessSourceHandle = 'next' | 'success' | 'matched' | 'otherwise'

export interface HarnessExecutionStep {
  id: string
  label: string
  roleKey: string
  designerType: HarnessDesignerType
  kind: HarnessFlowKind
  matchedLabelKey?: string
  otherwiseLabelKey?: string
  /** Shown on the condition card in place of the designer default rule. */
  ruleKey?: string
}

export interface HarnessExecutionEdge {
  id: string
  source: string
  target: string
  sourceHandle: HarnessSourceHandle
  /** Step edges that need a name. Condition branches use the node labels. */
  labelKey?: string
}

export interface HarnessExecutionDocument {
  steps: readonly HarnessExecutionStep[]
  edges: readonly HarnessExecutionEdge[]
}

export function harnessSectionKey(step: HarnessExecutionStep): string {
  if (step.kind === 'stock') return 'settings.evolution.phases.stock'
  if (step.kind === 'kernel') return 'settings.evolution.phases.kernel'
  return 'settings.evolution.phases.scheduler'
}

const edge = (key: string) => `settings.evolution.edges.${key}`
const role = (key: string) => `settings.evolution.roles.${key}`
const rule = (key: string) => `settings.evolution.rules.${key}`

const STOCK_STEPS: readonly HarnessExecutionStep[] = [
  { id: 'system', label: 'system', roleKey: role('system'), designerType: 'flow.step', kind: 'stock' },
  { id: 'tools', label: 'tools', roleKey: role('tools'), designerType: 'flow.step', kind: 'stock' },
  { id: 'budget', label: 'budget', roleKey: role('budget'), designerType: 'flow.step', kind: 'stock' },
  { id: 'compact', label: 'compact', roleKey: role('compact'), designerType: 'flow.step', kind: 'stock' },
  { id: 'infer', label: 'infer', roleKey: role('infer'), designerType: 'flow.step', kind: 'stock' }
]

const DETAIL_STEPS: readonly HarnessExecutionStep[] = [
  {
    id: 'user-message',
    label: 'user message',
    roleKey: role('userMessage'),
    designerType: 'flow.start',
    kind: 'scheduler'
  },
  {
    id: 'compact-choice',
    label: 'compact choice',
    roleKey: role('compactChoice'),
    designerType: 'flow.condition',
    kind: 'scheduler',
    matchedLabelKey: edge('compactNow'),
    otherwiseLabelKey: edge('directInfer'),
    ruleKey: rule('compactChoice')
  },
  {
    id: 'summarize',
    label: 'summarize',
    roleKey: role('summarize'),
    designerType: 'flow.step',
    kind: 'scheduler'
  },
  {
    id: 'text-choice',
    label: 'text choice',
    roleKey: role('textChoice'),
    designerType: 'flow.condition',
    kind: 'scheduler',
    matchedLabelKey: edge('text'),
    otherwiseLabelKey: edge('unfinished'),
    ruleKey: rule('textChoice')
  },
  {
    id: 'assistant-text',
    label: 'assistant text',
    roleKey: role('assistantText'),
    designerType: 'flow.complete',
    kind: 'scheduler'
  },
  {
    id: 'question-choice',
    label: 'question choice',
    roleKey: role('questionChoice'),
    designerType: 'flow.condition',
    kind: 'scheduler',
    matchedLabelKey: edge('question'),
    otherwiseLabelKey: edge('toolCall'),
    ruleKey: rule('questionChoice')
  },
  {
    id: 'question',
    label: 'question',
    roleKey: role('question'),
    designerType: 'flow.step',
    kind: 'scheduler'
  },
  {
    id: 'answer',
    label: 'answer',
    roleKey: role('answer'),
    designerType: 'flow.step',
    kind: 'scheduler'
  },
  {
    id: 'budget-choice',
    label: 'budget choice',
    roleKey: role('budgetChoice'),
    designerType: 'flow.condition',
    kind: 'scheduler',
    matchedLabelKey: edge('budgetLeft'),
    otherwiseLabelKey: edge('budgetOver'),
    ruleKey: rule('budgetChoice')
  },
  {
    id: 'budget-denied',
    label: 'budget denied',
    roleKey: role('budgetDenied'),
    designerType: 'flow.fail',
    kind: 'scheduler'
  },
  {
    id: 'permission-projection',
    label: 'permission projection',
    roleKey: role('permissionProjection'),
    designerType: 'flow.condition',
    kind: 'kernel',
    matchedLabelKey: edge('beforeTool'),
    otherwiseLabelKey: edge('confirm'),
    ruleKey: rule('permissionProjection')
  },
  {
    id: 'confirm-choice',
    label: 'confirm choice',
    roleKey: role('confirmChoice'),
    designerType: 'flow.condition',
    kind: 'scheduler',
    matchedLabelKey: edge('confirm'),
    otherwiseLabelKey: edge('permissionDeny'),
    ruleKey: rule('confirmChoice')
  },
  {
    id: 'confirm',
    label: 'confirm',
    roleKey: role('confirm'),
    designerType: 'flow.step',
    kind: 'scheduler'
  },
  {
    id: 'approved-choice',
    label: 'approved choice',
    roleKey: role('approvedChoice'),
    designerType: 'flow.condition',
    kind: 'scheduler',
    matchedLabelKey: edge('confirmAllow'),
    otherwiseLabelKey: edge('confirmDeny'),
    ruleKey: rule('approvedChoice')
  },
  {
    id: 'tool-execution',
    label: 'tool execution',
    roleKey: role('toolExecution'),
    designerType: 'flow.step',
    kind: 'scheduler'
  },
  {
    id: 'denied',
    label: 'denied',
    roleKey: role('denied'),
    designerType: 'flow.step',
    kind: 'scheduler'
  },
  {
    id: 'deny-queue',
    label: 'deny queue',
    roleKey: role('denyQueue'),
    designerType: 'flow.condition',
    kind: 'scheduler',
    matchedLabelKey: edge('moreCalls'),
    otherwiseLabelKey: edge('denyEnd'),
    ruleKey: rule('denyQueue')
  },
  {
    id: 'queue-choice',
    label: 'queue choice',
    roleKey: role('queueChoice'),
    designerType: 'flow.condition',
    kind: 'scheduler',
    matchedLabelKey: edge('moreCalls'),
    otherwiseLabelKey: edge('queueDone'),
    ruleKey: rule('queueChoice')
  },
  {
    id: 'mutating-choice',
    label: 'mutating choice',
    roleKey: role('mutatingChoice'),
    designerType: 'flow.condition',
    kind: 'scheduler',
    matchedLabelKey: edge('mutatingSuccess'),
    otherwiseLabelKey: edge('noMutation'),
    ruleKey: rule('mutatingChoice')
  },
  {
    id: 'completion-gate',
    label: 'completion gate',
    roleKey: role('completionGate'),
    designerType: 'flow.condition',
    kind: 'kernel',
    matchedLabelKey: edge('gateClosed'),
    otherwiseLabelKey: edge('gateOpen'),
    ruleKey: rule('completionGate')
  },
  {
    id: 'turn-done',
    label: 'turn done',
    roleKey: role('turnDone'),
    designerType: 'flow.complete',
    kind: 'scheduler'
  },
  {
    id: 'incomplete',
    label: 'incomplete',
    roleKey: role('incomplete'),
    designerType: 'flow.step',
    kind: 'kernel'
  }
]

const EDGES: readonly HarnessExecutionEdge[] = [
  { id: 'user-message__compact-choice', source: 'user-message', target: 'compact-choice', sourceHandle: 'next' },
  {
    id: 'compact-choice__summarize',
    source: 'compact-choice',
    target: 'summarize',
    sourceHandle: 'matched'
  },
  { id: 'compact-choice__infer', source: 'compact-choice', target: 'infer', sourceHandle: 'otherwise' },
  { id: 'summarize__infer', source: 'summarize', target: 'infer', sourceHandle: 'success' },
  {
    id: 'infer__text-choice',
    source: 'infer',
    target: 'text-choice',
    sourceHandle: 'success',
    labelKey: edge('modelReturn')
  },
  { id: 'text-choice__assistant-text', source: 'text-choice', target: 'assistant-text', sourceHandle: 'matched' },
  {
    id: 'text-choice__question-choice',
    source: 'text-choice',
    target: 'question-choice',
    sourceHandle: 'otherwise'
  },
  { id: 'question-choice__question', source: 'question-choice', target: 'question', sourceHandle: 'matched' },
  {
    id: 'question-choice__budget-choice',
    source: 'question-choice',
    target: 'budget-choice',
    sourceHandle: 'otherwise'
  },
  { id: 'question__answer', source: 'question', target: 'answer', sourceHandle: 'success' },
  {
    id: 'answer__compact-choice',
    source: 'answer',
    target: 'compact-choice',
    sourceHandle: 'success',
    labelKey: edge('answerLoop')
  },
  { id: 'budget__budget-choice', source: 'budget', target: 'budget-choice', sourceHandle: 'success' },
  {
    id: 'budget-choice__permission-projection',
    source: 'budget-choice',
    target: 'permission-projection',
    sourceHandle: 'matched'
  },
  {
    id: 'budget-choice__budget-denied',
    source: 'budget-choice',
    target: 'budget-denied',
    sourceHandle: 'otherwise'
  },
  {
    id: 'permission-projection__tool-execution',
    source: 'permission-projection',
    target: 'tool-execution',
    sourceHandle: 'matched'
  },
  {
    id: 'permission-projection__confirm-choice',
    source: 'permission-projection',
    target: 'confirm-choice',
    sourceHandle: 'otherwise'
  },
  { id: 'confirm-choice__confirm', source: 'confirm-choice', target: 'confirm', sourceHandle: 'matched' },
  { id: 'confirm__approved-choice', source: 'confirm', target: 'approved-choice', sourceHandle: 'success' },
  {
    id: 'approved-choice__tool-execution',
    source: 'approved-choice',
    target: 'tool-execution',
    sourceHandle: 'matched'
  },
  { id: 'approved-choice__denied', source: 'approved-choice', target: 'denied', sourceHandle: 'otherwise' },
  { id: 'confirm-choice__denied', source: 'confirm-choice', target: 'denied', sourceHandle: 'otherwise' },
  { id: 'denied__deny-queue', source: 'denied', target: 'deny-queue', sourceHandle: 'success' },
  {
    id: 'deny-queue__permission-projection',
    source: 'deny-queue',
    target: 'permission-projection',
    sourceHandle: 'matched'
  },
  { id: 'deny-queue__turn-done', source: 'deny-queue', target: 'turn-done', sourceHandle: 'otherwise' },
  { id: 'tool-execution__queue-choice', source: 'tool-execution', target: 'queue-choice', sourceHandle: 'success' },
  {
    id: 'queue-choice__permission-projection',
    source: 'queue-choice',
    target: 'permission-projection',
    sourceHandle: 'matched'
  },
  {
    id: 'queue-choice__mutating-choice',
    source: 'queue-choice',
    target: 'mutating-choice',
    sourceHandle: 'otherwise'
  },
  {
    id: 'mutating-choice__completion-gate',
    source: 'mutating-choice',
    target: 'completion-gate',
    sourceHandle: 'matched'
  },
  {
    id: 'mutating-choice__compact-choice',
    source: 'mutating-choice',
    target: 'compact-choice',
    sourceHandle: 'otherwise'
  },
  {
    id: 'completion-gate__turn-done',
    source: 'completion-gate',
    target: 'turn-done',
    sourceHandle: 'matched'
  },
  {
    id: 'completion-gate__incomplete',
    source: 'completion-gate',
    target: 'incomplete',
    sourceHandle: 'otherwise'
  },
  {
    id: 'incomplete__infer',
    source: 'incomplete',
    target: 'infer',
    sourceHandle: 'success',
    labelKey: edge('continueInfer')
  }
]

/** Stock view slots, then the scheduler and kernel decision tree. */
export function harnessExecutionDocument(): HarnessExecutionDocument {
  return {
    steps: [...STOCK_STEPS, ...DETAIL_STEPS],
    edges: EDGES
  }
}
