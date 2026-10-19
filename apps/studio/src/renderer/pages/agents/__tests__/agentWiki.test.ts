import { describe, expect, it } from 'vitest'

import { agentIdFromMentionToken, agentWikiDirectory, agentWikiFolderId, agentWikiGuide } from '../agentWiki'

describe('agent encyclopedia folder', () => {
  it('keeps an installed agent id inside wiki/agents', () => {
    expect(agentWikiFolderId('legal-expert')).toBe('legal-expert')
    expect(agentWikiFolderId('../wiki')).toBeNull()
    expect(agentWikiDirectory('/Users/roy/work', 'legal-expert')).toBe('/Users/roy/work/wiki/agents/legal-expert')
    expect(agentIdFromMentionToken('agent-mention:legal-expert:11111111-1111-1111-1111-111111111111')).toBe(
      'legal-expert'
    )
    expect(agentWikiGuide('legal-expert')).toContain('wiki/agents/legal-expert')
    expect(agentWikiGuide('legal-expert')).toContain('scope=team')
    expect(agentWikiGuide('legal-expert')).toContain('web_search')
  })
})
