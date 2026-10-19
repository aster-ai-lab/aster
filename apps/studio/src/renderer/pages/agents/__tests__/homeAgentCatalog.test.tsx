import { render, waitFor } from '@testing-library/react'
import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import type { TFunction } from 'i18next'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { createAgentMentionToken } from '@renderer/components/composer/agentMentionToken'
import { clearAgentParameterDraft } from '@renderer/components/composer/agentParameterDraft'
import { createComposerTextHighlightExtension } from '@renderer/components/composer/composerTextHighlight'
import { ComposerToken } from '@renderer/components/composer/tokenView/ComposerToken'
import { officialAgentAvatar } from '@renderer/pages/agents/officialAgentAvatars'

import {
  agentMentionHighlight,
  agentPromptText,
  homeAgentSuggestionItems,
  installedMentionAgents,
  marketCatalogAgents,
  rememberAgentAvatars,
  type HomeInstalledAgent
} from '../homeAgentCatalog'

const t = ((key: string, fallback?: string) => fallback ?? key) as unknown as TFunction

const agents: HomeInstalledAgent[] = [
  { id: 'b', name: '审查员', description: '检查结论和引用', avatar: '文' },
  { id: 'a', name: 'Writer', description: 'Drafts documents', avatar: '✍' },
  { id: 'c', name: '  ', description: 'blank' }
]

function insertedToken(item: ReturnType<typeof homeAgentSuggestionItems>[number]) {
  let token: { id?: string; kind?: string; label?: string; icon?: string; promptText?: string } | undefined
  const editor = {
    chain: () => ({
      focus: () => ({
        insertComposerToken: (value: typeof token) => ({
          insertContent: () => ({
            run: () => {
              token = value
            }
          })
        })
      })
    })
  }
  item.command({ editor: editor as unknown as Editor, range: { from: 0, to: 0 }, item, query: '' })
  return token
}

let editor: Editor | undefined

afterEach(() => {
  editor?.destroy()
  editor = undefined
  clearAgentParameterDraft()
})

describe('marketCatalogAgents', () => {
  it('lists the cloud agent and other market presets', () => {
    expect(
      marketCatalogAgents([
        { id: 'acp-cloud', name: 'ACP 云端智能体', description: '记住 sessionId', emoji: '☁', prompt: '记住上一轮' },
        { id: 'writer', name: ' Writer ', description: ' Drafts ', emoji: ' ✍ ' },
        { id: 'blank', name: '  ', description: 'skip' }
      ])
    ).toEqual([
      {
        id: 'acp-cloud',
        name: 'ACP 云端智能体',
        description: '记住 sessionId',
        prompt: '记住上一轮',
        avatar: '☁'
      },
      { id: 'writer', name: 'Writer', description: 'Drafts', avatar: '✍' }
    ])
  })
})

const edgeRuntime = {
  runtime: 'edge' as const,
  harness: 'builtin' as const,
  resources: { cpuMillis: 500, memoryBytes: 268435456 }
}

describe('installedMentionAgents', () => {
  it('lists saved assistants and installed packages, and leaves the rest of the catalog out', () => {
    expect(
      installedMentionAgents(
        [
          { id: 'writer', name: 'Writer', description: 'Drafts', emoji: '✍', prompt: '写清楚' },
          {
            id: 'legal-expert',
            name: '法务专家',
            description: '审查合同',
            packageSlug: 'legal-expert',
            installed: true,
            agentRuntime: edgeRuntime
          },
          {
            id: 'a3s-review',
            name: '审查员',
            description: '检查结论',
            packageSlug: 'a3s-review',
            installed: true,
            avatar: '文',
            agentRuntime: edgeRuntime
          },
          { id: 'bare', name: '无声明', packageSlug: 'bare', installed: true },
          { id: 'acp-cloud', name: 'ACP 云端智能体', description: 'retired' }
        ],
        [
          { id: 'local-writer', name: 'Writer', description: '本地', prompt: '本地提示', emoji: '笔' },
          { id: 'local-custom', name: '我的助手', description: '自己建的', emoji: '助' },
          { id: 'saved-legal', name: '法务专家', description: '已安装', emoji: '' }
        ]
      )
    ).toEqual([
      { id: 'local-writer', name: 'Writer', description: 'Drafts', prompt: '写清楚', avatar: '✍' },
      {
        id: 'legal-expert',
        name: '法务专家',
        description: '审查合同',
        avatar: officialAgentAvatar('legal-expert'),
        agentRuntime: edgeRuntime
      },
      { id: 'a3s-review', name: '审查员', description: '检查结论', avatar: '文', agentRuntime: edgeRuntime },
      { id: 'local-custom', name: '我的助手', description: '自己建的', avatar: '助' }
    ])
  })

  it('carries a verified call schema only for an installed package', () => {
    const schema = JSON.stringify({
      type: 'object',
      properties: { days: { type: 'integer', title: '天数', minimum: 1, maximum: 14 } },
      required: ['days']
    })
    const agents = installedMentionAgents(
      [
        {
          id: 'legal-expert',
          name: '法务专家',
          packageSlug: 'legal-expert',
          installed: true,
          parametersSchema: schema,
          agentRuntime: edgeRuntime
        },
        { id: 'writer', name: 'Writer', packageSlug: 'writer', installed: false, parametersSchema: schema },
        {
          id: 'broken',
          name: '坏的',
          packageSlug: 'broken',
          installed: true,
          parametersSchema: '{"type":"object","properties":{"body":{"type":"object"}}}'
        }
      ],
      []
    )
    expect(agents.find((agent) => agent.name === '法务专家')?.parametersSchema).toBe(schema)
    expect(agents.find((agent) => agent.name === 'Writer')).toBeUndefined()
    expect(agents.find((agent) => agent.name === '坏的')?.parametersSchema).toBeUndefined()
  })

  it('carries a valid run declaration and drops a mismatched one', () => {
    const agents = installedMentionAgents(
      [
        {
          id: 'weather',
          name: '天气',
          packageSlug: 'weather',
          installed: true,
          agentRuntime: { runtime: 'remote', card: 'https://agents.example.com/card' }
        },
        {
          id: 'bad',
          name: '坏运行',
          packageSlug: 'bad',
          installed: true,
          agentRuntime: { runtime: 'edge', card: 'https://agents.example.com/card' }
        }
      ],
      []
    )
    expect(agents.find((agent) => agent.name === '天气')?.agentRuntime).toEqual({
      runtime: 'remote',
      card: 'https://agents.example.com/card'
    })
    expect(agents.find((agent) => agent.name === '坏运行')?.agentRuntime).toBeUndefined()
  })
})

