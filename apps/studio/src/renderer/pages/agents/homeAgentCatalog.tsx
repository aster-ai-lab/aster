import { projectAgentParameterSchema } from '@a3s-lab/intelligent-ui/openui'
import type { Editor } from '@tiptap/core'
import type { TFunction } from 'i18next'

import { AgentAvatar } from '@renderer/components/Avatar/AgentAvatar'
import { agentAvatarElement } from '@renderer/components/Avatar/agentAvatarDom'
import { agentMentionPhrase, createAgentMentionToken } from '@renderer/components/composer/agentMentionToken'
import { setAgentParameterDraft } from '@renderer/components/composer/agentParameterDraft'
import type { ComposerTextHighlight } from '@renderer/components/composer/composerTextHighlight'
import type { ComposerSuggestionItem } from '@renderer/components/composer/quickPanel'
import {
  aixDeclarationFromUnknown,
  type AixAgentDeclaration,
  type AixCloudBinding
} from '@renderer/pages/agents/aixAgentRuntime'
import { officialAgentAvatar } from '@renderer/pages/agents/officialAgentAvatars'
import { DEFAULT_AGENT_AVATAR, getAgentAvatar } from '@renderer/utils/agent'

export interface HomeInstalledAgent {
  id: string
  name: string
  description: string
  prompt?: string
  avatar?: string
  /** Closed call schema from an installed package. */
  parametersSchema?: string
  /** Edge, cloud, or remote run declared by the installed package. */
  agentRuntime?: AixAgentDeclaration
  /** Cloud asset binding for cloud-runtime agents. */
  cloudBinding?: AixCloudBinding
  /** Cloud conversation id for cloud-runtime agents. */
  conversationId?: string
  /** Inference route model alias the Cloud agent uses for this run. */
  modelAlias?: string
}

/**
 * Market catalog rows with blank names removed.
 * The `@` panel does not use this list. It uses {@link installedMentionAgents}.
 */
export function marketCatalogAgents(
  presets: readonly {
    id: string
    name: string
    description?: string
    emoji?: string
    prompt?: string
    avatar?: string
  }[]
): HomeInstalledAgent[] {
  const seen = new Set<string>()
  const agents: HomeInstalledAgent[] = []
  for (const preset of presets) {
    const name = preset.name.trim()
    if (!name || seen.has(name)) continue
    seen.add(name)
    const avatar = preset.avatar?.trim() || preset.emoji?.trim() || ''
    const prompt = preset.prompt?.trim() ?? ''
    agents.push({
      id: preset.id,
      name,
      description: (preset.description || '').trim(),
      prompt: prompt || undefined,
      avatar: avatar || undefined
    })
  }
  return agents
}

function displayAgentAvatar(agent: { id: string; name: string; avatar?: string }) {
  const stored = agent.avatar?.trim()
  if (stored) return stored
  return officialAgentAvatar(agent.id) || officialAgentAvatar(agent.name) || getAgentAvatar(agent.avatar)
}

function mentionAvatar(
  preset: { id: string; name: string; avatar?: string; emoji?: string; packageSlug?: string },
  local?: MentionAssistant
) {
  const stored = preset.avatar?.trim() || preset.emoji?.trim() || local?.emoji?.trim() || ''
  if (stored) return stored
  return (
    officialAgentAvatar(preset.packageSlug) || officialAgentAvatar(preset.id) || officialAgentAvatar(preset.name) || ''
  )
}

export interface MentionAssistant {
  id: string
  name: string
  description?: string | null
  prompt?: string | null
  emoji?: string | null
}

/**
 * Agents the member has installed. A catalog row counts only when a saved
 * assistant has the same name, or when its package is installed. A saved
 * assistant that is not in the catalog is included on its own.
 */
export function installedMentionAgents(
  presets: readonly {
    id: string
    name: string
    description?: string
    emoji?: string
    prompt?: string
    avatar?: string
    packageSlug?: string
    installed?: boolean
    parametersSchema?: string
    agentRuntime?: AixAgentDeclaration
  }[],
  assistants: readonly MentionAssistant[]
): HomeInstalledAgent[] {
  const installedByName = new Map<string, MentionAssistant>()
  for (const assistant of assistants) {
    const name = assistant.name.trim()
    if (!name || installedByName.has(name)) continue
    installedByName.set(name, assistant)
  }
  const presetNames = new Set<string>()
  const seen = new Set<string>()
  const agents: HomeInstalledAgent[] = []
  for (const preset of presets) {
    const name = preset.name.trim()
    if (!name) continue
    presetNames.add(name)
    const installed = preset.packageSlug ? preset.installed === true : installedByName.has(name)
    if (!installed || seen.has(name)) continue
    if (preset.packageSlug && !aixDeclarationFromUnknown(preset.agentRuntime)) continue
    seen.add(name)
    const local = installedByName.get(name)
    const avatar = mentionAvatar(preset, local)
    const prompt = preset.prompt?.trim() || local?.prompt?.trim() || ''
    const parametersSchema = mentionSchema(preset.parametersSchema)
    const agentRuntime = aixDeclarationFromUnknown(preset.agentRuntime) ?? undefined
    agents.push({
      id: preset.packageSlug ? preset.id : local?.id || preset.id,
      name,
      description: (preset.description || local?.description || '').trim(),
      prompt: prompt || undefined,
      avatar: avatar || undefined,
      ...(parametersSchema ? { parametersSchema } : {}),
      ...(agentRuntime ? { agentRuntime } : {})
    })
  }
  for (const [name, assistant] of installedByName) {
    if (presetNames.has(name) || seen.has(name)) continue
    const prompt = assistant.prompt?.trim() ?? ''
    const avatar = assistant.emoji?.trim() || officialAgentAvatar(assistant.id) || officialAgentAvatar(name) || ''
    agents.push({
      id: assistant.id,
      name,
      description: (assistant.description || '').trim(),
      prompt: prompt || undefined,
      avatar: avatar || undefined
    })
  }
  return agents
}

