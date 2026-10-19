import { act, render } from '@testing-library/react'
import { createElement } from 'react'
import { beforeEach, describe, expect, it } from 'vitest'

import { receiveAppletHomeDocument, resetAppletHomeForTests } from '@renderer/services/plugins/aixAppletHome'
import { publishAixCatalogue } from '@renderer/services/plugins/aixCatalogueStore'
import type { CherryMessagePart } from '@shared/data/types/message'
import type { AixWorkbenchCatalogue } from '@shared/types/aixWorkbench'

import { readPresentationDeck, resetPresentationDeckForTests } from '../presentationDeck'
import {
  EDIT_OUTLINE_TOOL,
  EDIT_SLIDE_TOOL,
  GENERATE_DECK_TOOL,
  PresentationGenerateBridge,
  READ_DECK_TOOL,
  applyGenerateDeckPart,
  normalizeGenerateDeckSeed,
  resetGenerateDeckForTests
} from '../presentationGenerate'
import { PRESENTON_PACKAGE_ID, PRESENTON_SLUG, PRESENTON_TITLE } from '../presentonHomepage'

const REVISION = `sha256:${'ab'.repeat(32)}`

function installedCatalogue(packageId = PRESENTON_PACKAGE_ID): AixWorkbenchCatalogue {
  return {
    protocolVersion: 1,
    revision: '',
    packages: [
      {
        slug: PRESENTON_SLUG,
        packageId,
        generation: 4,
        revision: REVISION,
        containers: [],
        views: [
          {
            id: 'workbench',
            title: PRESENTON_TITLE,
            description: '',
            icon: 'panels',
            order: 20,
            container: null,
            point: 'studio.applets',
            presentation: 'webview'
          }
        ]
      }
    ]
  }
}

function part(input: unknown, extra: Record<string, unknown> = {}): CherryMessagePart {
  return {
    type: 'dynamic-tool',
    toolName: GENERATE_DECK_TOOL,
    toolCallId: 'call-1',
    state: 'output-available',
    input,
    ...extra
  } as CherryMessagePart
}

