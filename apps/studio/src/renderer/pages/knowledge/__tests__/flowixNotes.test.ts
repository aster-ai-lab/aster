import { describe, expect, it } from 'vitest'

import { noteExcerpt, noteHasOpenTodo, noteProperties, noteTags, noteTitle, openTodoLines } from '../flowixNotes'

const MARKDOWN = `---
title: "报销"
status: draft
---

# 报销

差旅标准写在这里 #policy

- [ ] 补发票
- [x] 已订票
`

describe('flowix notes', () => {
  it('reads tags, properties, and open to-dos from a markdown note', () => {
    expect(noteTags(MARKDOWN)).toEqual(['policy'])
    expect(noteTags('# 标题\n正文')).toEqual([])
    expect(noteProperties(MARKDOWN)).toEqual([
      { key: 'title', value: '报销' },
      { key: 'status', value: 'draft' }
    ])
    expect(noteTitle('ignored.md', noteProperties(MARKDOWN))).toBe('报销')
    expect(noteTitle('规范.md', [])).toBe('规范')
    expect(noteExcerpt(MARKDOWN)).toBe('差旅标准写在这里')
    expect(noteHasOpenTodo(MARKDOWN)).toBe(true)
    expect(openTodoLines(MARKDOWN)).toEqual(['补发票'])
    expect(openTodoLines('- \\[ \\] 补发票')).toEqual(['补发票'])
    expect(noteHasOpenTodo('- \\[ \\] 补发票')).toBe(true)
    expect(noteHasOpenTodo('- [x] 已完成')).toBe(false)
    expect(openTodoLines('- [x] 已完成')).toEqual([])
  })
})
