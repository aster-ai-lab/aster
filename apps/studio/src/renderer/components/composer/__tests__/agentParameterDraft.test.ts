import { afterEach, describe, expect, it } from 'vitest'

import { createAgentMentionToken } from '@renderer/components/composer/agentMentionToken'
import {
  acceptParameterPrefill,
  agentParameterDraftSnapshot,
  agentParameterTurn,
  clearAgentParameterDraft,
  editAgentParameter,
  setAgentParameterDraft
} from '@renderer/components/composer/agentParameterDraft'

const schema = JSON.stringify({
  type: 'object',
  properties: {
    days: { type: 'integer', title: '天数', minimum: 1, maximum: 14 },
    kind: { type: 'string', title: '类型', enum: ['a', 'b'] }
  },
  required: ['days', 'kind']
})

afterEach(() => {
  clearAgentParameterDraft()
})

describe('agent parameter draft', () => {
  it('blocks the turn until required fields are filled and keeps an edit ahead of a late extract', () => {
    setAgentParameterDraft('legal-expert', schema)
    const first = agentParameterDraftSnapshot()
    expect(first?.ready).toBe(false)
    const token = createAgentMentionToken({
      id: 'legal-expert',
      name: '法务专家',
      avatar: '法',
      parametersSchema: schema
    })
    expect(agentParameterTurn('legal-expert', [token])).toEqual({ schema })

    editAgentParameter('days', 9)
    acceptParameterPrefill('legal-expert', first?.generation ?? 0, { days: 3, kind: 'b' })
    const filled = agentParameterDraftSnapshot()
    expect(filled?.values).toEqual({ days: 9, kind: 'b' })
    expect(filled?.ready).toBe(true)
    expect(agentParameterTurn('legal-expert', [token])).toEqual({ schema })

    setAgentParameterDraft('writer', schema)
    acceptParameterPrefill('legal-expert', first?.generation ?? 0, { days: 1, kind: 'a' })
    expect(agentParameterDraftSnapshot()?.agentId).toBe('writer')
    expect(agentParameterDraftSnapshot()?.values).toEqual({})
  })
})
