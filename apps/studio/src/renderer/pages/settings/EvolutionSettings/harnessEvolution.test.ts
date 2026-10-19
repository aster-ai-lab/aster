import { describe, expect, it } from 'vitest'

import {
  addHarnessSkill,
  diffEvolution,
  HARNESS_EVOLUTION_KEY,
  readHarnessEvolution,
  removeHarnessSkill,
  writeHarnessTool,
  type EvolutionStorage
} from './harnessEvolution'

function memory(): EvolutionStorage {
  const jar = new Map<string, string>()
  return {
    getItem: (key) => jar.get(key) ?? null,
    setItem: (key, value) => {
      jar.set(key, value)
    }
  }
}

describe('harness evolution records', () => {
  it('seeds lookup tools, a host skill, and refuses an unknown tool name', () => {
    const storage = memory()
    const book = readHarnessEvolution(storage)

    expect(book.default.skills).toEqual([])
    expect(book.default.tools.every((tool) => tool.included)).toBe(true)
    expect(book.lean.tools.filter((tool) => tool.included).map((tool) => tool.name)).toEqual([
      'read',
      'ls',
      'search',
      'web_fetch',
      'web_search',
      'Skill'
    ])
    expect(book.host.tools.every((tool) => tool.included)).toBe(true)
    expect(book.host.skills.map((skill) => skill.name)).toEqual(['intent-review'])

    storage.setItem(
      HARNESS_EVOLUTION_KEY,
      JSON.stringify({
        lean: {
          tools: [
            { name: 'not-a-tool', included: true, description: 'no' },
            { name: 'bash', included: true, description: 'kept' }
          ]
        }
      })
    )
    const lean = readHarnessEvolution(storage).lean
    expect(lean.tools.some((tool) => tool.name === 'not-a-tool')).toBe(false)
    expect(lean.tools.find((tool) => tool.name === 'bash')).toMatchObject({ included: true, description: 'kept' })
    expect(lean.tools.find((tool) => tool.name === 'read')?.included).toBe(true)

    storage.setItem(HARNESS_EVOLUTION_KEY, '{')
    expect(readHarnessEvolution(storage).default.tools.every((tool) => tool.included)).toBe(true)
  })

  it('diffs created skills and deleted tools, and round-trips an edit', () => {
    const storage = memory()
    const book = readHarnessEvolution(storage)
    const leanRows = diffEvolution(book.default, book.lean, { base: '改仓库里的代码', next: '只查资料' })
    expect(leanRows.find((row) => row.asset === 'scenario')?.kind).toBe('optimized')
    expect(leanRows.find((row) => row.asset === 'tool' && row.name === 'bash')?.kind).toBe('removed')
    expect(leanRows.find((row) => row.asset === 'tool' && row.name === 'write')?.kind).toBe('removed')
    expect(leanRows.find((row) => row.name === 'read')).toBeUndefined()

    const hostRows = diffEvolution(book.default, book.host, { base: '改仓库里的代码', next: '宿主意图' })
    expect(hostRows.find((row) => row.asset === 'skill' && row.name === 'intent-review')?.kind).toBe('added')
    expect(hostRows.find((row) => row.asset === 'tool' && row.name === 'bash')).toBeUndefined()

    writeHarnessTool('host', 'read', { description: '只读一段文本。' }, storage)
    const revised = diffEvolution(readHarnessEvolution(storage).default, readHarnessEvolution(storage).host, {
      base: '改代码',
      next: '宿主'
    })
    expect(revised.find((row) => row.asset === 'tool' && row.name === 'read')?.kind).toBe('optimized')

    const added = addHarnessSkill('lean', storage)
    expect(readHarnessEvolution(storage).lean.skills.map((skill) => skill.id)).toEqual([added])
    removeHarnessSkill('host', 'intent-review', storage)
    expect(readHarnessEvolution(storage).host.skills).toEqual([])
  })
})