describe('homeAgentSuggestionItems', () => {
  it('lists installed agents in name order with the skill-panel row shape', () => {
    const items = homeAgentSuggestionItems(t, agents, '')
    expect(items.map((item) => item.label)).toEqual(['审查员', 'Writer'])
    const writer = items.find((item) => item.label === 'Writer')
    expect(writer).toMatchObject({
      description: 'Drafts documents',
      suffix: '智能体',
      filterText: 'Writer a'
    })
    expect(insertedToken(items[0]!)).toMatchObject({
      kind: 'reference',
      label: agentPromptText('审查员'),
      icon: '文',
      promptText: agentPromptText('审查员')
    })
    const withPrompt = homeAgentSuggestionItems(
      t,
      [{ id: 'legal', name: '法务专家', description: '审查合同', prompt: '先读完整份合同', avatar: '⚖' }],
      ''
    )
    expect(insertedToken(withPrompt[0]!)).toMatchObject({
      label: agentPromptText('法务专家'),
      promptText: `${agentPromptText('法务专家')}\n先读完整份合同`
    })
    expect(insertedToken(items[0]!)?.id.startsWith('agent-mention:b:')).toBe(true)
    expect(writer?.icon).toMatchObject({ props: { 'data-agent-avatar': '✍' } })
    expect(items[0]?.icon).toMatchObject({ props: { 'data-agent-avatar': '文' } })
  })

  it('paints the selected agent chip with that agent avatar', () => {
    rememberAgentAvatars(agents)
    expect(agentMentionHighlight.attributesForMatch?.(agentPromptText('审查员'))).toEqual({
      'data-agent-avatar': '文'
    })
    expect(agentMentionHighlight.attributes?.style).toContain('background:#f5f5f5')
    editor = new Editor({
      element: document.createElement('div'),
      extensions: [StarterKit, createComposerTextHighlightExtension([agentMentionHighlight])],
      content: `<p>${agentPromptText('审查员')}</p>`
    })
    const chip = editor.view.dom.querySelector('[data-composer-highlight="agent"]')
    expect(chip?.textContent).toBe(agentPromptText('审查员'))
    expect(chip?.getAttribute('style')).toContain('background:')
    expect(editor.view.dom.querySelector('[data-agent-avatar="文"]')?.textContent).toBe('文')
  })

  it('loads the stored catalog icon for an installed agent', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => new Response('<svg/>', { headers: { 'content-type': 'image/svg+xml' } }))
    )
    vi.stubGlobal('__A3S_STUDIO_RUNTIME__', { apiBaseUrl: 'http://127.0.0.1:4010', token: 'ef'.repeat(32) })
    vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:review-icon')
    const icon = '/v1/api/org/capabilities/a3s-review/icon?v=abc'
    const [item] = homeAgentSuggestionItems(
      t,
      [
        {
          id: 'a3s-review',
          name: '法务专家',
          description: '审查合同',
          avatar: icon
        }
      ],
      ''
    )
    const view = render(<>{item?.icon}</>)
    await waitFor(() => expect(view.container.querySelector('img')?.getAttribute('src')).toBe('blob:review-icon'))
    expect(view.container.textContent).not.toContain('/api/')
    view.unmount()
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it('filters by name or description and shows an empty row when nothing matches', () => {
    expect(homeAgentSuggestionItems(t, agents, 'draft').map((item) => item.label)).toEqual(['Writer'])
    const empty = homeAgentSuggestionItems(t, agents, 'missing')
    expect(empty).toEqual([
      expect.objectContaining({
        id: 'workbench-agent:empty',
        label: '没有已安装的智能体',
        disabled: true
      })
    ])
  })
})

describe('selected agent chip', () => {
  it('shows the agent name and picture, and keeps the sentence for the model', () => {
    const token = createAgentMentionToken({
      id: 'legal-expert',
      name: '领券市场',
      avatar: '/assets/agents/legal-expert.jpg'
    })
    const view = render(<ComposerToken token={token} />)
    const mark = view.container.querySelector('[data-agent-avatar] [data-avatar="image"]')
    expect(view.container.textContent).toBe('领券市场')
    expect(view.container.querySelector('img')).toBeNull()
    expect(mark?.getAttribute('style')).toContain('/assets/agents/legal-expert.jpg')
    expect(token.promptText).toBe('使用智能体 领券市场：')
  })
})
