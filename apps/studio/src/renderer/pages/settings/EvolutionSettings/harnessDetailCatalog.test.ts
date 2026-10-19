import { describe, expect, it } from 'vitest'

import {
  ADMITTED_TOOLS,
  COMPACTION_SYSTEM_PROMPT,
  INTENT_STAMP_MARKER,
  SKILL_FIELDS,
  admittedTool,
  internalsFocus,
  systemLine
} from './harnessDetailCatalog'

describe('harness internals', () => {
  it('adds the host marker only on the host assemble', () => {
    expect(systemLine('default')).toBe('')
    expect(systemLine('lean')).toBe('')
    expect(systemLine('host')).toBe(INTENT_STAMP_MARKER)
    expect(INTENT_STAMP_MARKER).toBe('a3s.meta_harness.intent_stamp.v1')
  })

  it('keeps one tool catalog, with real parameter names', () => {
    const bash = admittedTool('bash')
    expect(bash?.fields.find((field) => field.name === 'command')?.required).toBe(true)
    const skill = admittedTool('Skill')
    expect(skill?.fields.find((field) => field.name === 'skill_name')?.required).toBe(true)
    expect(skill?.fields.find((field) => field.name === 'prompt')?.required).toBe(false)
    expect(admittedTool('search_skills')).toBeUndefined()
    expect(admittedTool('web_search')?.name).toBe('web_search')
    expect(admittedTool('web_fetch')?.name).toBe('web_fetch')
    expect(ADMITTED_TOOLS.map((tool) => tool.name)).toContain('edit')
    expect(SKILL_FIELDS).toContain('allowed-tools')
    expect(SKILL_FIELDS).toContain('content')
  })

  it('points compact and tools at their admitted text', () => {
    expect(internalsFocus('compact')).toBe('compact')
    expect(internalsFocus('tools')).toBe('tools')
    expect(internalsFocus('user-message')).toBe('none')
    expect(COMPACTION_SYSTEM_PROMPT.startsWith('You are a context-compaction engine.')).toBe(true)
  })
})
