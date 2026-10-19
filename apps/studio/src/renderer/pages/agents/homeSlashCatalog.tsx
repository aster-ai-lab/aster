import type { TFunction } from 'i18next'
import { BookOpen, Sparkles, Terminal } from 'lucide-react'
import type { ReactNode } from 'react'
import useSWR from 'swr'

import type { ComposerTextHighlight } from '@renderer/components/composer/composerTextHighlight'
import type { QuickPanelInputAdapter, QuickPanelListItem } from '@renderer/components/QuickPanel'
import { prepareBuiltinSkill, type BuiltinSkillName } from '@renderer/pages/capabilities/capabilitiesData'
import { AGENT_RUNTIME_CAPABILITIES } from '@shared/ai/agentRuntimeCapabilities'
import { newConversationSlashCommands } from '@shared/ai/slashCommands'
import { readStudioRuntime } from '@shared/studioRuntime'

import { agentMentionHighlight } from './homeAgentCatalog'

export interface HomeLocalSkill {
  name: string
  description: string
  source: 'installed' | 'user'
}

const LOCAL_SKILLS_KEY = 'workbench.home.local-skills'
const EMPTY_SKILLS: readonly HomeLocalSkill[] = Object.freeze([])

function asSkill(value: unknown): HomeLocalSkill | null {
  if (!value || typeof value !== 'object') return null
  const record = value as Record<string, unknown>
  if (typeof record.name !== 'string' || !record.name) return null
  return {
    name: record.name,
    description: typeof record.description === 'string' ? record.description : '',
    source: record.source === 'installed' ? 'installed' : 'user'
  }
}

export async function fetchHomeLocalSkills(): Promise<HomeLocalSkill[]> {
  const runtime = readStudioRuntime()
  if (!runtime) return []
  const response = await fetch(`${runtime.apiBaseUrl.replace(/\/$/, '')}/api/local-skills`, {
    headers: { authorization: `Bearer ${runtime.token}` }
  })
  if (!response.ok) throw new Error(`local skills ${response.status}`)
  const body: unknown = await response.json()
  const skills = body && typeof body === 'object' ? (body as { skills?: unknown }).skills : undefined
  return Array.isArray(skills) ? skills.flatMap((item) => asSkill(item) ?? []) : []
}

export function useHomeLocalSkills(enabled: boolean): readonly HomeLocalSkill[] {
  const { data } = useSWR(enabled ? LOCAL_SKILLS_KEY : null, fetchHomeLocalSkills, {
    revalidateOnFocus: true,
    shouldRetryOnError: false
  })
  return data ?? EMPTY_SKILLS
}

export function skillPromptText(name: string) {
  return `使用技能 ${name}：`
}

// The home composer always starts a new task and has its own model picker.
const HOME_SLASH_COMMANDS = newConversationSlashCommands(AGENT_RUNTIME_CAPABILITIES['a3s-code'].slashCommands)

const escapeRegExp = (value: string) => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/** Chosen commands and skills read as distinct, recognisable spans in the home input. */
export function homeComposerTextHighlights(options?: {
  omitCommands?: readonly string[]
}): readonly ComposerTextHighlight[] {
  const omitted = new Set(options?.omitCommands ?? [])
  const commands = HOME_SLASH_COMMANDS.filter((command) => !omitted.has(command.command))
  // An empty alternation matches every position. Omit the rule instead.
  const commandRule: ComposerTextHighlight[] =
    commands.length === 0
      ? []
      : [
          {
            pattern: new RegExp(
              `(?<=^|\\s)(?:${commands.map((command) => escapeRegExp(command.command)).join('|')})(?=\\s|$)`,
              'g'
            ),
            className: 'composer-slash-chip',
            attributes: { 'data-composer-highlight': 'command' }
          }
        ]
  return [
    ...commandRule,
    {
      pattern: /使用技能 [^：\n]+：/g,
      className: 'composer-slash-chip',
      attributes: { 'data-composer-highlight': 'skill' }
    },
    agentMentionHighlight
  ]
}

export const HOME_COMPOSER_TEXT_HIGHLIGHTS: readonly ComposerTextHighlight[] = homeComposerTextHighlights()

