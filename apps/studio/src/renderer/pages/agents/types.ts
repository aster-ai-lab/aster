import type { AgentSessionWorkspaceSource } from '@shared/data/api/schemas/agentWorkspaces'

export type CreateAgentSessionDefaults = {
  agentId?: string | null
  workspace?: AgentSessionWorkspaceSource
  workspaceId?: string
  workspaceMode?: 'system'
  /** Skip this session when reusing an empty untitled one, so a busy task is not continued. */
  excludeSessionId?: string
}
