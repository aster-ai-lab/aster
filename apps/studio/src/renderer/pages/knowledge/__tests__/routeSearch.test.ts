import { describe, expect, it } from 'vitest'

import { parseKnowledgeRouteSearch } from '../routeSearch'

describe('parseKnowledgeRouteSearch', () => {
  it('keeps a non-empty knowledge base id', () => {
    expect(parseKnowledgeRouteSearch({ baseId: 'base-1' })).toEqual({ baseId: 'base-1', section: undefined })
  })

  it('drops empty and non-string values', () => {
    expect(parseKnowledgeRouteSearch({ baseId: '' })).toEqual({ baseId: undefined, section: undefined })
    expect(parseKnowledgeRouteSearch({ baseId: 1 })).toEqual({ baseId: undefined, section: undefined })
  })

  it('keeps a document, wiki, or compile section', () => {
    expect(parseKnowledgeRouteSearch({ baseId: 'base-1', section: 'wiki' })).toEqual({ baseId: 'base-1', section: 'wiki' })
    expect(parseKnowledgeRouteSearch({ section: 'recall' })).toEqual({ baseId: undefined, section: undefined })
  })
})
