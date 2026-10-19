import { describe, expect, it } from 'vitest'

import { entry } from '@renderer/im/__tests__/fakeImApi'
import type { ImMessage } from '@renderer/im/imApi'
import { encodeQuotedText } from '@renderer/im/quote'
import {
  applyMention,
  entryMentionsYou,
  filterMentions,
  highlightLabels,
  mentionBeforeCaret,
  mentionQuery,
  splitMentions,
  textMentionsBot,
  textMentionsNamedBot,
  textMentionsYou
} from '@renderer/utils/messageMentions'

const labels = highlightLabels({
  everyone: '所有人',
  myName: '林知夏',
  bot: 'ToBot',
  people: [
    { label: 'Ann', me: false },
    { label: 'Anna', me: false },
    { label: '林知夏', me: true },
    { label: '周予安', me: false }
  ]
})

function textOf(sequence: number, text: string, sender = 'peer'): ImMessage {
  return {
    id: `m-${sequence}`,
    conversationId: 'group',
    sequence,
    sender: { kind: 'member', id: sender },
    body: { kind: 'text', text },
    recalled: false,
    createdAt: 0
  }
}

describe('mention query', () => {
  it('opens at a boundary and ignores an address', () => {
    expect(mentionQuery('@周', 2)).toEqual({ start: 0, end: 2, filter: '周' })
    expect(mentionQuery('你好 @', 4)).toEqual({ start: 3, end: 4, filter: '' })
    expect(mentionQuery('你好 ＠陈', 5)).toEqual({ start: 3, end: 5, filter: '陈' })
    expect(mentionQuery('lin@zhi.com', 4)).toBeNull()
    expect(mentionQuery('@周 予', 4)).toBeNull()
  })

  it('ranks a prefix ahead of a substring and inserts a trailing space', () => {
    const options = [{ label: '周予安' }, { label: '安可' }, { label: '所有人' }]
    expect(filterMentions(options, '安').map((item) => item.label)).toEqual(['安可', '周予安'])
    expect(applyMention('去 @周', { start: 2, end: 4, filter: '周' }, '周予安')).toEqual({
      text: '去 @周予安 ',
      caret: 7
    })
  })
})

describe('mention text', () => {
  it('keeps the longer name and marks everyone and me', () => {
    const pieces = splitMentions('@Anna @Ann @所有人 @林知夏 user@周予安', labels)
    expect(pieces.filter((piece) => piece.kind === 'mention')).toEqual([
      { kind: 'mention', text: '@Anna', me: false, target: 'member' },
      { kind: 'mention', text: '@Ann', me: false, target: 'member' },
      { kind: 'mention', text: '@所有人', me: true, target: 'all' },
      { kind: 'mention', text: '@林知夏', me: true, target: 'member' }
    ])
    expect(textMentionsYou('@Everyone 看一下', { myName: '林知夏', everyone: 'Everyone' })).toBe(true)
    expect(textMentionsYou('hello', { myName: '林知夏', everyone: '所有人' })).toBe(false)
  })

  it('wakes ToBot only for @ToBot or everyone', () => {
    expect(textMentionsBot('@ToBot 在吗', 'ToBot', '所有人')).toBe(true)
    expect(textMentionsBot('@everyone', 'ToBot', 'Everyone')).toBe(true)
    expect(textMentionsBot('@周予安 在吗', 'ToBot', '所有人')).toBe(false)
    expect(textMentionsBot('@messages.file_transfer 在吗', 'messages.file_transfer', 'messages.mention.all')).toBe(true)
    const quoted = encodeQuotedText('好的', {
      messageId: 'm-1',
      senderId: 'peer',
      senderName: '周予安',
      excerpt: '@ToBot 在吗',
      kind: 'text'
    })
    expect(textMentionsBot(quoted, 'ToBot', '所有人')).toBe(false)
    expect(textMentionsNamedBot('@ToBoss 你说', 'ToBoss')).toBe(true)
    expect(textMentionsNamedBot('@所有人 看一下', 'ToBoss')).toBe(false)
    expect(textMentionsNamedBot('@everyone', 'ToBoss')).toBe(false)
    expect(textMentionsBot('@everyone', 'ToBoss', 'everyone')).toBe(true)
    expect(
      textMentionsBot(
        encodeQuotedText('@ToBot 看一下', {
          messageId: 'm-1',
          senderId: 'peer',
          senderName: '周予安',
          excerpt: 'hello',
          kind: 'text'
        }),
        'ToBot',
        '所有人'
      )
    ).toBe(true)
  })

  it('deletes a finished mention and its trailing space in one key', () => {
    expect(mentionBeforeCaret('@周予安 你好', 5, labels)).toEqual({ start: 0, end: 5 })
    expect(mentionBeforeCaret('@周予安 你好', 4, labels)).toEqual({ start: 0, end: 4 })
    expect(mentionBeforeCaret('@周予安 你好', 2, labels)).toBeNull()
  })
})

describe('unread mention hint', () => {
  const identity = { myId: 'me', myName: '林知夏', everyone: '所有人' }

  it('uses an older unread message when the latest preview does not mention me', () => {
    const group = entry({
      id: 'group',
      kind: 'group',
      unread: 2,
      lastReadSequence: 1,
      lastSequence: 3,
      preview: {
        kind: 'text',
        messageId: 'm-3',
        sequence: 3,
        recalled: false,
        createdAt: 0,
        sender: { kind: 'member', id: 'peer' },
        text: '收到',
        name: null,
        code: null,
        params: {}
      }
    })
    expect(entryMentionsYou(group, [textOf(2, '@林知夏 看一下'), textOf(3, '收到')], identity)).toBe(true)
    expect(entryMentionsYou(group, [textOf(2, '@林知夏 看一下', 'me'), textOf(3, '收到')], identity)).toBe(false)
    expect(entryMentionsYou({ ...group, unread: 0 }, undefined, identity)).toBe(false)
    expect(entryMentionsYou({ ...group, kind: 'direct' }, undefined, identity)).toBe(false)
  })

  it('reads the preview before the thread has loaded', () => {
    const group = entry({
      kind: 'group',
      unread: 1,
      lastReadSequence: 1,
      preview: {
        kind: 'text',
        messageId: 'm-2',
        sequence: 2,
        recalled: false,
        createdAt: 0,
        sender: { kind: 'member', id: 'peer' },
        text: '@所有人 开会',
        name: null,
        code: null,
        params: {}
      }
    })
    expect(entryMentionsYou(group, undefined, identity)).toBe(true)
    const hidden = encodeQuotedText('收到', {
      messageId: 'm-9',
      senderId: 'peer',
      senderName: '周予安',
      excerpt: '@林知夏 看一下',
      kind: 'text'
    })
    expect(
      entryMentionsYou(
        {
          ...group,
          preview: { ...group.preview!, text: hidden }
        },
        undefined,
        identity
      )
    ).toBe(false)
  })
})