describe('generate_deck', () => {
  beforeEach(() => {
    resetGenerateDeckForTests()
    resetAppletHomeForTests()
    resetPresentationDeckForTests()
    publishAixCatalogue(installedCatalogue())
  })

  it('fills the closed-set defaults and refuses a value outside them', () => {
    expect(normalizeGenerateDeckSeed({ content: '  项目总结  ' })).toEqual({
      content: '项目总结',
      n_slides: null,
      language: 'Chinese (Simplified - 中文, 汉语)',
      tone: 'default',
      verbosity: 'standard',
      include_title_slide: true,
      instructions: ''
    })
    expect(normalizeGenerateDeckSeed({ content: '项目总结', language: '中文' })).toBeNull()
    expect(normalizeGenerateDeckSeed({ content: '项目总结', n_slides: 21 })).toBeNull()
    expect(normalizeGenerateDeckSeed({ content: '项目总结', n_slides: 1.5 })).toBeNull()
    expect(normalizeGenerateDeckSeed({ content: '项目总结', instructions: '字'.repeat(4001) })).toBeNull()
    expect(normalizeGenerateDeckSeed({})).toBeNull()
    expect(normalizeGenerateDeckSeed({ content: '   ' })).toBeNull()
  })

  it('seeds the current task once per successful call', async () => {
    const channel = new MessageChannel()
    const messages: Array<{ type?: string; seed?: { content?: string; n_slides?: unknown; tone?: string } }> = []
    channel.port1.onmessage = (event: MessageEvent) => messages.push(event.data)
    receiveAppletHomeDocument(PRESENTON_SLUG, 'workbench', channel.port2)
    channel.port1.start()
    channel.port2.start()
    expect(applyGenerateDeckPart('task-1', part({ content: '项目总结' }))).toBe('applied')
    expect(applyGenerateDeckPart('task-1', part({ content: '项目总结' }))).toBe('idle')
    expect(readPresentationDeck()).toMatchObject({
      sessionId: 'task-1',
      title: PRESENTON_TITLE,
      topic: '项目总结',
      lines: []
    })
    for (let attempt = 0; attempt < 20 && messages.length < 1; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 0))
    }
    expect(messages[0]?.type).toBe('aix.view.seed')
    expect(messages[0]?.seed).toMatchObject({ content: '项目总结', n_slides: null, include_title_slide: true })

    expect(
      applyGenerateDeckPart(
        'task-1',
        part({ content: '第二稿', n_slides: 8, tone: 'professional' }, { toolCallId: 'call-2' })
      )
    ).toBe('applied')
    for (let attempt = 0; attempt < 20 && messages.length < 2; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 0))
    }
    expect(messages[1]?.seed).toMatchObject({ content: '第二稿', n_slides: 8, tone: 'professional' })
  })

  it('gives the replacement document a seed that was already posted to the open one', async () => {
    const first = new MessageChannel()
    receiveAppletHomeDocument(PRESENTON_SLUG, 'workbench', first.port2)
    first.port1.start()
    first.port2.start()
    expect(applyGenerateDeckPart('task-1', part({ content: '季度业务回顾' }))).toBe('applied')
    const next = new MessageChannel()
    const received = new Promise<unknown>((resolve) => {
      next.port1.onmessage = (event: MessageEvent) => resolve(event.data)
    })
    receiveAppletHomeDocument(PRESENTON_SLUG, 'workbench', next.port2)
    next.port1.start()
    next.port2.start()
    const posted = (await received) as { type?: string; seed?: { content?: string } }
    expect(posted.type).toBe('aix.view.seed')
    expect(posted.seed?.content).toBe('季度业务回顾')
  })

  it('ignores another tool, an unfinished call, and an error result', () => {
    expect(
      applyGenerateDeckPart('task-1', part({ content: '项目总结' }, { toolName: 'mcp__other__generate_deck' }))
    ).toBe('idle')
    expect(applyGenerateDeckPart('task-1', part({ content: '项目总结' }, { state: 'input-available' }))).toBe('idle')
    expect(applyGenerateDeckPart('task-1', part({ content: '项目总结' }, { state: 'output-error' }))).toBe('idle')
    expect(applyGenerateDeckPart('task-1', part({ content: '项目总结' }, { output: { isError: true } }))).toBe('idle')
    expect(applyGenerateDeckPart('', part({ content: '项目总结' }))).toBe('idle')
    expect(readPresentationDeck()).toBeNull()
  })

  it('posts a read and an edit on the editor port after the topic seed', async () => {
    expect(applyGenerateDeckPart('task-1', part({ content: '项目总结' }))).toBe('applied')
    expect(applyGenerateDeckPart('task-1', part({}, { toolName: READ_DECK_TOOL, toolCallId: 'read-1' }))).toBe(
      'applied'
    )
    expect(
      applyGenerateDeckPart(
        'task-1',
        part({ index: 1, content: '第二节' }, { toolName: EDIT_OUTLINE_TOOL, toolCallId: 'outline-1' })
      )
    ).toBe('applied')
    expect(
      applyGenerateDeckPart(
        'task-1',
        part({ index: 2, text: '新的一页', note: '备注' }, { toolName: EDIT_SLIDE_TOOL, toolCallId: 'slide-1' })
      )
    ).toBe('applied')
    const channel = new MessageChannel()
    const messages: Array<{ type?: string; seed?: { content?: string }; part?: { toolName?: string } }> = []
    channel.port1.onmessage = (event: MessageEvent) => messages.push(event.data)
    receiveAppletHomeDocument(PRESENTON_SLUG, 'workbench', channel.port2)
    channel.port1.start()
    channel.port2.start()
    for (let attempt = 0; attempt < 20 && messages.length < 4; attempt += 1) {
      await new Promise((resolve) => setTimeout(resolve, 0))
    }
    expect(messages.map((message) => message.type)).toEqual([
      'aix.view.seed',
      'aix.view.deck',
      'aix.view.deck',
      'aix.view.deck'
    ])
    expect(messages[0]?.seed?.content).toBe('项目总结')
    expect(messages.slice(1).map((message) => message.part?.toolName)).toEqual([
      READ_DECK_TOOL,
      EDIT_OUTLINE_TOOL,
      EDIT_SLIDE_TOOL
    ])
    expect(readPresentationDeck()).toMatchObject({ topic: '项目总结', lines: [] })
    expect(
      applyGenerateDeckPart(
        'task-1',
        part({ index: 2, text: '新的一页', note: '备注' }, { toolName: EDIT_SLIDE_TOOL, toolCallId: 'slide-1' })
      )
    ).toBe('idle')
    expect(
      applyGenerateDeckPart(
        'task-1',
        part(
          { index: 0, text: '不会写入', note: '' },
          { toolName: 'mcp__presenton__edit_slides', toolCallId: 'lookalike' }
        )
      )
    ).toBe('idle')
    expect(
      applyGenerateDeckPart(
        'task-1',
        part(
          { index: 0, text: '失败', note: '' },
          { toolName: EDIT_SLIDE_TOOL, toolCallId: 'failed', output: { isError: true } }
        )
      )
    ).toBe('idle')
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(messages).toHaveLength(4)
  })

  it('drops the armed deck and ignores a later tool result after uninstall', async () => {
    const parts = { message: [part({ content: '项目总结' })] }
    render(createElement(PresentationGenerateBridge, { sessionId: 'task-1', partsByMessageId: parts }))
    expect(readPresentationDeck()).toMatchObject({ topic: '项目总结' })
    act(() => publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] }))
    expect(readPresentationDeck()).toBeNull()
    const afterUninstall = applyGenerateDeckPart('task-1', part({ content: '项目总结' }, { toolCallId: 'call-2' }))
    expect(afterUninstall).toBe('idle')
    console.log(`tool_result_after_uninstall=${afterUninstall}`)
    const channel = new MessageChannel()
    let posted = false
    channel.port1.onmessage = () => {
      posted = true
    }
    receiveAppletHomeDocument(PRESENTON_SLUG, 'workbench', channel.port2)
    channel.port1.start()
    channel.port2.start()
    await new Promise((resolve) => setTimeout(resolve, 0))
    expect(posted).toBe(false)

    publishAixCatalogue(installedCatalogue('use/other/presenton'))
    expect(applyGenerateDeckPart('task-1', part({ content: '项目总结' }, { toolCallId: 'call-3' }))).toBe('idle')
    expect(readPresentationDeck()).toBeNull()
  })

  it('applies the same generate_deck result again after the plugin is installed again', async () => {
    const parts = { message: [part({ content: '项目总结' })] }
    render(createElement(PresentationGenerateBridge, { sessionId: 'task-1', partsByMessageId: parts }))
    expect(readPresentationDeck()).toMatchObject({ topic: '项目总结' })
    act(() => publishAixCatalogue({ protocolVersion: 1, revision: '', packages: [] }))
    expect(readPresentationDeck()).toBeNull()
    expect(applyGenerateDeckPart('task-1', part({ content: '项目总结' }))).toBe('idle')

    const channel = new MessageChannel()
    const received = new Promise<unknown>((resolve) => {
      channel.port1.onmessage = (event: MessageEvent) => resolve(event.data)
    })
    receiveAppletHomeDocument(PRESENTON_SLUG, 'workbench', channel.port2)
    channel.port1.start()
    channel.port2.start()
    act(() => publishAixCatalogue(installedCatalogue()))
    expect(readPresentationDeck()).toMatchObject({ sessionId: 'task-1', topic: '项目总结' })
    const posted = (await received) as { type?: string; seed?: { content?: string } }
    expect(posted.type).toBe('aix.view.seed')
    expect(posted.seed?.content).toBe('项目总结')
  })
})