const BUILTIN_SKILLS: readonly {
  name: BuiltinSkillName
  labelKey: string
  labelFallback: string
  hintKey: string
  hintFallback: string
  icon: 'sparkles' | 'book'
}[] = [
  {
    name: 'skill-creator',
    labelKey: 'capabilities.skill.menu.create',
    labelFallback: '创建技能',
    hintKey: 'capabilities.skill.builtin.create',
    hintFallback: '编写一个新的技能',
    icon: 'sparkles'
  },
  {
    name: 'schedule-task',
    labelKey: 'capabilities.skill.menu.schedule',
    labelFallback: '创建定时任务',
    hintKey: 'capabilities.skill.builtin.schedule',
    hintFallback: '确认名称、时间和要做的事，然后创建定时任务',
    icon: 'sparkles'
  },
  {
    name: 'stc-answer',
    labelKey: 'capabilities.skill.menu.understand',
    labelFallback: '知识理解',
    hintKey: 'capabilities.skill.builtin.understand',
    hintFallback: '用简明技术中文说明。复杂内容打开一页 HTML。',
    icon: 'book'
  }
]

function skillIcon(kind: 'sparkles' | 'book'): ReactNode {
  const Icon = kind === 'book' ? BookOpen : Sparkles
  return <Icon size={16} strokeWidth={1.7} />
}

function insertSkill(name: string, prepare: boolean, inputAdapter: QuickPanelInputAdapter | undefined) {
  if (prepare) {
    const builtin = name as BuiltinSkillName
    void prepareBuiltinSkill(builtin).catch(() => undefined)
  }
  inputAdapter?.insertText(`${skillPromptText(name)} `, { tokenizeVariables: false })
  inputAdapter?.focus()
}

export function homeSlashItems(
  t: TFunction,
  skills: readonly HomeLocalSkill[],
  options?: { includeLaunchers?: boolean; omitCommands?: readonly string[] }
): QuickPanelListItem[] {
  const includeLaunchers = options?.includeLaunchers !== false
  const omittedCommands = new Set(options?.omitCommands ?? [])
  const skillLabel = t('plugins.skills', '技能')
  const commandLabel = t('workbench.composer.slash.command', '指令')
  const commands = HOME_SLASH_COMMANDS.filter(
    (command) => !omittedCommands.has(command.command)
  ).map<QuickPanelListItem>((command) => {
    const name = command.command.slice(1)
    return {
      id: `workbench-slash:${command.command}`,
      label: command.command,
      description: t(`workbench.composer.slash.${name}`, command.description),
      filterText: command.command,
      searchAliases: [commandLabel],
      icon: <Terminal size={16} strokeWidth={1.7} />,
      suffix: commandLabel,
      action: ({ inputAdapter }) => {
        inputAdapter?.insertText(`${command.command} `, { tokenizeVariables: false })
        inputAdapter?.focus()
      }
    }
  })
  const builtinNames = new Set<string>(BUILTIN_SKILLS.map((skill) => skill.name))
  const builtins = includeLaunchers
    ? BUILTIN_SKILLS.map<QuickPanelListItem>((skill) => {
        const label = t(skill.labelKey, skill.labelFallback)
        return {
          id: `workbench-skill:${skill.name}`,
          label,
          description: t(skill.hintKey, skill.hintFallback),
          filterText: `${label} ${skill.name}`,
          searchAliases: [skillLabel, 'skill', skill.name],
          icon: skillIcon(skill.icon),
          suffix: skillLabel,
          action: ({ inputAdapter }) => insertSkill(skill.name, true, inputAdapter)
        }
      })
    : []
  const skillItems = skills
    .filter((skill) => !includeLaunchers || !builtinNames.has(skill.name))
    .map<QuickPanelListItem>((skill) => ({
      id: `workbench-skill:${skill.name}`,
      label: skill.name,
      description: skill.description,
      filterText: skill.name,
      searchAliases: [skillLabel, 'skill'],
      icon: skillIcon('sparkles'),
      suffix: skillLabel,
      action: ({ inputAdapter }) => insertSkill(skill.name, false, inputAdapter)
    }))
  return [...builtins, ...skillItems, ...commands]
}
