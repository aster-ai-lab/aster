import { describe, expect, it } from 'vitest'

import {
  agentFormBindingSchema,
  agentFormPartSchema,
  agentFormSubmissionSchema,
  agentFormValuesSchema
} from '../agentForm'

const binding = {
  protocolVersion: 1,
  requestId: 'a'.repeat(64),
  revision: `sha256:${'b'.repeat(64)}`,
  owner: {
    sessionId: 'session',
    executionId: 'model',
    attemptId: 1,
    anchorMessageId: 'assistant',
    acpSessionId: 'acp',
    promptId: 'prompt'
  }
}

describe('native form contracts', () => {
  it('admits OpenUI forms and refuses a plugin-named source tag for the same live binding', () => {
    const part = { id: binding.requestId, binding, format: 'openui', spec: 'source', state: 'form-requested' }
    expect(agentFormPartSchema.safeParse(part).success).toBe(true)
    expect(agentFormPartSchema.safeParse({ ...part, format: 'aix-ui' }).success).toBe(false)
  })
  it('admits a closed native card without inventing an answer and keeps its binding strict', () => {
    const part = { id: binding.requestId, binding, format: 'question', spec: 'source', state: 'closed' }
    expect(agentFormPartSchema.safeParse(part).success).toBe(true)
    expect(agentFormPartSchema.safeParse({ ...part, values: { choice: 'question-1' } }).success).toBe(false)
    expect(agentFormPartSchema.safeParse({ ...part, id: 'c'.repeat(64) }).success).toBe(false)
  })

  it('refuses missing, unknown, incompatible and ambiguous ownership', () => {
    expect(agentFormSubmissionSchema.safeParse({ values: {} }).success).toBe(false)
    expect(agentFormBindingSchema.safeParse({ ...binding, protocolVersion: 2 }).success).toBe(false)
    expect(
      agentFormBindingSchema.safeParse({ ...binding, owner: { ...binding.owner, authority: 'all' } }).success
    ).toBe(false)
    expect(agentFormBindingSchema.safeParse({ ...binding, owner: { ...binding.owner, run: null } }).success).toBe(false)
    expect(
      agentFormBindingSchema.safeParse({
        ...binding,
        owner: { ...binding.owner, run: { coreSessionId: 'core', runId: 'run' } }
      }).success
    ).toBe(false)
    expect(
      agentFormBindingSchema.safeParse({
        ...binding,
        owner: { ...binding.owner, run: { coreSessionId: 'core', runId: 'run', modelTurn: null } }
      }).success
    ).toBe(true)
    expect(
      agentFormPartSchema.safeParse({
        id: 'c'.repeat(64),
        binding,
        format: 'openui',
        spec: 'source',
        state: 'form-requested'
      }).success
    ).toBe(false)
  })

  it('accepts typed scalars and refuses recursive, unsafe and oversized answers without UTF-8 drift', () => {
    expect(agentFormValuesSchema.parse({ count: 0, ready: false, topic: 'Work' })).toEqual({
      count: 0,
      ready: false,
      topic: 'Work'
    })
    for (const value of [
      null,
      [],
      {},
      Number.NaN,
      Number.POSITIVE_INFINITY,
      Number.MAX_SAFE_INTEGER + 1,
      '界'.repeat(1366)
    ])
      expect(agentFormValuesSchema.safeParse({ value }).success).toBe(false)
    expect(agentFormValuesSchema.safeParse({ '': true }).success).toBe(false)
    expect(
      agentFormValuesSchema.safeParse(
        Object.fromEntries(Array.from({ length: 65 }, (_, index) => [`field-${index}`, true]))
      ).success
    ).toBe(false)
    expect(
      agentFormValuesSchema.safeParse(
        Object.fromEntries(Array.from({ length: 17 }, (_, index) => [`field-${index}`, 'a'.repeat(4096)]))
      ).success
    ).toBe(false)
    expect(agentFormValuesSchema.safeParse({ ['界'.repeat(43)]: true }).success).toBe(false)
  })
})
