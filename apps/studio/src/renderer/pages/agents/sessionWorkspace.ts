import type { AgentSessionWorkspaceSource } from '@shared/data/api/schemas/agentWorkspaces'
import { AGENT_WORKSPACE_TYPE } from '@shared/data/api/schemas/agentWorkspaces'

import type { CreateAgentSessionDefaults } from './types'

/** The plane rejects session create with this message when a user workspace id is gone. */
export function isWorkspaceMissingError(error: unknown): boolean {
  return error instanceof Error && error.message === 'workspace not found'
}

type WorkspaceLookup = (path: string) => Promise<unknown>

type ChosenWorkspace = {
  source: AgentSessionWorkspaceSource
  clearPending: boolean
  clearLastUsed: boolean
}

type FallbackSession = {
  agentId?: string | null
  workspaceId?: string | null
  workspace?: { type?: string | null } | null
}

const systemWorkspace = (): AgentSessionWorkspaceSource => ({ type: AGENT_WORKSPACE_TYPE.SYSTEM })

async function userWorkspaceWhenPresent(
  workspaceId: string,
  lookup: WorkspaceLookup
): Promise<AgentSessionWorkspaceSource | null> {
  try {
    await lookup(`/agent-workspaces/${workspaceId}`)
    return { type: AGENT_WORKSPACE_TYPE.USER, workspaceId }
  } catch {
    return null
  }
}

function sourceFromSession(session: FallbackSession): AgentSessionWorkspaceSource {
  if (session.workspace?.type === AGENT_WORKSPACE_TYPE.SYSTEM) return systemWorkspace()
  return session.workspaceId ? { type: AGENT_WORKSPACE_TYPE.USER, workspaceId: session.workspaceId } : systemWorkspace()
}

/**
 * A user workspace the plane cannot load is dropped.
 * Sending it makes session create toast `workspace not found`.
 */
export async function chooseCreateWorkspaceSource(input: {
  defaults: CreateAgentSessionDefaults
  fallbackSession?: FallbackSession | null
  lastUsedWorkspaceId?: string | null
  pendingProjectId?: string | null
  lookup: WorkspaceLookup
}): Promise<ChosenWorkspace> {
  const { defaults, fallbackSession, lastUsedWorkspaceId, pendingProjectId, lookup } = input
  if (defaults.workspace) return { source: defaults.workspace, clearPending: false, clearLastUsed: false }
  if (defaults.workspaceMode === 'system') {
    return { source: systemWorkspace(), clearPending: false, clearLastUsed: false }
  }

  const skipped = new Set<string>()
  let clearPending = false
  if (defaults.workspaceId) {
    const present = await userWorkspaceWhenPresent(defaults.workspaceId, lookup)
    if (present) return { source: present, clearPending: false, clearLastUsed: false }
    skipped.add(defaults.workspaceId)
    clearPending = pendingProjectId === defaults.workspaceId
  }

  if (fallbackSession && (!defaults.agentId || defaults.agentId === fallbackSession.agentId)) {
    const source = sourceFromSession(fallbackSession)
    if (source.type !== AGENT_WORKSPACE_TYPE.USER) return { source, clearPending, clearLastUsed: false }
    if (!skipped.has(source.workspaceId)) {
      const present = await userWorkspaceWhenPresent(source.workspaceId, lookup)
      if (present) return { source: present, clearPending, clearLastUsed: false }
      skipped.add(source.workspaceId)
    }
  }

  if (lastUsedWorkspaceId && !skipped.has(lastUsedWorkspaceId)) {
    const present = await userWorkspaceWhenPresent(lastUsedWorkspaceId, lookup)
    if (present) return { source: present, clearPending, clearLastUsed: false }
    return { source: systemWorkspace(), clearPending, clearLastUsed: true }
  }

  return {
    source: systemWorkspace(),
    clearPending,
    clearLastUsed: !!lastUsedWorkspaceId && skipped.has(lastUsedWorkspaceId)
  }
}
