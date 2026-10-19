import { createA3sFlowExpression, type A3sFlowDagNodeCatalog } from '@a3s-lab/flow-ui'

import type { FlowWebsiteLocale } from '../../../../../../web/src/flow-designer/flow-node-catalog'
import type { WorkflowExampleDefinition } from '../../../../../../web/src/flow-designer/WorkflowPlayground.examples'
import {
  createPlaygroundEdge,
  createPlaygroundNode
} from '../../../../../../web/src/flow-designer/WorkflowPlayground.model'
import { harnessExecutionDocument, type HarnessExecutionStep } from './harnessExecutionFlow'

const COLUMN_PITCH = 300
const ROW_PITCH = 156
const ORIGIN_X = 16
const ORIGIN_Y = 16

/** Five columns. The read-only canvas has no rail, so the origin can sit at the margin. */
const HARNESS_PLACES: Record<string, { column: number; row: number }> = {
  'user-message': { column: 0, row: 0 },
  'compact-choice': { column: 1, row: 0 },
  infer: { column: 2, row: 0 },
  'text-choice': { column: 3, row: 0 },
  'assistant-text': { column: 4, row: 0 },
  system: { column: 0, row: 1 },
  summarize: { column: 1, row: 1 },
  tools: { column: 2, row: 1 },
  'question-choice': { column: 3, row: 1 },
  question: { column: 4, row: 1 },
  compact: { column: 0, row: 2 },
  budget: { column: 1, row: 2 },
  'budget-choice': { column: 2, row: 2 },
  'permission-projection': { column: 3, row: 2 },
  'tool-execution': { column: 4, row: 2 },
  answer: { column: 0, row: 3 },
  'budget-denied': { column: 1, row: 3 },
  'confirm-choice': { column: 2, row: 3 },
  confirm: { column: 3, row: 3 },
  'approved-choice': { column: 4, row: 3 },
  denied: { column: 0, row: 4 },
  'deny-queue': { column: 1, row: 4 },
  'queue-choice': { column: 2, row: 4 },
  'mutating-choice': { column: 3, row: 4 },
  'completion-gate': { column: 4, row: 4 },
  incomplete: { column: 0, row: 5 },
  'turn-done': { column: 4, row: 5 }
}

function sanitizeFlowId(value: string): string {
  return value.replace(/^flow\./u, '').replace(/[^a-z0-9]+/giu, '_')
}

/** Same id the console edge component writes to `data-edge-id`. */
export function harnessEdgeDomId(source: string, sourceHandle: string, target: string): string {
  return `edge_${sanitizeFlowId(source)}_${sanitizeFlowId(sourceHandle)}_${sanitizeFlowId(target)}_${sanitizeFlowId('in')}`
}

/**
 * `featured` keeps the console zoom of 0.62.
 * Fitting this decision tree into the settings pane would shrink the cards.
 */
export function harnessPlaygroundExample(
  locale: FlowWebsiteLocale,
  catalog: A3sFlowDagNodeCatalog,
  label: (key: string) => string
): WorkflowExampleDefinition {
  const document = harnessExecutionDocument()
  const steps = document.steps
  const nodes = steps.map((step) => {
    const place = HARNESS_PLACES[step.id] ?? { column: 0, row: 0 }
    return createPlaygroundNode(
      step.id,
      step.designerType,
      { x: ORIGIN_X + place.column * COLUMN_PITCH, y: ORIGIN_Y + place.row * ROW_PITCH },
      locale,
      {
        configuration: configurationFor(step, label),
        registry: catalog.registry
      }
    )
  })
  const edges = document.edges.map((edge) => {
    const labelOverride = edge.labelKey ? label(edge.labelKey) : undefined
    return createPlaygroundEdge(
      {
        source: edge.source,
        sourceHandle: edge.sourceHandle,
        target: edge.target,
        targetHandle: 'in'
      },
      nodes,
      locale,
      catalog.registry,
      { labelOverride }
    )
  })

  return {
    id: 'meta-harness',
    category: 'agent',
    level: 'starter',
    featured: true,
    title: 'Meta Harness',
    description: '',
    outcome: '',
    capabilities: [],
    graph: { nodes, edges, annotations: [] }
  }
}

function configurationFor(step: HarnessExecutionStep, label: (key: string) => string) {
  const configuration: Record<string, unknown> = { title: step.label, desc: '' }
  if (step.matchedLabelKey) configuration.matched_label = label(step.matchedLabelKey)
  if (step.otherwiseLabelKey) configuration.otherwise_label = label(step.otherwiseLabelKey)
  if (step.ruleKey) {
    configuration.expression = createA3sFlowExpression({ op: 'literal', value: label(step.ruleKey) })
  }
  return configuration
}
