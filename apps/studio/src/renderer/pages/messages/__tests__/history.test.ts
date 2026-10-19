import { describe, expect, it } from 'vitest'

import { encodeQuotedText, makeQuote } from '@renderer/im/quote'

import { dayKey, filterHistory, highlightParts, toHistoryRecord, type HistoryKind } from '../history'

function record(id: string, kind: Exclude<HistoryKind, 'link'>, text: string, at: number) {
  return toHistoryRecord({ id, at, senderName: '周予安', kind, text })
}

describe('history', () => {
  it('promotes links, filters kinds and a day, and marks the query', () => {
    const morning = Date.parse('2026-10-06T09:00:00')
    const noon = Date.parse('2026-10-06T12:00:00')
    const rows = [
      record('link', 'text', 'see https://a3s.dev/docs', Date.parse('2026-10-06T10:00:00')),
      record('file', 'file', 'notes.pdf', morning),
      record('image', 'image', '[图片]', Date.parse('2026-10-05T09:00:00')),
      record('text', 'text', 'hello', noon)
    ]
    expect(rows[0]?.kind).toBe('link')
    expect(rows[2]?.thumb).toBe(true)
    expect(filterHistory(rows, '', 'link', null).map((item) => item.id)).toEqual(['link'])
    expect(filterHistory(rows, '', 'file', null).map((item) => item.id)).toEqual(['file'])
    expect(filterHistory(rows, '', 'image', null).map((item) => item.id)).toEqual(['image'])
    expect(filterHistory(rows, 'HELLO', 'all', null).map((item) => item.id)).toEqual(['text'])
    const day = dayKey(noon)
    expect(filterHistory(rows, '', 'all', { start: day, end: day }).map((item) => item.id)).toEqual([
      'text',
      'link',
      'file'
    ])
    expect(filterHistory(rows, '', 'file', { start: day, end: day }).map((item) => item.id)).toEqual(['file'])
    expect(filterHistory(rows, '', 'all', { start: '2026-10-05', end: day }).map((item) => item.id)).toEqual([
      'text',
      'link',
      'file',
      'image'
    ])
    expect(filterHistory(rows, '', 'all', { start: '2026-10-01', end: '2026-10-04' })).toEqual([])
    expect(highlightParts('Hello hello', 'HELLO').map((part) => [part.text, part.hit])).toEqual([
      ['Hello', true],
      [' ', false],
      ['hello', true]
    ])
  })

  it('indexes the visible text of a quoted message', () => {
    const encoded = encodeQuotedText('收到', makeQuote('m-1', 'peer', '周予安', 'text', 'hello'))
    const row = toHistoryRecord({ id: 'q', at: 1, senderName: '周予安', kind: 'text', text: encoded })
    expect(row.text).toBe('收到')
    expect(row.text.includes('A3SQ1')).toBe(false)
    expect(filterHistory([row], 'hello', 'all', null)).toEqual([])
    expect(filterHistory([row], '收到', 'all', null).map((item) => item.id)).toEqual(['q'])
  })
})
