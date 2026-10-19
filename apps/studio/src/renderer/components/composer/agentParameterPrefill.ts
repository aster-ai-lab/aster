import {
  acceptedParameterPrefill,
  type AgentParameterField,
  type ParameterValues
} from '@a3s-lab/intelligent-ui/openui'

import { ipcApi } from '@renderer/ipc'

/**
 * One structured call for the current mention. This is not a conversation turn
 * and does not write into the task transcript.
 */
export async function prefillAgentParameters(input: {
  modelId: string
  text: string
  schema: string
  fields: readonly AgentParameterField[]
}): Promise<ParameterValues> {
  const text = input.text.trim().slice(0, 4000)
  if (!text || !input.modelId) return {}
  try {
    const { text: answer } = await ipcApi.request('ai.text.generate', {
      uniqueModelId: input.modelId,
      reasoningEffort: 'none',
      system:
        'Read the user text and return one JSON object whose keys are a subset of the schema properties. Use only string, number, or boolean values. Do not explain.',
      prompt: `Schema:\n${input.schema.slice(0, 8000)}\n\nUser:\n${text}`
    })
    return acceptedParameterPrefill(input.fields, jsonObject(answer))
  } catch {
    return {}
  }
}

export function jsonObject(text: string): unknown {
  const fenced = text.trim().match(/^```(?:json)?\s*([\s\S]*?)\s*```$/i)
  const source = fenced?.[1] ?? text.trim()
  try {
    return JSON.parse(source)
  } catch {
    return null
  }
}
