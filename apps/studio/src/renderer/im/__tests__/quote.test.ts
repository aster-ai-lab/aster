import { describe, expect, it } from 'vitest'

import {
  BOSS_MARK,
  decodeBossWire,
  decodeQuotedText,
  encodeBossWire,
  encodeQuotedText,
  makeQuote,
  visibleText
} from '../quote'

describe('quote', () => {
  it('round-trips one quote and leaves a broken suffix as plain text', () => {
    const quote = makeQuote('m-1', 'peer', ' 周予安 ', 'text', '  hello   1 ')
    expect(quote).toMatchObject({ senderName: '周予安', excerpt: 'hello 1', kind: 'text' })
    const raw = encodeQuotedText('收到', quote)
    expect(visibleText(raw)).toBe('收到')
    expect(decodeQuotedText(raw).quote).toMatchObject({ messageId: 'm-1', excerpt: 'hello 1' })
    expect(raw).toContain('A3SQ1:')

    expect(decodeQuotedText('收到\u2063A3SQ1:not-json').quote).toBeNull()
    expect(decodeQuotedText('收到\u2063A3SQ1:not-json').text).toContain('A3SQ1')
    expect(visibleText('收到\u2063A3SQ1:not-json')).toBe('收到')
    expect(decodeQuotedText('收到\u2063A3SQ1:%7B%22i%22%3A%22m%22%2C%22e%22%3A%22%20%22%7D').quote).toBeNull()

    const again = encodeQuotedText(visibleText(raw), makeQuote('m-2', 'me', '林知夏', 'file', 'notes.pdf'))
    expect(decodeQuotedText(again).text).toBe('收到')
    expect(decodeQuotedText(again).quote).toMatchObject({ messageId: 'm-2', kind: 'file' })
  })

  it('hides a ToBoss suffix and keeps a join mark through trim', () => {
    const reply = encodeBossWire('answer', 'reply')
    expect(reply).toContain(BOSS_MARK)
    expect(visibleText(reply)).toBe('answer')
    expect(decodeQuotedText(reply)).toEqual({ text: 'answer', quote: null })
    expect(decodeBossWire(reply)).toEqual({ text: 'answer', kind: 'reply' })
    expect(encodeBossWire('', 'join').trim().length).toBeGreaterThan(0)
    const huge = encodeBossWire('答'.repeat(20_000), 'reply')
    expect(new TextEncoder().encode(huge).length).toBeLessThanOrEqual(16 * 1024)
    const kept = decodeBossWire(huge)
    expect(kept?.kind).toBe('reply')
    expect(kept?.text.length).toBeGreaterThan(0)
    expect(kept?.text.endsWith('答')).toBe(true)
  })
})
