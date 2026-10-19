import { describe, expect, it } from 'vitest'

import { reconcileContext, reviewFiberActive, type PluginRecord } from '../pluginContext'

const review: PluginRecord = { name: 'review', description: 'Review a document', source: 'installed' }

describe('plugin context', () => {
  it('keeps a plugin inactive until every injected service is present', () => {
    const context = reconcileContext([review], [])

    expect(context.services).toEqual(['workspace'])
    expect(context.fibers[0]).toMatchObject({
      state: 'inactive',
      missing: ['document', 'conversation'],
      effects: []
    })
    expect(reviewFiberActive(context)).toBe(false)
  })

  it('activates the plugin and publishes its service once the coeffects arrive', () => {
    const context = reconcileContext([review], ['document', 'conversation'])

    expect(context.services).toEqual(['conversation', 'document', 'review', 'workspace'])
    expect(context.fibers[0]).toMatchObject({
      state: 'active',
      missing: [],
      effects: ['applet', 'command', 'findings', 'annotation']
    })
    expect(reviewFiberActive(context)).toBe(true)
  })

  it('reverts a dependent when its provider leaves the context', () => {
    const follower: PluginRecord = { name: 'follower', description: '', source: 'user' }
    const contracts = { follower: { inject: ['review'], provide: ['follower'], effects: ['note'] } }
    const session = ['document', 'conversation']
    const withReview = reconcileContext([review, follower], session, contracts)
    const withoutReview = reconcileContext([follower], session, contracts)

    expect(withReview.fibers.find((fiber) => fiber.name === 'follower')?.state).toBe('active')
    expect(withoutReview.fibers[0]).toMatchObject({ state: 'inactive', missing: ['review'], effects: [] })
    expect(withoutReview.services).not.toContain('review')
  })
})
