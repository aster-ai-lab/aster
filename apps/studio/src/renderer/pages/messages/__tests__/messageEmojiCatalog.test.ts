import { describe, expect, it } from 'vitest'

import { messageEmojisByGroup, messageEmojiLabel, searchMessageEmojis } from '../messageEmojiCatalog'

describe('messageEmojiCatalog', () => {
  it('searches Chinese and English names and keeps the grinning face in smileys', () => {
    expect(messageEmojisByGroup.smileys.some((item) => item.emoji === '😀')).toBe(true)
    expect(messageEmojisByGroup.smileys.some((item) => item.emoji === '😅')).toBe(true)
    expect(searchMessageEmojis('苦笑').some((item) => item.emoji === '😅')).toBe(true)
    expect(searchMessageEmojis('grinning face with sweat').some((item) => item.emoji === '😅')).toBe(true)
    expect(searchMessageEmojis('苦笑').some((item) => item.emoji === '😀')).toBe(false)
    expect(searchMessageEmojis('zzzz-no-such-emoji')).toEqual([])
    expect(messageEmojiLabel('😀', 'zh-CN')).toBe('嘿嘿')
    expect(messageEmojiLabel('😀', 'en-US')).toBe('grinning face')
  })
})
