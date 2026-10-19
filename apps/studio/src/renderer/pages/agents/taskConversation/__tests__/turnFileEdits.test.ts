import { describe, expect, it } from 'vitest'

import type { CherryMessagePart } from '@shared/data/types/message'

import { turnFileEdits } from '../turn'

describe('turnFileEdits', () => {
  it('keeps the last successful edit of a path and drops reads, failures, and a streaming turn', () => {
    const parts = [
      { type: 'tool-read_file', state: 'output-available', input: { file_path: '/tmp/a.ts' } },
      { type: 'tool-edit_file', state: 'output-error', input: { file_path: '/tmp/b.ts' } },
      { type: 'tool-edit_file', state: 'output-available', input: { file_path: '/tmp/c.ts' } },
      {
        type: 'tool-write_file',
        state: 'output-available',
        input: { file_path: '/tmp/c.ts', content: 'next\n' }
      }
    ] as CherryMessagePart[]

    expect(turnFileEdits(parts, true)).toEqual([])
    const edits = turnFileEdits(parts, false)
    expect(edits.map((edit) => edit.path)).toEqual(['/tmp/c.ts'])
    expect(edits[0]?.diff.operation).toBe('write')
  })
})
