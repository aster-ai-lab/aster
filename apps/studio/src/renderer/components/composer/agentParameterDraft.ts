import {
  mergeParameterPrefill,
  parameterValuesReady,
  projectAgentParameterSchema,
  type AgentParameterField,
  type ParameterValues
} from '@a3s-lab/intelligent-ui/openui'

import { agentIdFromMentionToken } from '@renderer/pages/agents/agentWiki'

export type AgentParameterDraft = {
  agentId: string
  schema: string
  fields: AgentParameterField[]
  values: ParameterValues
  edited: string[]
  generation: number
  ready: boolean
}

let draft: AgentParameterDraft | null = null
let generation = 0
const listeners = new Set<() => void>()

export function subscribeAgentParameterDraft(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function agentParameterDraftSnapshot() {
  return draft
}

/** A new mention always clears values, including a switch back to the same agent. */
export function setAgentParameterDraft(agentId: string, schema: string | undefined) {
  if (!schema) {
    clearAgentParameterDraft()
    return
  }
  const projected = projectAgentParameterSchema(schema)
  if (!projected.ok) {
    clearAgentParameterDraft()
    return
  }
  generation += 1
  draft = {
    agentId,
    schema,
    fields: projected.fields,
    values: {},
    edited: [],
    generation,
    ready: parameterValuesReady(projected.fields, {})
  }
  emit()
}

export function editAgentParameter(name: string, value: string | number | boolean | null) {
  if (!draft || !draft.fields.some((field) => field.name === name)) return
  const values = { ...draft.values }
  if (value === null || value === '') delete values[name]
  else values[name] = value
  const edited = draft.edited.includes(name) ? draft.edited : [...draft.edited, name]
  draft = { ...draft, values, edited, ready: parameterValuesReady(draft.fields, values) }
  emit()
}

export function acceptParameterPrefill(agentId: string, capturedGeneration: number, extracted: ParameterValues) {
  if (!draft || draft.agentId !== agentId || draft.generation !== capturedGeneration) return
  const values = mergeParameterPrefill(draft.values, extracted, new Set(draft.edited))
  draft = { ...draft, values, ready: parameterValuesReady(draft.fields, values) }
  emit()
}

export function clearAgentParameterDraft() {
  if (!draft) return
  draft = null
  emit()
}

/** The schema travels with the mention. The conversation renders the form after the task exists. */
export function agentParameterTurn(
  agentId: string | undefined,
  tokens: readonly { id: string; payload?: unknown }[]
): { schema: string } | null {
  if (!agentId || !/^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$/.test(agentId)) return null
  const token = tokens.find((item) => agentIdFromMentionToken(item.id) === agentId)
  const schema = schemaFromPayload(token?.payload)
  if (!schema || !projectAgentParameterSchema(schema).ok) return null
  return { schema }
}

function schemaFromPayload(payload: unknown) {
  if (!payload || typeof payload !== 'object') return null
  const schema = (payload as { parametersSchema?: unknown }).parametersSchema
  return typeof schema === 'string' ? schema : null
}

function emit() {
  for (const listener of listeners) listener()
}
