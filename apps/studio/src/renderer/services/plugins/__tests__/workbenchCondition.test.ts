import { describe, expect, it } from 'vitest'

import { studioContributionConditionSchema } from '@shared/types/studioContribution'

import { matchesWorkbenchCondition, type WorkbenchContributionContext } from '../workbenchCondition'

describe('host-owned contribution visibility', () => {
  // Catch missing execution facts or appearance snapshots accidentally enabling scoped UI.
  it('requires all host facts and withdraws on execution or appearance changes', () => {
    const condition = studioContributionConditionSchema.parse({
      all: ['workspace', 'conversation'],
      none: ['run', 'turn'],
      theme: 'dark',
      locale: 'en-US'
    })
    const context: WorkbenchContributionContext = {
      workspace: true,
      document: false,
      conversation: true,
      run: false,
      turn: false,
      harness: false,
      theme: 'dark',
      locale: 'en-us'
    }
    expect(matchesWorkbenchCondition(condition, context)).toBe(true)
    for (const changed of [
      { workspace: false },
      { conversation: false },
      { run: true },
      { turn: true },
      { theme: null },
      { locale: null },
      { locale: 'zh-CN' }
    ])
      expect(matchesWorkbenchCondition(condition, { ...context, ...changed })).toBe(false)
    expect(matchesWorkbenchCondition(undefined, { ...context, theme: null, locale: null })).toBe(true)
  })
})
