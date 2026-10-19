const FOLDER_ID = /^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/

/** Folder name under `wiki/agents`. Rejects path separators and empty ids. */
export function agentWikiFolderId(id: string): string | null {
  const trimmed = id.trim()
  return FOLDER_ID.test(trimmed) ? trimmed : null
}

/** `agent-mention:{id}:{uuid}` stores the agent id before the trailing random id. */
export function agentIdFromMentionToken(tokenId: string): string | null {
  const prefix = 'agent-mention:'
  if (!tokenId.startsWith(prefix)) return null
  const rest = tokenId.slice(prefix.length)
  const split = rest.lastIndexOf(':')
  if (split <= 0) return null
  return agentWikiFolderId(rest.slice(0, split))
}

export function agentWikiDirectory(workspaceRoot: string, agentId: string): string | null {
  const id = agentWikiFolderId(agentId)
  const root = workspaceRoot.trim().replace(/[/\\]+$/u, '')
  if (!id || !root) return null
  return `${root}/wiki/agents/${id}`
}

/** Standing hint so this turn can choose its own encyclopedia. */
export function agentWikiGuide(agentId: string): string {
  const id = agentWikiFolderId(agentId)
  if (!id) return ''
  return `本智能体的专属百科在 wiki/agents/${id}。需要时再选择检索：专属百科用 search_wiki 且 scope=agent、agent=${id}；本地百科用 scope=local；团队百科用 scope=team；联网用 web_search。不需要的来源不要查。`
}
