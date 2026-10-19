import { describe, expect, it } from 'vitest'

import { getSidebarIconLabelKey } from '@renderer/i18n/label'
import zhCn from '@renderer/i18n/locales/zh-cn.json'
import { createSidebarShortcutId, type SidebarShortcutItem } from '@shared/data/preference/preferenceTypes'

import {
  addSidebarShortcut,
  createSidebarShortcutTarget,
  getSidebarDefaultLandingUrl,
  activityBarOrderIsCustom,
  getVisibleSidebarShortcutItems,
  isPlatformActivityApp,
  isTeamOnlyActivityApp,
  markActivityBarOrderCustom,
  normalizeSidebarShortcutItems,
  removeSidebarShortcut,
  reorderSidebarShortcuts,
  temporarilyHiddenActivityBarReplacement
} from '../sidebar'

const shortcut = (providerId: string, resourceId: string, activationId?: string): SidebarShortcutItem => {
  const target = createSidebarShortcutTarget(providerId, resourceId, activationId)
  return { type: 'shortcut', id: createSidebarShortcutId(target), target }
}

describe('sidebar shortcut storage transforms', () => {
  it('preserves prototype-named future items without treating them as legacy resource types', () => {
    const future = ['constructor', 'toString', '__proto__'].map((type) => ({ type, id: `future-${type}` }))
    expect(normalizeSidebarShortcutItems(future)).toEqual(future)
    expect(getVisibleSidebarShortcutItems(future)).toEqual([])
  })
  it('migrates legacy leaves, deduplicates them, and preserves future top-level items', () => {
    const future = { type: 'group', id: 'future', children: ['x'] }
    const result = normalizeSidebarShortcutItems([
      { type: 'agent', id: 'agent-1', fallbackLabel: 'Researcher' },
      { type: 'agent', id: 'agent-1' },
      future,
      { type: 'shortcut', id: 'stale-id', target: shortcut('core.prompt', 'prompt-1').target }
    ])

    expect(result).toEqual([
      { ...shortcut('core.agent', 'agent-1'), fallbackLabel: 'Researcher' },
      future,
      shortcut('core.prompt', 'prompt-1')
    ])
    expect(getVisibleSidebarShortcutItems(result)).not.toContain(future)
  })

  it('allows different activations for one resource but rejects an exact duplicate', () => {
    const reveal = createSidebarShortcutTarget('core.prompt', 'prompt-1')
    const insert = createSidebarShortcutTarget('core.prompt', 'prompt-1', 'insert')
    const result = addSidebarShortcut(addSidebarShortcut([], reveal), insert)

    expect(result.map((item) => item.id)).toEqual([createSidebarShortcutId(reveal), createSidebarShortcutId(insert)])
    expect(addSidebarShortcut(result, reveal)).toEqual(result)
  })

  it('drops shortcuts for resources that are no longer exposed in the sidebar', () => {
    const agent = shortcut('core.agent', 'agent-1')

    expect(
      normalizeSidebarShortcutItems([
        shortcut('core.skill', 'skill-1'),
        shortcut('core.mcp-server', 'server-1'),
        shortcut('core.provider', 'provider-1'),
        shortcut('core.app', 'paintings'),
        shortcut('core.app', 'translate'),
        { type: 'app', id: 'paintings' },
        agent
      ])
    ).toEqual([agent])
  })

  it('removes the last built-in app and preserves other resource and future slots', () => {
    const assistant = shortcut('core.app', 'assistants')
    const agent = shortcut('core.agent', 'agent-1')
    const topic = shortcut('core.topic', 'topic-1')
    const future = { type: 'group', id: 'future' } as unknown as SidebarShortcutItem
    const stored = [agent, future, assistant, topic]

    expect(removeSidebarShortcut(stored, assistant.target)).toEqual([agent, future, topic])
    expect(reorderSidebarShortcuts(stored, [topic, assistant, agent])).toEqual([topic, future, assistant, agent])
    expect(reorderSidebarShortcuts([agent, assistant, topic], [topic, agent])).toEqual([topic, assistant, agent])
  })

  it('keeps a dragged ActivityBar order and still inserts a missing app', () => {
    const agents = shortcut('core.app', 'agents')
    const files = shortcut('core.app', 'files')
    const messages = shortcut('core.app', 'messages')
    const stored = markActivityBarOrderCustom([files, agents, messages])

    expect(activityBarOrderIsCustom(stored)).toBe(true)
    expect(getVisibleSidebarShortcutItems(stored).map((item) => item.target.locator.resourceId)).toEqual([
      'files',
      'agents',
      'messages',
      'knowledge',
      'org',
      'plugins',
      'platform'
    ])
  })

  it('uses the first built-in app shortcut as the startup destination', () => {
    const stored = [
      shortcut('core.mini-app', 'mini-1'),
      shortcut('core.app', 'paintings'),
      shortcut('core.app', 'agents')
    ]

    expect(getSidebarDefaultLandingUrl(stored, 'openai')).toBe('/app/agents')
    expect(getSidebarDefaultLandingUrl([shortcut('core.mini-app', 'mini-1')], 'openai')).toBe('')
  })

  it('shows 知识 directly under 工作, then 消息, and puts 插件 after 团队', () => {
    const agents = shortcut('core.app', 'agents')
    const assistants = shortcut('core.app', 'assistants')
    const knowledge = shortcut('core.app', 'knowledge')
    const messages = shortcut('core.app', 'messages')
    const plugins = shortcut('core.app', 'plugins')
    const capabilities = shortcut('core.app', 'capabilities')
    const stored = [agents, assistants, knowledge, plugins, capabilities]

    expect(normalizeSidebarShortcutItems(stored)).toEqual(stored)
    expect(getVisibleSidebarShortcutItems(stored)).toEqual([
      agents,
      knowledge,
      messages,
      shortcut('core.app', 'org'),
      plugins,
      shortcut('core.app', 'platform')
    ])
    expect(getSidebarIconLabelKey('schedule')).toBe('schedule.title')
    expect(zhCn['schedule.title']).toBe('日程')
    expect(getSidebarIconLabelKey('mail')).toBe('mail.title')
    expect(zhCn['mail.title']).toBe('邮件')
    expect(isTeamOnlyActivityApp('schedule')).toBe(true)
    expect(isTeamOnlyActivityApp('org')).toBe(true)
    expect(isTeamOnlyActivityApp('platform')).toBe(true)
    expect(isPlatformActivityApp('platform')).toBe(true)
    expect(isPlatformActivityApp('org')).toBe(false)
    expect(getSidebarIconLabelKey('platform')).toBe('platform.title')
    expect(isTeamOnlyActivityApp('mail')).toBe(false)
    expect(isTeamOnlyActivityApp('messages')).toBe(false)
    expect(getSidebarIconLabelKey('knowledge')).toBe('knowledge.notes.title')
    expect(zhCn['knowledge.notes.title']).toBe('知识')
    expect(getSidebarIconLabelKey('agents')).toBe('title.work')
    expect(zhCn['title.work']).toBe('工作')
    expect(getSidebarDefaultLandingUrl([assistants], 'openai')).toBe('')
    expect(temporarilyHiddenActivityBarReplacement('/app/chat?topicId=1')).toBe('/app/agents')
    expect(temporarilyHiddenActivityBarReplacement('/app/knowledge')).toBeNull()
    expect(temporarilyHiddenActivityBarReplacement('/app/knowledge/base-1')).toBeNull()
    expect(temporarilyHiddenActivityBarReplacement('/app/plugins')).toBeNull()
    expect(temporarilyHiddenActivityBarReplacement('/app/capabilities')).toBeNull()
    expect(temporarilyHiddenActivityBarReplacement('/app/documents')).toBeNull()
    expect(temporarilyHiddenActivityBarReplacement('/app/agents')).toBeNull()
    expect(getVisibleSidebarShortcutItems([agents, shortcut('core.app', 'documents')])).toEqual([
      agents,
      knowledge,
      messages,
      shortcut('core.app', 'org'),
      plugins,
      shortcut('core.app', 'platform')
    ])
  })

  it('removes mail and schedule from saved ActivityBar preferences', () => {
    const saved = [shortcut('core.app', 'agents'), shortcut('core.app', 'mail'), shortcut('core.app', 'schedule')]
    const visible = getVisibleSidebarShortcutItems(saved).map((item) => item.target.locator.resourceId)
    expect(visible).toContain('org')
    expect(visible).not.toContain('mail')
    expect(visible).not.toContain('schedule')
    expect(visible).toContain('knowledge')
  })

  it('keeps team mail and schedule off both rails while retaining plugins', () => {
    const agents = shortcut('core.app', 'agents')
    const team = getVisibleSidebarShortcutItems([agents])
    const ids = team.map((item) => item.target.locator.resourceId)
    expect(ids.indexOf('plugins')).toBe(ids.indexOf('org') + 1)
    expect(ids).not.toContain('mail')
    expect(ids).not.toContain('schedule')
    expect(ids).toContain('knowledge')
    expect(zhCn['org.title']).toBe('团队')
    const personal = team.filter((item) => !isTeamOnlyActivityApp(item.target.locator.resourceId))
    const personalIds = personal.map((item) => item.target.locator.resourceId)
    expect(personalIds).toContain('plugins')
    expect(personalIds).not.toContain('mail')
    expect(personalIds).not.toContain('schedule')
    expect(personalIds).not.toContain('org')
  })

  it('places 知识 on the ActivityBar directly under 工作', () => {
    const agents = shortcut('core.app', 'agents')
    const messages = shortcut('core.app', 'messages')
    const knowledge = shortcut('core.app', 'knowledge')
    const org = shortcut('core.app', 'org')
    const files = shortcut('core.app', 'files')

    expect(normalizeSidebarShortcutItems([agents, messages, org])).toEqual([agents, messages, org])
    const plugins = shortcut('core.app', 'plugins')
    expect(getVisibleSidebarShortcutItems([files, agents])).toEqual([
      files,
      agents,
      knowledge,
      messages,
      org,
      plugins,
      shortcut('core.app', 'platform')
    ])
    expect(getVisibleSidebarShortcutItems([agents, files, messages])).toEqual([
      agents,
      knowledge,
      messages,
      files,
      org,
      plugins,
      shortcut('core.app', 'platform')
    ])
    expect(getVisibleSidebarShortcutItems([agents, messages, org, files])).toEqual([
      agents,
      knowledge,
      messages,
      org,
      plugins,
      shortcut('core.app', 'platform'),
      files
    ])
    expect(getVisibleSidebarShortcutItems([files])).toEqual([files])
    expect(temporarilyHiddenActivityBarReplacement('/app/messages')).toBeNull()
  })

  it('hides documents from the ActivityBar and keeps 知识 under 工作', () => {
    const agents = shortcut('core.app', 'agents')
    const documents = shortcut('core.app', 'documents')
    const knowledge = shortcut('core.app', 'knowledge')
    const org = shortcut('core.app', 'org')
    const files = shortcut('core.app', 'files')

    const plugins = shortcut('core.app', 'plugins')
    const messages = shortcut('core.app', 'messages')
    expect(getVisibleSidebarShortcutItems([agents])).toEqual([
      agents,
      knowledge,
      messages,
      org,
      plugins,
      shortcut('core.app', 'platform')
    ])
    expect(getVisibleSidebarShortcutItems([agents, documents, files])).toEqual([
      agents,
      knowledge,
      messages,
      org,
      plugins,
      shortcut('core.app', 'platform'),
      files
    ])
    expect(getVisibleSidebarShortcutItems([agents, org, documents])).toEqual([
      agents,
      knowledge,
      messages,
      org,
      plugins,
      shortcut('core.app', 'platform')
    ])
    expect(getVisibleSidebarShortcutItems([agents, files])).toEqual([
      agents,
      knowledge,
      messages,
      org,
      plugins,
      shortcut('core.app', 'platform'),
      files
    ])
    expect(temporarilyHiddenActivityBarReplacement('/app/documents')).toBeNull()
  })
})
