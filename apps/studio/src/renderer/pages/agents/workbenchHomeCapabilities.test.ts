import { describe, expect, it } from 'vitest'

import {
  activePluginCandidates,
  homeTurnContext,
  homeTurnDirectives,
  composerGoalObjective,
  composerGoalSummary,
  durableGoalFile,
  insertVoiceTranscript,
  parseDurableGoal,
  sessionGoalFromUserTexts,
  taskGoalPresentation,
  userTextsFromSessionMessages,
  mergeScreenshotNotes,
  readHomeComposerOptions,
  recognitionEventTranscripts,
  researchBlocksSend,
  withoutComposerGoal,
  writeHomeComposerOptions
} from './workbenchHomeCapabilities'

describe('workbench home composer capabilities', () => {
  it('keeps host review and provenance out of the author instructions', () => {
    for (const reviewMode of [false, true])
      for (const artifactTrace of [false, true]) {
        const context = homeTurnContext({ reviewMode, artifactTrace, researchMode: false })
        expect(context.workbench).toEqual({ reviewMode, artifactTrace })
        expect(context.directives).toEqual([])
      }
    expect(homeTurnDirectives({ reviewMode: true, artifactTrace: true, researchMode: true })).toHaveLength(1)
  })

  it('blocks research sends only while resources are attached', () => {
    const flags = { reviewMode: false, artifactTrace: false, researchMode: true }
    expect(researchBlocksSend(flags, 1)).toBe(true)
    expect(researchBlocksSend(flags, 0)).toBe(false)
    expect(researchBlocksSend({ ...flags, researchMode: false }, 2)).toBe(false)
  })

  it('reads a goal command and can clear it', () => {
    expect(composerGoalSummary('你好\n/goal 整理周报')).toBe('整理周报')
    expect(composerGoalSummary('/goal')).toBe('')
    expect(composerGoalSummary('普通消息')).toBeNull()
    expect(withoutComposerGoal('前文\n/goal 整理周报')).toBe('前文')
  })

  it('treats goal verbs as commands and keeps a typed objective', () => {
    expect(composerGoalObjective('/goal 整理周报')).toBe('整理周报')
    expect(composerGoalObjective('/goal')).toBe('')
    expect(composerGoalObjective('/goal clear')).toBeNull()
    expect(composerGoalObjective('/goal PAUSE')).toBeNull()
    expect(composerGoalObjective('/goal Pause the rollout')).toBe('Pause the rollout')
    expect(parseDurableGoal('{"objective":" 清理垃圾文件 ","paused":true}')).toEqual({
      objective: '清理垃圾文件',
      paused: true
    })
    expect(parseDurableGoal('{"objective":""}')).toBeNull()
    expect(parseDurableGoal('not json')).toBeNull()
    expect(durableGoalFile('/work/a3s/')).toBe('/work/a3s/.a3s/durable-goal.json')
    expect(taskGoalPresentation('整理周报', { objective: '旧目标', paused: true })).toEqual({
      text: '整理周报',
      paused: false,
      clearable: true
    })
    expect(taskGoalPresentation('', { objective: '清理垃圾文件', paused: true })).toEqual({
      text: '清理垃圾文件',
      paused: true,
      clearable: false
    })
    expect(taskGoalPresentation(null, null)).toBeNull()
    expect(taskGoalPresentation('', null)).toBeNull()
  })

  it('keeps a goal only for the conversation that set it', () => {
    expect(
      userTextsFromSessionMessages([
        { role: 'assistant', searchableText: 'done' },
        { role: 'user', searchableText: '/goal 分析并清理一下当前项目的垃圾文件' },
        { role: 'user', searchableText: '继续' }
      ])
    ).toEqual(['/goal 分析并清理一下当前项目的垃圾文件', '继续'])
    expect(sessionGoalFromUserTexts(['/goal 分析并清理一下当前项目的垃圾文件', '继续'])).toEqual({
      objective: '分析并清理一下当前项目的垃圾文件',
      paused: false
    })
    expect(
      sessionGoalFromUserTexts(['/goal 整理周报', 'hello', '/goal pause', '/goal resume', '/goal clear'])
    ).toBeNull()
    expect(sessionGoalFromUserTexts(['please /goal 整理周报', '/goalie ship it'])).toBeNull()
  })

  it('lists only active plugins that match the query', () => {
    const servers = [
      { id: 'a', name: '文档', description: 'docs', isActive: true },
      { id: 'b', name: '闲置', isActive: false }
    ]
    expect(activePluginCandidates(servers, '').map((server) => server.id)).toEqual(['a'])
    expect(activePluginCandidates(servers, 'doc').map((server) => server.id)).toEqual(['a'])
    expect(activePluginCandidates(servers, '闲置')).toEqual([])
  })

  it('inserts a voice transcript with a space between latin words', () => {
    expect(insertVoiceTranscript('Hello', 'world', 5, 5)).toEqual({ cursor: 11, value: 'Hello world' })
    expect(insertVoiceTranscript('你好', '世界', 2, 2).value).toBe('你好世界')
  })

  it('keeps the live voice preview and only commits new final text', () => {
    const transcripts = recognitionEventTranscripts(
      {
        length: 2,
        0: { isFinal: true, 0: { transcript: '今天' } },
        1: { isFinal: false, 0: { transcript: '想' } }
      },
      1
    )
    expect(transcripts.previewTranscript).toBe('今天想')
    expect(transcripts.finalTranscript).toBe('')
  })

  it('turns screenshot pins into prompt notes', () => {
    expect(mergeScreenshotNotes('看这里', [{ x: 0.5, y: 0.25, note: '标题' }])).toBe('看这里\n\n截图标注：\n1. 标题')
  })

  it('remembers homepage option toggles in the given storage', () => {
    const values = new Map<string, string>()
    const storage = {
      getItem: (key: string) => values.get(key) ?? null,
      setItem: (key: string, value: string) => {
        values.set(key, value)
      }
    }
    writeHomeComposerOptions({ reviewMode: true, artifactTrace: false, researchMode: true }, storage)
    expect(readHomeComposerOptions(storage)).toEqual({
      reviewMode: true,
      artifactTrace: false,
      researchMode: true
    })
  })

  it('submits each policy as a typed turn contract independent of later settings', () => {
    const first = homeTurnContext({ reviewMode: true, artifactTrace: false, researchMode: false })
    const second = homeTurnContext({ reviewMode: false, artifactTrace: true, researchMode: false })
    expect(second.workbench).toEqual({ reviewMode: false, artifactTrace: true })
    expect(first.workbench).toEqual({ reviewMode: true, artifactTrace: false })
  })
  it('reports storage failures without throwing or changing the saved settings', () => {
    expect(
      writeHomeComposerOptions(
        { reviewMode: true, artifactTrace: true, researchMode: false },
        {
          setItem() {
            throw new Error('quota')
          }
        }
      )
    ).toBe(false)
  })
})
