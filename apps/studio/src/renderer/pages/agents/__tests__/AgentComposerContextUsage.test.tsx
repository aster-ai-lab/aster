import { render, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { cacheService } from '@data/CacheService'
import { AgentComposerContextUsage } from '@renderer/pages/agents/AgentComposer'
import { AGENT_SESSION_CONTEXT_USAGE_CACHE_KEY } from '@shared/ai/agentSessionContextUsage'
import type { Model } from '@shared/data/types/model'

vi.unmock('@data/CacheService')
vi.unmock('@data/hooks/useCache')

const SESSION_ID = 'session-composer-usage'
const KEY = AGENT_SESSION_CONTEXT_USAGE_CACHE_KEY(SESSION_ID)

const model = (id: string, contextWindow: number): Model => ({ id, contextWindow }) as Model

beforeEach(() => {
  Object.defineProperty(window, 'api', {
    configurable: true,
    value: { cache: { broadcastSync: vi.fn(), onSync: vi.fn(), getAllShared: vi.fn(async () => ({})) } }
  })
  cacheService.deleteShared(KEY)
})

describe('AgentComposerContextUsage', () => {
  it('stays hidden until a reading for this model is cached', () => {
    const { rerender } = render(<AgentComposerContextUsage sessionId={SESSION_ID} model={model('openai::small-model', 500_000)} />)
    expect(screen.queryByRole('meter')).not.toBeInTheDocument()

    cacheService.setShared(KEY, {
      categories: [],
      totalTokens: 200_000,
      maxTokens: 250_000,
      percentage: 80,
      model: 'small-model'
    })
    rerender(<AgentComposerContextUsage sessionId={SESSION_ID} model={model('openai::small-model', 500_000)} />)
    expect(screen.getByRole('meter')).toHaveAttribute('aria-valuenow', '40')

    rerender(<AgentComposerContextUsage sessionId={SESSION_ID} model={model('openai::big-model', 1_000_000)} />)
    expect(screen.queryByRole('meter')).not.toBeInTheDocument()
  })
})
