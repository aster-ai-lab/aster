import { describe, expect, it } from 'vitest'

import {
  CreateAgentSessionSchema,
  EnsureToBotAgentSchema,
  OpenToBotTurnSchema,
  ListAgentSessionsQuerySchema,
  SetAgentSessionWorkspaceSchema,
  UpdateAgentSessionSchema
} from '../agentSessions'

describe('AgentSession schemas', () => {
  it('accepts non-empty exact ids and enforces the list limit', () => {
    const ids = Array.from({ length: 200 }, (_, index) => `session-${index}`)

    expect(ListAgentSessionsQuerySchema.parse({ ids }).ids).toEqual(ids)
    expect(ListAgentSessionsQuerySchema.safeParse({ ids: [] }).success).toBe(false)
    expect(ListAgentSessionsQuerySchema.safeParse({ ids: [...ids, 'overflow'] }).success).toBe(false)
  })

  it('accepts workspace changes through the dedicated workspace source body', () => {
    expect(SetAgentSessionWorkspaceSchema.safeParse({ type: 'user', workspaceId: 'workspace-1' }).success).toBe(true)
    expect(SetAgentSessionWorkspaceSchema.safeParse({ type: 'system' }).success).toBe(true)
    expect(SetAgentSessionWorkspaceSchema.safeParse({ type: 'user' }).success).toBe(false)
  })

  it('rejects workspace fields on the generic session PATCH body', () => {
    expect(
      UpdateAgentSessionSchema.safeParse({
        workspace: { type: 'user', workspaceId: 'workspace-1' }
      }).success
    ).toBe(false)
    expect(
      UpdateAgentSessionSchema.safeParse({
        workspaceId: 'workspace-1'
      }).success
    ).toBe(false)
  })

  it('accepts manual-name marker updates', () => {
    expect(
      UpdateAgentSessionSchema.parse({
        name: 'Renamed session',
        isNameManuallyEdited: true
      })
    ).toEqual({
      name: 'Renamed session',
      isNameManuallyEdited: true
    })
  })

  it('accepts an empty ToBot warm body and a gateway model', () => {
    expect(EnsureToBotAgentSchema.parse({})).toEqual({})
    expect(
      EnsureToBotAgentSchema.parse({ harnessComponents: ['system', 'tools', 'budget', 'infer'] }).harnessComponents
    ).toEqual(['system', 'tools', 'budget', 'infer'])
    expect(EnsureToBotAgentSchema.safeParse({ harnessComponents: [] }).success).toBe(false)
    expect(EnsureToBotAgentSchema.safeParse({ harnessComponents: [''] }).success).toBe(false)
    expect(EnsureToBotAgentSchema.parse({ model: 'a3s::deepseek-v4-flash' })).toEqual({
      model: 'a3s::deepseek-v4-flash'
    })
    expect(EnsureToBotAgentSchema.safeParse({ model: '' }).success).toBe(false)
    expect(EnsureToBotAgentSchema.safeParse({ extra: true }).success).toBe(false)
    expect(EnsureToBotAgentSchema.parse({ hostContext: { personaName: '小团', persona: '温和' } })).toEqual({
      hostContext: { personaName: '小团', persona: '温和' }
    })
    expect(EnsureToBotAgentSchema.safeParse({ hostContext: { personaName: '', persona: '' } }).success).toBe(false)
    expect(
      EnsureToBotAgentSchema.safeParse({ hostContext: { personaName: '小团', persona: '温和', wiki: '/tmp/wiki' } })
        .success
    ).toBe(false)
  })

  it('accepts one text part as a ToBot turn', () => {
    expect(
      OpenToBotTurnSchema.parse({
        userMessageParts: [{ type: 'text', text: 'hello bot' }]
      })
    ).toEqual({ userMessageParts: [{ type: 'text', text: 'hello bot' }] })
    expect(
      OpenToBotTurnSchema.parse({
        userMessageParts: [{ type: 'text', text: 'hello bot' }],
        harnessComponents: ['system', 'tools', 'budget', 'compact', 'infer']
      }).harnessComponents
    ).toEqual(['system', 'tools', 'budget', 'compact', 'infer'])
    expect(
      OpenToBotTurnSchema.safeParse({
        userMessageParts: [{ type: 'text', text: 'hello bot' }],
        harnessComponents: []
      }).success
    ).toBe(false)
    expect(OpenToBotTurnSchema.safeParse({ userMessageParts: [] }).success).toBe(false)
    expect(OpenToBotTurnSchema.safeParse({ userMessageParts: [{ type: 'text', text: '' }] }).success).toBe(false)
    expect(OpenToBotTurnSchema.safeParse({ userMessageParts: [{ type: 'image', text: 'x' }] }).success).toBe(false)
    expect(
      OpenToBotTurnSchema.parse({
        userMessageParts: [
          {
            type: 'file',
            url: 'file:///tmp/a%20b.png',
            filename: 'a b.png',
            mediaType: 'image/png',
            sizeBytes: 4
          }
        ]
      })
    ).toEqual({
      userMessageParts: [
        {
          type: 'file',
          url: 'file:///tmp/a%20b.png',
          filename: 'a b.png',
          mediaType: 'image/png',
          sizeBytes: 4
        }
      ]
    })
    expect(
      OpenToBotTurnSchema.safeParse({
        userMessageParts: [
          { type: 'file', url: 'https://example.com/a.png', filename: 'a.png', mediaType: 'image/png' }
        ]
      }).success
    ).toBe(false)
    expect(
      OpenToBotTurnSchema.parse({
        userMessageParts: [{ type: 'text', text: 'hello bot' }],
        hostContext: { personaName: '小团', persona: '只用短句' }
      }).hostContext
    ).toEqual({ personaName: '小团', persona: '只用短句' })
    expect(
      OpenToBotTurnSchema.safeParse({
        userMessageParts: [{ type: 'text', text: 'hello bot' }],
        hostContext: { personaName: '小团', persona: '只用短句', wiki: '/tmp/wiki' }
      }).success
    ).toBe(false)
    expect(
      OpenToBotTurnSchema.parse({
        userMessageParts: [{ type: 'text', text: '读取 Notion' }],
        turnContext: { directives: ['Notion read: 会议'] }
      }).turnContext
    ).toEqual({ directives: ['Notion read: 会议'] })
    expect(
      OpenToBotTurnSchema.safeParse({
        userMessageParts: [{ type: 'text', text: 'hello bot' }],
        turnContext: { directives: ['x'.repeat(501)] }
      }).success
    ).toBe(false)
  })

  it('allows blank names for untitled placeholder sessions', () => {
    expect(
      CreateAgentSessionSchema.safeParse({
        agentId: 'agent-1',
        name: '',
        workspace: { type: 'system' }
      }).success
    ).toBe(true)
    expect(UpdateAgentSessionSchema.parse({ name: '' })).toEqual({ name: '' })
  })

  it('caps session names at 255 characters, matching topic.name semantics', () => {
    const maxName = 'a'.repeat(255)
    const overflowName = 'a'.repeat(256)

    expect(
      CreateAgentSessionSchema.safeParse({
        agentId: 'agent-1',
        name: maxName,
        workspace: { type: 'system' }
      }).success
    ).toBe(true)
    expect(
      CreateAgentSessionSchema.safeParse({
        agentId: 'agent-1',
        name: overflowName,
        workspace: { type: 'system' }
      }).success
    ).toBe(false)
    expect(UpdateAgentSessionSchema.safeParse({ name: overflowName }).success).toBe(false)
  })
})
