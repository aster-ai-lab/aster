import type { TFunction } from 'i18next'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { agentPromptText } from '../homeAgentCatalog'
import {
  fetchHomeLocalSkills,
  HOME_COMPOSER_TEXT_HIGHLIGHTS,
  homeComposerTextHighlights,
  homeSlashItems,
  skillPromptText
} from '../homeSlashCatalog'

const t = ((key: string, fallback?: string) => fallback ?? key) as unknown as TFunction

function adapter() {
  return { insertText: vi.fn(), focus: vi.fn() }
}

function runAction(item: ReturnType<typeof homeSlashItems>[number], inputAdapter: ReturnType<typeof adapter>) {
  item.action?.({ inputAdapter } as never)
}

describe('homeSlashItems', () => {
  it('lists local skills, then only the a3s-code commands that make sense for a new task', () => {
    const items = homeSlashItems(t, [
      { name: 'pdf', description: 'Fill PDFs', source: 'installed' }
    ])

    // Conversation-only commands and /model (the home composer has its own model picker) stay out.
    expect(items.map((item) => item.label)).toEqual([
      '创建技能',
      '创建定时任务',
      '知识理解',
      'pdf',
      '/goal',
      '/help',
      '/tools',
      '/mcp'
    ])
    expect(items.map((item) => item.label)).not.toContain('审查')
    expect(items[0]).toMatchObject({
      description: '编写一个新的技能',
      suffix: '技能',
      filterText: '创建技能 skill-creator'
    })
    expect(items[2]).toMatchObject({
      description: '用简明技术中文说明。复杂内容打开一页 HTML。',
      suffix: '技能',
      filterText: '知识理解 stc-answer'
    })
    expect(items[3]).toMatchObject({ description: 'Fill PDFs', suffix: '技能', filterText: 'pdf' })
    expect(items[4]).toMatchObject({ suffix: '指令', filterText: '/goal' })
  })

  it('inserts the command or the skill request into the composer', () => {
    const items = homeSlashItems(t, [{ name: 'pdf', description: '', source: 'user' }])
    const goal = items.find((item) => item.label === '/goal')
    const skill = items.find((item) => item.label === 'pdf')
    const create = items.find((item) => item.label === '创建技能')
    const understand = items.find((item) => item.label === '知识理解')
    const forGoal = adapter()
    runAction(goal!, forGoal)
    expect(forGoal.insertText).toHaveBeenCalledWith('/goal ', { tokenizeVariables: false })
    expect(items.map((item) => item.label)).not.toContain('审查')
    expect(forGoal.focus).toHaveBeenCalled()

    const forSkill = adapter()
    runAction(skill!, forSkill)
    expect(forSkill.insertText).toHaveBeenCalledWith(`${skillPromptText('pdf')} `, { tokenizeVariables: false })

    const forCreate = adapter()
    runAction(create!, forCreate)
    expect(forCreate.insertText).toHaveBeenCalledWith(`${skillPromptText('skill-creator')} `, {
      tokenizeVariables: false
    })
    const forUnderstand = adapter()
    runAction(understand!, forUnderstand)
    expect(forUnderstand.insertText).toHaveBeenCalledWith(`${skillPromptText('stc-answer')} `, {
      tokenizeVariables: false
    })
  })

  it('keeps installed skills and commands when home launchers are omitted', () => {
    const items = homeSlashItems(
      t,
      [
        { name: 'pdf', description: 'Fill PDFs', source: 'installed' },
        { name: 'skill-creator', description: 'local creator', source: 'installed' }
      ],
      { includeLaunchers: false }
    )
    const labels = items.map((item) => item.label)
    expect(labels).toContain('pdf')
    expect(labels).toContain('skill-creator')
    expect(labels).toContain('/goal')
    expect(labels).not.toContain('创建技能')
    expect(labels).not.toContain('创建定时任务')
    expect(labels).not.toContain('查找技能')
    expect(labels).not.toContain('知识理解')

    const creator = items.find((item) => item.label === 'skill-creator')
    const input = adapter()
    runAction(creator!, input)
    expect(input.insertText).toHaveBeenCalledWith(`${skillPromptText('skill-creator')} `, { tokenizeVariables: false })
  })

  it('omits one command and keeps the other new-task commands', () => {
    const items = homeSlashItems(t, [{ name: 'pdf', description: 'Fill PDFs', source: 'installed' }], {
      includeLaunchers: false,
      omitCommands: ['/goal']
    })
    const labels = items.map((item) => item.label)
    expect(labels).toContain('pdf')
    expect(labels).not.toContain('/goal')
    expect(labels).toEqual(expect.arrayContaining(['/help', '/tools', '/mcp']))
  })

  it('lists a skill named review as an ordinary skill', () => {
    const items = homeSlashItems(t, [{ name: 'review', description: 'local', source: 'installed' }])
    expect(items.map((item) => item.label)).not.toContain('审查')
    expect(items.map((item) => item.label)).toContain('review')
  })
})

