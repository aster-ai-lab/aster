import { describe, expect, it } from 'vitest'

import {
  attentionStep,
  notificationBody,
  tobotDeltaAction,
  tobotSettleAction,
  type AttentionRow
} from '../messageAttention'

const row = (id: string, unread: number, extra: Partial<AttentionRow> = {}): AttentionRow => ({
  id,
  kind: 'direct',
  unread,
  muted: false,
  title: id,
  body: 'hello',
  ...extra
})

describe('messageAttention', () => {
  it('badges the first inbox and does not notify history', () => {
    const actions = attentionStep(
      null,
      [row('a', 4), row('b', 2, { muted: true }), row('c', 1, { kind: 'transfer' })],
      null
    )
    expect(actions).toEqual([{ type: 'badge', count: 5 }])
  })

  it('notifies a new incoming row and skips muted, open, and ToBot rows', () => {
    const previous = [
      row('a', 1),
      row('b', 0, { muted: true }),
      row('c', 0),
      row('t', 0, { kind: 'transfer' }),
      row('boss', 0, { kind: 'boss' })
    ]
    const next = [
      row('a', 2, { title: '周予安', body: '在吗' }),
      row('b', 3, { muted: true }),
      row('c', 1),
      row('t', 1, { kind: 'transfer', body: '我自己的上一句' }),
      row('boss', 1, { kind: 'boss', body: '本周目标' })
    ]
    expect(attentionStep(previous, next, 'c')).toEqual([
      { type: 'badge', count: 5 },
      { type: 'notify', id: 'a', title: '周予安', body: '在吗' }
    ])
  })

  it('withdraws a banner when the row is read or leaves the inbox', () => {
    const previous = [row('a', 2), row('t', 1, { kind: 'transfer' }), row('gone', 3)]
    const next = [row('a', 0), row('t', 0, { kind: 'transfer' })]
    expect(attentionStep(previous, next, null)).toEqual([
      { type: 'badge', count: 0 },
      { type: 'withdraw', id: 'a' },
      { type: 'withdraw', id: 't' },
      { type: 'withdraw', id: 'gone' }
    ])
  })

  it('collapses a banner body and keeps it within 140 characters', () => {
    expect(notificationBody('  你好 \n 世界  ', '新消息')).toBe('你好 世界')
    expect(notificationBody('   ', '新消息')).toBe('新消息')
    const long = '字'.repeat(141)
    expect(notificationBody(long, '新消息')).toBe(`${'字'.repeat(139)}…`)
    expect(notificationBody('字'.repeat(140), '新消息')).toBe('字'.repeat(140))
  })

  it('marks one ToBot unread on the first token and banners once when the reply settles', () => {
    expect(tobotDeltaAction({ noted: false, looking: false, unread: 0, id: 't' })).toEqual({ type: 'note', id: 't' })
    expect(tobotDeltaAction({ noted: true, looking: false, unread: 1, id: 't' })).toEqual({ type: 'ignore' })
    expect(tobotDeltaAction({ noted: false, looking: true, unread: 0, id: 't' })).toEqual({ type: 'ignore' })
    expect(tobotDeltaAction({ noted: false, looking: false, unread: 4, id: 't' })).toEqual({ type: 'ignore' })
    expect(tobotSettleAction({ text: '我是 \n ToBot', looking: false, id: 't', fallback: 'ToBot 有新回复' })).toEqual({
      type: 'publish',
      id: 't',
      body: '我是 ToBot'
    })
    expect(tobotSettleAction({ text: '', looking: false, id: 't', fallback: 'ToBot 有新回复' })).toEqual({
      type: 'publish',
      id: 't',
      body: 'ToBot 有新回复'
    })
    expect(tobotSettleAction({ text: '我是 ToBot', looking: true, id: 't', fallback: 'ToBot 有新回复' })).toEqual({
      type: 'withdraw',
      id: 't'
    })
  })
})
