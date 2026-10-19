import { describe, expect, it } from 'vitest'

import { backlinkNotes, findNote, noteLinks, noteTitle } from '../knowledgeLinks'

const local = [
  { name: '甲.md', markdown: '见 [[乙]] 和 [乙文](乙.md)\n[[团队纪要]]' },
  { name: '乙.md', markdown: '乙的正文' }
]
const team = [{ name: '团队纪要.md', markdown: '只在团队' }]

describe('knowledge note links', () => {
  it('reads a wikilink and a markdown link from the note text', () => {
    expect(noteLinks(local[0].markdown)).toEqual([
      { target: '乙', label: '乙' },
      { target: '团队纪要', label: '团队纪要' },
      { target: '乙.md', label: '乙文' }
    ])
    expect(noteTitle('notes/乙.markdown')).toBe('乙')
  })

  it('resolves a link only inside the library that was passed in', () => {
    expect(findNote(local, '乙')?.name).toBe('乙.md')
    expect(findNote(local, '乙.md')?.name).toBe('乙.md')
    expect(findNote(local, '团队纪要')).toBeUndefined()
    expect(findNote(team, '团队纪要')?.name).toBe('团队纪要.md')
    expect(findNote(team, '乙')).toBeUndefined()
  })

  it('reads a wikilink the editor stored with escaped brackets', () => {
    const saved = '\\[\\[乙\\]\\]\n'
    expect(noteLinks(saved)).toEqual([{ target: '乙', label: '乙' }])
    expect(backlinkNotes([{ name: '甲.md', markdown: saved }], '乙.md').map((note) => note.name)).toEqual(['甲.md'])
  })

  it('lists the source note as a backlink of the target', () => {
    expect(backlinkNotes(local, '乙.md').map((note) => note.name)).toEqual(['甲.md'])
    expect(backlinkNotes(team, '团队纪要.md')).toEqual([])
  })
})
