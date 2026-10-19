import { describe, expect, it } from 'vitest'

import { splitOpenUiFences } from '../splitOpenUiFences'

describe('OpenUI message framing', () => {
  it('keeps identity and source across completion of an open fence', () => {
    const source = 'Before\n\n```openui\nroot = Card("Budget")\n'
    const early = splitOpenUiFences(source)
    const settled = splitOpenUiFences(source + '```\n\nAfter')
    expect(early[1]).toEqual({ kind: 'openui', source: 'root = Card("Budget")\n', start: 8, incomplete: true })
    expect(settled[1].start).toBe(early[1].start)
    expect(settled[1].incomplete).toBe(false)
    expect(settled[2].source).toBe('\nAfter')
  })

  it('does not mount UI from a nested fence or an indented fence', () => {
    for (const source of [
      '````mdx\n```openui\nroot = Card("Budget")\n```\n````',
      '    ```openui\nroot = Card("Budget")\n    ```'
    ])
      expect(splitOpenUiFences(source)).toEqual([{ kind: 'markdown', source, start: 0 }])
  })

  it('keeps ordinary json as markdown', () => {
    const source = '```json\n{"ok":true}\n```'
    expect(splitOpenUiFences(source)).toEqual([{ kind: 'markdown', source, start: 0 }])
  })
})
