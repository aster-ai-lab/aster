import { dataApiService } from '@data/DataApiService'
import { preferredWorkbenchAgentId } from '@renderer/pages/agents/useWorkbenchDefaultAgent'

import { isPersonalVaultDirectory } from './personalOkfView'

/** Stored on the compile session. The work page omits this exact value. */
export const KNOWLEDGE_COMPILE_SESSION = 'knowledge-compile'

export interface KnowledgeCompileSession {
  sessionId: string
  agentId: string
}

/** A compile session, or the personal vault workspace it is bound to, is not a work-page task. */
export function isKnowledgeCompileWorkbenchRow(row: { description?: string | null; path?: string | null }): boolean {
  if (row.description === KNOWLEDGE_COMPILE_SESSION) return true
  const path = row.path?.replace(/\/+$/, '')
  return Boolean(path && isPersonalVaultDirectory(path))
}

interface ListedAgent {
  id: string
  type: string
  createdAt?: string
}

function listedAgents(payload: unknown): ListedAgent[] {
  if (!payload || typeof payload !== 'object' || !('items' in payload)) return []
  const items = (payload as { items?: unknown }).items
  if (!Array.isArray(items)) return []
  return items.flatMap((item) => {
    if (!item || typeof item !== 'object') return []
    const row = item as { id?: unknown; type?: unknown; createdAt?: unknown }
    if (typeof row.id !== 'string' || typeof row.type !== 'string') return []
    return [{ id: row.id, type: row.type, createdAt: typeof row.createdAt === 'string' ? row.createdAt : undefined }]
  })
}

/** Open an a3s code session whose workspace is this vault. The agent does the compile. */
export async function startKnowledgeCompileSession(vaultRoot: string): Promise<KnowledgeCompileSession> {
  const listed = await dataApiService.get('/agents')
  const agentId = preferredWorkbenchAgentId(listedAgents(listed).filter((agent) => agent.type === 'a3s-code'))
  if (!agentId) throw new Error('no a3s code agent')
  const workspace = await dataApiService.post('/agent-workspaces', {
    body: { path: vaultRoot, name: '知识' }
  })
  const session = await dataApiService.post('/agent-sessions', {
    body: {
      agentId,
      name: '知识编译',
      description: KNOWLEDGE_COMPILE_SESSION,
      workspace: { type: 'user', workspaceId: workspace.id }
    }
  })
  return { sessionId: session.id, agentId }
}
