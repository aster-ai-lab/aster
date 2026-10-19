import { describe, expect, it } from 'vitest'

import {
  canRecall,
  conversationTitle,
  departmentTree,
  previewText,
  readState,
  systemNotice,
  activityBarUnread,
  totalUnread
} from '../imView'
import { encodeBossWire, encodeQuotedText, makeQuote } from '../quote'
import { detail, entry, GROUP, ME, message, PEER, THIRD } from './fakeImApi'

const t = (key: string, options?: Record<string, unknown>) => (options ? `${key}:${JSON.stringify(options)}` : key)
const snapshot = {
  me: { userId: ME, displayName: '林知夏' },
  people: {
    [PEER]: { userId: PEER, displayName: '周予安', email: '', departmentId: '', position: null },
    [THIRD]: { userId: THIRD, displayName: '陈可', email: '', departmentId: '', position: null }
  }
}

describe('imView', () => {
  it('totals unread for the sidebar without muted conversations', () => {
    expect(totalUnread([entry({ unread: 3 }), entry({ unread: 5, muted: true }), entry({ unread: 2 })])).toBe(5)
    expect(totalUnread([])).toBe(0)
  })

  it('counts ActivityBar unread for a ready team and a personal ToBot, and ignores other states', () => {
    const unread = [entry({ unread: 1 })]
    expect(activityBarUnread({ status: 'ready', conversations: unread })).toBe(1)
    expect(activityBarUnread({ status: 'no-org', conversations: unread })).toBe(1)
    expect(activityBarUnread({ status: 'no-org', conversations: [entry({ unread: 3, muted: true })] })).toBe(0)
    expect(activityBarUnread({ status: 'signed-out', conversations: unread })).toBe(0)
    expect(activityBarUnread({ status: 'loading', conversations: unread })).toBe(0)
  })

  it('titles direct chats by the peer and keeps explicit titles', () => {
    expect(conversationTitle(entry(), snapshot, t)).toBe('周予安')
    expect(conversationTitle(entry({ kind: 'group', title: '设计评审', peerUserId: null }), snapshot, t)).toBe(
      '设计评审'
    )
    expect(conversationTitle(entry({ kind: 'product', title: null, peerUserId: null }), snapshot, t)).toBe(
      'messages.file_transfer'
    )
    expect(conversationTitle(entry({ kind: 'boss', title: null, peerUserId: null }), snapshot, t)).toBe(
      'messages.toboss'
    )
    expect(conversationTitle(entry({ remark: '小周' }), snapshot, t)).toBe('小周')
  })

  it('prefixes group previews with the sender and hides recalled content', () => {
    const group = entry({ id: GROUP, kind: 'group', title: 'g', peerUserId: null })
    expect(previewText(group, snapshot, t)).toBe('周予安: hello 2')
    expect(previewText(entry({ preview: { ...entry().preview!, recalled: true, text: null } }), snapshot, t)).toBe(
      'messages.recall:{"name":"周予安"}'
    )
    expect(
      previewText(entry({ preview: { ...entry().preview!, kind: 'file', text: null, name: 'a.pdf' } }), snapshot, t)
    ).toBe('messages.preview.file a.pdf')
    const quoted = encodeQuotedText('收到', makeQuote('m-1', PEER, '周予安', 'text', 'hello 1'))
    expect(previewText(entry({ preview: { ...entry().preview!, text: quoted } }), snapshot, t)).toBe('收到')
    expect(previewText(entry({ preview: { ...entry().preview!, text: quoted.slice(0, 40) } }), snapshot, t)).toBe(
      '收到'
    )
    const reply = encodeBossWire('answer', 'reply')
    expect(previewText({ ...group, preview: { ...group.preview!, text: reply } }, snapshot, t)).toBe(
      'messages.toboss: answer'
    )
    expect(
      previewText({ ...group, preview: { ...group.preview!, text: encodeBossWire('', 'join') } }, snapshot, t)
    ).toBe('messages.toboss.joined')
  })

  it('renders system notices with directory names', () => {
    expect(systemNotice('member_added', { actor_id: ME, added_ids: `${PEER},${THIRD}` }, snapshot, t)).toBe(
      'messages.system.member_added:{"actor":"林知夏","names":"周予安messages.list_separator陈可"}'
    )
    expect(systemNotice('member_left', { actor_id: PEER }, snapshot, t)).toBe(
      'messages.system.member_left:{"actor":"周予安"}'
    )
    expect(systemNotice('group_dissolved', { actor_id: ME }, snapshot, t)).toBe(
      'messages.system.group_dissolved:{"actor":"林知夏"}'
    )
    expect(systemNotice('title_changed', { actor_id: ME, title: '设计评审' }, snapshot, t)).toBe(
      'messages.system.title_changed:{"actor":"林知夏","title":"设计评审"}'
    )
    expect(systemNotice('owner_transferred', { actor_id: ME, user_id: PEER }, snapshot, t)).toBe(
      'messages.system.owner_transferred:{"actor":"林知夏","name":"周予安"}'
    )
    expect(systemNotice('role_changed', { actor_id: ME, user_id: PEER, role: 'admin' }, snapshot, t)).toBe(
      'messages.system.role_changed:{"actor":"林知夏","name":"周予安","role":"messages.role.admin"}'
    )
    expect(systemNotice('mute_all_on', { actor_id: ME }, snapshot, t)).toBe(
      'messages.system.mute_all_on:{"actor":"林知夏"}'
    )
    expect(systemNotice('speech_banned', { actor_id: ME, user_id: PEER }, snapshot, t)).toBe(
      'messages.system.speech_banned:{"actor":"林知夏","name":"周予安"}'
    )
    expect(systemNotice('join_request', { actor_name: '陈可' }, snapshot, t)).toBe(
      'messages.system.join_request:{"name":"陈可"}'
    )
    expect(systemNotice('not_a_code', {}, snapshot, t)).toBe('messages.system.unknown')
  })

  it('derives read state from the other members cursors', () => {
    const mine = message(2, { sender: { kind: 'member', id: ME } })
    expect(readState(mine, detail(), ME)).toEqual({ kind: 'read' })
    expect(readState(message(3, { sender: { kind: 'member', id: ME } }), detail(), ME)).toEqual({ kind: 'unread' })
    expect(readState(message(2), detail(), ME)).toBeNull()
    const group = detail({
      kind: 'group',
      members: [
        { userId: ME, role: 'owner', left: false, lastReadSequence: 9 },
        { userId: PEER, role: 'member', left: false, lastReadSequence: 1 },
        { userId: THIRD, role: 'member', left: false, lastReadSequence: 0 }
      ]
    })
    expect(readState(mine, group, ME)).toEqual({ kind: 'unread_count', count: 2 })
  })

  it('allows recall of own messages inside 5 minutes', () => {
    const mine = message(2, { sender: { kind: 'member', id: ME } })
    expect(canRecall(mine, ME, mine.createdAt + 1000)).toBe(true)
    expect(canRecall(mine, ME, mine.createdAt + 5 * 60 * 1000 - 1)).toBe(true)
    expect(canRecall(mine, ME, mine.createdAt + 5 * 60 * 1000)).toBe(false)
    expect(canRecall(message(2), ME, Date.now())).toBe(false)
  })

  it('orders departments parent first', () => {
    expect(
      departmentTree([
        { id: 'b', parentId: 'a', name: '设计部', sortOrder: 1 },
        { id: 'a', parentId: null, name: '总部', sortOrder: 0 },
        { id: 'c', parentId: 'a', name: '研发部', sortOrder: 0 }
      ])
    ).toEqual([
      { id: 'a', name: '总部', depth: 0 },
      { id: 'c', name: '研发部', depth: 1 },
      { id: 'b', name: '设计部', depth: 1 }
    ])
  })
})
