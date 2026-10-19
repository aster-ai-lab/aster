import { describe, expect, it } from 'vitest'

import { bodySearchHits, bodySearchPassages, noteHeadings, noteReading, type ReadingNote } from '../noteReading'

const notes: ReadingNote[] = [
  { name: '甲.md', path: '/vault/甲.md', markdown: '见 [[乙]] 和 [[团队纪要]]\n#topic\n- [ ] 买票\n- [x] 已订' },
  { name: '乙.md', path: '/vault/乙.md', markdown: '---\ntitle: 乙\nstatus: draft\n---\n乙的正文' },
  { name: '丙.md', path: '/vault/丙.md', markdown: null }
]

describe('note reading', () => {
  it('opens a resolved wikilink and keeps an unknown target unresolved', () => {
    const reading = noteReading(notes, '甲.md', notes[0].markdown ?? '')
    expect(reading.outgoing).toEqual([{ label: '乙', path: '/vault/乙.md' }])
    expect(reading.unresolved).toEqual([{ label: '团队纪要', target: '团队纪要' }])
    expect(reading.tags).toEqual(['topic'])
    expect(reading.todos).toEqual(['买票'])
    expect(reading.properties).toEqual([])
  })

  it('lists the saved source as a backlink and reads flat properties except title', () => {
    const reading = noteReading(notes, '乙.md', notes[1].markdown ?? '')
    expect(reading.backlinks).toEqual([{ label: '甲', path: '/vault/甲.md', line: '见 [[乙]] 和 [[团队纪要]]' }])
    expect(reading.properties).toEqual([{ key: 'status', value: 'draft' }])
    expect(reading.outgoing).toEqual([])
  })

  it('reads links and to-dos from the editor escape form', () => {
    const saved = '\\[\\[乙\\]\\]\n\n#topic\n\n- \\[ \\] 买票\n'
    const reading = noteReading(
      [
        { name: '甲.md', path: '/vault/甲.md', markdown: saved },
        { name: '乙.md', path: '/vault/乙.md', markdown: '\\[\\[甲\\]\\]\n' }
      ],
      '甲.md',
      saved
    )
    expect(reading.outgoing).toEqual([{ label: '乙', path: '/vault/乙.md' }])
    expect(reading.backlinks).toEqual([{ label: '乙', path: '/vault/乙.md', line: '\\[\\[甲\\]\\]' }])
    expect(reading.tags).toEqual(['topic'])
    expect(reading.todos).toEqual(['买票'])
  })

  it('searches saved text only when the file name does not already match', () => {
    expect(bodySearchHits(notes, '买票').map((note) => note.name)).toEqual(['甲.md'])
    expect(bodySearchHits(notes, '乙').map((note) => note.name)).toEqual(['甲.md'])
    expect(bodySearchHits(notes, '  ')).toEqual([])
  })

  it('returns the matching source line even when the file name also matches', () => {
    expect(bodySearchPassages(notes, '乙').map((hit) => hit.name)).toEqual(['甲.md', '乙.md'])
    expect(bodySearchPassages(notes, '买票')[0]?.line).toBe('- [ ] 买票')
    expect(bodySearchPassages(notes, '  ')).toEqual([])
  })

  it('reads draft headings and skips headings inside a fence', () => {
    expect(noteHeadings('# 真标题\n\n```\n# 不是标题\n```\n\n## 第二节\n#topic\n')).toEqual([
      { level: 1, text: '真标题' },
      { level: 2, text: '第二节' }
    ])
  })
})
