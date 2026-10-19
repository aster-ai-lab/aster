import { describe, expect, it, vi } from 'vitest'

import { AGENT_WORKSPACE_TYPE } from '@shared/data/api/schemas/agentWorkspaces'

import { chooseCreateWorkspaceSource, isWorkspaceMissingError } from '../sessionWorkspace'

describe('chooseCreateWorkspaceSource', () => {
  it('keeps an explicit workspace when the lookup resolves without throwing', async () => {
    const lookup = vi.fn(async () => undefined)
    const chosen = await chooseCreateWorkspaceSource({
      defaults: { workspaceId: 'workspace-a' },
      pendingProjectId: 'workspace-a',
      lookup
    })

    expect(chosen).toEqual({
      source: { type: AGENT_WORKSPACE_TYPE.USER, workspaceId: 'workspace-a' },
      clearPending: false,
      clearLastUsed: false
    })
    expect(lookup).toHaveBeenCalledWith('/agent-workspaces/workspace-a')
  })

  it('drops a pending project the plane cannot load and uses the system workspace', async () => {
    const lookup = vi.fn(async (path: string) => {
      if (path === '/agent-workspaces/workspace-gone') throw new Error('workspace not found')
      return { id: 'workspace-live' }
    })
    const chosen = await chooseCreateWorkspaceSource({
      defaults: { workspaceId: 'workspace-gone' },
      pendingProjectId: 'workspace-gone',
      lookup
    })

    expect(chosen).toEqual({
      source: { type: AGENT_WORKSPACE_TYPE.SYSTEM },
      clearPending: true,
      clearLastUsed: false
    })
  })

  it('drops a deleted session workspace and keeps a different live project', async () => {
    const lookup = vi.fn(async (path: string) => {
      if (path === '/agent-workspaces/workspace-gone') throw new Error('workspace not found')
      return { id: 'workspace-live' }
    })
    const chosen = await chooseCreateWorkspaceSource({
      defaults: { agentId: 'agent-a' },
      fallbackSession: {
        agentId: 'agent-a',
        workspaceId: 'workspace-gone',
        workspace: { type: 'user' }
      },
      lastUsedWorkspaceId: 'workspace-live',
      lookup
    })

    expect(chosen.source).toEqual({ type: AGENT_WORKSPACE_TYPE.USER, workspaceId: 'workspace-live' })
    expect(chosen.clearLastUsed).toBe(false)
  })

  it('forgets a remembered workspace that is gone', async () => {
    const lookup = vi.fn(async () => {
      throw new Error('workspace not found')
    })
    const chosen = await chooseCreateWorkspaceSource({
      defaults: {},
      lastUsedWorkspaceId: 'workspace-remembered',
      lookup
    })

    expect(chosen).toEqual({
      source: { type: AGENT_WORKSPACE_TYPE.SYSTEM },
      clearPending: false,
      clearLastUsed: true
    })
  })

  it('does not look up a workspace when the caller already asked for the system workspace', async () => {
    const lookup = vi.fn()
    const chosen = await chooseCreateWorkspaceSource({
      defaults: { workspaceMode: 'system', workspaceId: 'workspace-a' },
      lastUsedWorkspaceId: 'workspace-remembered',
      lookup
    })

    expect(chosen.source).toEqual({ type: AGENT_WORKSPACE_TYPE.SYSTEM })
    expect(lookup).not.toHaveBeenCalled()
  })
})

describe('isWorkspaceMissingError', () => {
  it('matches only the plane message for a missing workspace', () => {
    expect(isWorkspaceMissingError(new Error('workspace not found'))).toBe(true)
    expect(isWorkspaceMissingError(new Error('Failed to add a session: workspace not found'))).toBe(false)
    expect(isWorkspaceMissingError(new Error('create failed'))).toBe(false)
  })
})
