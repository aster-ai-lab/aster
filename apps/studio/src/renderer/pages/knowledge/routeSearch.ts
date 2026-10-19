export type KnowledgeSection = 'documents' | 'wiki' | 'compile'

export type KnowledgeRouteSearch = {
  baseId?: string
  section?: KnowledgeSection
}

export function parseKnowledgeRouteSearch(search: Record<string, unknown>): KnowledgeRouteSearch {
  const baseId = typeof search.baseId === 'string' && search.baseId.length > 0 ? search.baseId : undefined
  const section = search.section === 'documents' || search.section === 'wiki' || search.section === 'compile' ? search.section : undefined
  return { baseId, section }
}
