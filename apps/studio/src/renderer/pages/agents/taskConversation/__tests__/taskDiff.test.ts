import { describe, expect, it } from 'vitest'

import { taskToolCopy } from '../taskDiff'

describe('taskToolCopy', () => {
  it('does not treat search_skills as a directory listing', () => {
    const copy = taskToolCopy('search_skills', {
      type: 'tool-search_skills',
      state: 'output-available',
      input: { query: 'a3s-test' },
      output: '找到 1 个匹配的技能。请用 Skill 调用，并传入它的 skill_name。'
    })

    expect(copy.titleKey).toBe('workbench.tool.search')
    expect(copy.titleValues).toEqual({ query: 'a3s-test' })
    expect(copy.summaryText).toContain('找到 1 个匹配的技能')
  })

  it('keeps a real ls tool as a file list', () => {
    const copy = taskToolCopy('ls', {
      type: 'tool-ls',
      state: 'output-available',
      output: 'file  README.md  1'
    })

    expect(copy.titleKey).toBe('workbench.tool.list_files')
  })

  it('turns a permission question into the command it names', () => {
    const copy = taskToolCopy('是否执行这条命令：printf a3s-code-prompt-ok ?', {
      type: 'dynamic-tool',
      state: 'approval-requested'
    })

    expect(copy.titleKey).toBe('workbench.tool.run')
    expect(copy.titleValues).toEqual({ command: 'printf a3s-code-prompt-ok' })
  })
})