describe('fetchHomeLocalSkills', () => {
  afterEach(() => {
    vi.unstubAllGlobals()
    Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
  })

  it('returns nothing outside the shell', async () => {
    const fetchMock = vi.fn()
    vi.stubGlobal('fetch', fetchMock)
    await expect(fetchHomeLocalSkills()).resolves.toEqual([])
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('reads the shell list with the bearer and drops malformed rows', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32) },
      configurable: true
    })
    const fetchMock = vi.fn(async () =>
      Response.json({
        skills: [
          { name: 'pdf', description: 'Fill PDFs', source: 'installed' },
          { name: '', description: 'nameless' },
          { name: 'notes', source: 'elsewhere' }
        ]
      })
    )
    vi.stubGlobal('fetch', fetchMock)

    await expect(fetchHomeLocalSkills()).resolves.toEqual([
      { name: 'pdf', description: 'Fill PDFs', source: 'installed' },
      { name: 'notes', description: '', source: 'user' }
    ])
    expect(fetchMock).toHaveBeenCalledWith('http://127.0.0.1:9/api/local-skills', {
      headers: { authorization: `Bearer ${'ab'.repeat(32)}` }
    })
  })
})

describe('HOME_COMPOSER_TEXT_HIGHLIGHTS', () => {
  const matches = (kind: string, text: string) => {
    const rule = HOME_COMPOSER_TEXT_HIGHLIGHTS.find((item) => item.attributes?.['data-composer-highlight'] === kind)
    return [...text.matchAll(rule!.pattern)].map((match) => match[0])
  }

  it('colors only whole new-task commands', () => {
    expect(matches('command', '/goal ship it and /help')).toEqual(['/goal', '/help'])
    expect(matches('command', 'a/goal /goals /compact /model')).toEqual([])
  })

  it('drops an omitted command from the chip and keeps the others', () => {
    const rule = homeComposerTextHighlights({ omitCommands: ['/goal'] }).find(
      (item) => item.attributes?.['data-composer-highlight'] === 'command'
    )
    expect([...'/goal /help /tools'.matchAll(rule!.pattern)].map((match) => match[0])).toEqual(['/help', '/tools'])
  })

  it('colors the inserted skill phrase', () => {
    expect(matches('skill', `${skillPromptText('a3s-test')}run it`)).toEqual(['使用技能 a3s-test：'])
    expect(matches('skill', `${skillPromptText('PPT 演示文稿')}生成一份客户汇报 PPT`)).toEqual([
      '使用技能 PPT 演示文稿：'
    ])
  })

  it('colors the inserted agent phrase the same way as a skill', () => {
    expect(matches('agent', `${agentPromptText('审查员')}检查这份文档`)).toEqual(['使用智能体 审查员：'])
    expect(matches('skill', `${agentPromptText('审查员')}检查这份文档`)).toEqual([])
  })

  it('does not chip /review as a builtin skill', () => {
    expect(
      HOME_COMPOSER_TEXT_HIGHLIGHTS.some((item) => item.attributes?.['data-composer-highlight'] === 'review')
    ).toBe(false)
    expect(matches('command', '/review 审查这个文件')).toEqual([])
  })
})
