import { describe, expect, it } from 'vitest'

import { recentNotes, type VaultTreeNode } from '../knowledgeVault'

function file(name: string, modifiedMs: number): VaultTreeNode {
  return { name, path: `/vault/${name}`, kind: 'file', children: [], modifiedMs }
}

describe('recent notes', () => {
  it('keeps the five newest listing timestamps and does not read bodies', () => {
    const notes = recentNotes(
      [
        file('旧.md', 1),
        {
          name: '夹',
          path: '/vault/夹',
          kind: 'directory',
          children: [file('夹/新.md', 9)]
        },
        file('中.md', 4),
        file('次.md', 5),
        file('再.md', 6),
        file('更.md', 7),
        file('最新.md', 8)
      ],
      5
    )
    expect(notes.map((note) => note.name)).toEqual(['夹/新.md', '最新.md', '更.md', '再.md', '次.md'])
  })
})
