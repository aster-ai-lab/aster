import { describe, expect, it } from 'vitest'

import { AgentSessionEntitySchema, ListAgentSessionsQuerySchema } from '../agentSessions'

describe('ordinary task query contract', () => {
  it('preserves literal names and exact owner filters while refusing message statuses and invalid dates', () => {
    const query = ListAgentSessionsQuerySchema.parse({
      search: ' 中文计划%_ ',
      workspaceId: 'project-a',
      status: 'done',
      activityFrom: '2026-10-01T08:00:00+08:00',
      activityBefore: '2026-10-02T00:00:00Z'
    })
    expect(query.search).toBe('中文计划%_')
    expect(query.workspaceId).toBe('project-a')
    for (const input of [{ status: 'success' }, { activityFrom: 'yesterday' }, { search: '中'.repeat(256) }])
      expect(ListAgentSessionsQuerySchema.safeParse(input).success).toBe(false)
  })

  it('accepts native execution facts without requiring them on historical tasks', () => {
    const row = {
      id: 'task-1',
      agentId: 'agent-1',
      name: 'Report',
      isNameManuallyEdited: true,
      workspaceId: 'workspace-1',
      workspace: {
        id: 'workspace-1',
        type: 'user',
        name: 'Project',
        path: '/fixture',
        orderKey: 'a0',
        createdAt: '2026-10-01',
        updatedAt: '2026-10-01'
      },
      orderKey: 'a0',
      lastActivityAt: '2026-10-01T00:00:00Z',
      createdAt: '2026-10-01',
      updatedAt: '2026-10-01'
    }
    expect(AgentSessionEntitySchema.parse(row).execution).toBeUndefined()
    const current = {
      ...row,
      execution: {
        status: 'unknown',
        executionId: 'execution-1',
        anchorMessageId: 'assistant-1',
        observedAt: '2026-10-01T00:00:00Z'
      }
    }
    expect(AgentSessionEntitySchema.parse(current).execution?.status).toBe('unknown')
    expect(
      AgentSessionEntitySchema.safeParse({ ...current, execution: { ...current.execution, status: 'success' } }).success
    ).toBe(false)
  })
})