const agentAvatars = new Map<string, string>()

/** Names the `@` list already showed, so the composer chip can paint that avatar. */
export function rememberAgentAvatars(agents: readonly Pick<HomeInstalledAgent, 'name' | 'avatar'>[]) {
  agentAvatars.clear()
  for (const agent of agents) {
    const name = agent.name.trim()
    if (!name) continue
    agentAvatars.set(name, getAgentAvatar(agent.avatar))
  }
}

export function agentAvatarForName(name: string) {
  return agentAvatars.get(name.trim()) || DEFAULT_AGENT_AVATAR
}

export function agentPromptText(name: string) {
  return agentMentionPhrase(name)
}

const AGENT_CHIP_STYLE =
  'display:inline;margin:0 2px;padding:1px 4px;border:1px solid #e5e5e5;border-radius:6px;background:#f5f5f5;color:#171717;font-size:13px;font-weight:500;line-height:18px;vertical-align:baseline;box-decoration-break:clone;-webkit-box-decoration-break:clone;'

function agentNameFromPhrase(text: string) {
  return text.slice('使用智能体 '.length, -1)
}

function agentAvatarWidget(text: string) {
  const name = agentNameFromPhrase(text)
  const avatar = agentAvatarForName(name)
  const mark = agentAvatarElement(avatar, name)
  mark.setAttribute('data-agent-avatar', avatar)
  return mark
}

export const agentMentionHighlight: ComposerTextHighlight = {
  pattern: /使用智能体 [^：\n]+：/g,
  className: 'composer-slash-chip',
  attributes: {
    'data-composer-highlight': 'agent',
    style: AGENT_CHIP_STYLE
  },
  attributesForMatch: (text) => ({
    'data-agent-avatar': agentAvatarForName(agentNameFromPhrase(text))
  }),
  leadingWidget: agentAvatarWidget
}

function mentionSchema(schema: string | undefined) {
  if (!schema || !projectAgentParameterSchema(schema).ok) return undefined
  return schema
}

function insertAgent(agent: Omit<HomeInstalledAgent, 'description'>, editor: Editor) {
  const avatar = getAgentAvatar(agent.avatar)
  const parametersSchema = mentionSchema(agent.parametersSchema)
  setAgentParameterDraft(agent.id, parametersSchema)
  editor
    .chain()
    .focus()
    .insertComposerToken(
      createAgentMentionToken({
        id: agent.id,
        name: agent.name,
        avatar,
        prompt: agent.prompt,
        parametersSchema,
        agentRuntime: agent.agentRuntime,
        ...(agent.cloudBinding ? { cloudBinding: agent.cloudBinding } : {}),
        ...(agent.conversationId ? { conversationId: agent.conversationId } : {}),
        ...(agent.modelAlias ? { modelAlias: agent.modelAlias } : {})
      })
    )
    .insertContent(' ')
    .run()
}

/** Installed agents for the `@` panel. Rows match the `/` skill list: icon, name, description, suffix. */
export function homeAgentSuggestionItems(
  t: TFunction,
  agents: readonly HomeInstalledAgent[],
  query: string
): ComposerSuggestionItem[] {
  rememberAgentAvatars(agents)
  const suffix = t('workbench.composer.agents_title', '智能体')
  const needle = query.trim().toLowerCase()
  const matches = agents
    .flatMap((agent) => {
      const name = agent.name.trim()
      if (!name) return []
      const description = agent.description.trim()
      if (needle && !`${name}\n${description}`.toLowerCase().includes(needle)) return []
      return [{ ...agent, name, description }]
    })
    .sort((left, right) => left.name.localeCompare(right.name, 'zh') || left.id.localeCompare(right.id))

  if (matches.length === 0) {
    return [
      {
        id: 'workbench-agent:empty',
        label: t('workbench.composer.agents_empty', '没有已安装的智能体'),
        disabled: true,
        command: () => undefined
      }
    ]
  }

  return matches.map((agent) => {
    const avatar = displayAgentAvatar(agent)
    return {
      id: `workbench-agent:${agent.id}`,
      label: agent.name,
      description: agent.description,
      filterText: `${agent.name} ${agent.id}`,
      searchAliases: [suffix, 'agent', agent.name],
      icon: (
        <span data-agent-avatar={avatar} className="inline-flex size-4 items-center justify-center">
          <AgentAvatar avatar={avatar} name={agent.name} size={16} glyphStyle="plain" />
        </span>
      ),
      suffix,
      command: ({ editor }) => insertAgent({ ...agent, avatar }, editor)
    }
  })
}
