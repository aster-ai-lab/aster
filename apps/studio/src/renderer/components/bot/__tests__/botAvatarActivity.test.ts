import { describe, expect, it } from 'vitest'

import {
  botActivityForMessageStatus,
  botActivityForStreamStatus,
  latestTurnBotActivity,
  taskListBotActivity,
  taskListBotAnimated
} from '../botAvatarActivity'

describe('botAvatarActivity', () => {
  it('maps each live stream onto a document-expert pose', () => {
    expect(botActivityForStreamStatus('pending')).toBe('working')
    expect(botActivityForStreamStatus('streaming')).toBe('working')
    expect(botActivityForStreamStatus('awaiting-approval')).toBe('paused')
    expect(botActivityForStreamStatus('awaiting-form')).toBe('paused')
    expect(botActivityForStreamStatus('aborted')).toBe('paused')
    expect(botActivityForStreamStatus('error')).toBe('failed')
    expect(botActivityForStreamStatus('done')).toBe('completed')
    expect(botActivityForStreamStatus(undefined)).toBeUndefined()
  })

  it('maps a saved assistant status when the stream is gone', () => {
    expect(botActivityForMessageStatus('pending')).toBe('working')
    expect(botActivityForMessageStatus('error')).toBe('failed')
    expect(botActivityForMessageStatus('paused')).toBe('paused')
    expect(botActivityForMessageStatus('success')).toBe('completed')
    expect(botActivityForMessageStatus(undefined)).toBe('completed')
  })

  it('lets the live stream override a lagging saved status on the latest turn', () => {
    expect(latestTurnBotActivity('streaming', 'success')).toBe('working')
    expect(latestTurnBotActivity('error', 'pending')).toBe('failed')
    expect(latestTurnBotActivity('awaiting-approval', 'success')).toBe('paused')
    expect(latestTurnBotActivity('done', 'pending')).toBe('completed')
    expect(latestTurnBotActivity(undefined, 'paused')).toBe('paused')
    expect(latestTurnBotActivity(undefined, 'error')).toBe('failed')
  })

  it('rests a task-list row on the completed face when that session has no stream', () => {
    expect(taskListBotActivity('streaming')).toBe('working')
    expect(taskListBotActivity('aborted')).toBe('paused')
    expect(taskListBotActivity(undefined)).toBe('completed')
  })

  it('animates a running task and the open task, and holds other rows still', () => {
    expect(taskListBotAnimated('working', false)).toBe(true)
    expect(taskListBotAnimated('failed', true)).toBe(true)
    expect(taskListBotAnimated('paused', false)).toBe(false)
    expect(taskListBotAnimated('completed', false)).toBe(false)
    expect(taskListBotAnimated('failed', false)).toBe(false)
  })
})
