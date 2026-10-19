import type { AixAgentDeclaration, AixCloudBinding } from '@renderer/pages/agents/aixAgentRuntime'
import { DEFAULT_AGENT_AVATAR } from '@renderer/utils/agent'

import type { ComposerDraftToken } from './tokens'

export const AGENT_MENTION_TOKEN_PREFIX = 'agent-mention:'

export function agentMentionPhrase(name: string) {
  return `使用智能体 ${name}：`
}

/** Visible chip text. The full phrase stays in `promptText` for the model. */
export function agentMentionDisplayName(label: string) {
  const prefix = '使用智能体 '
  if (label.startsWith(prefix) && label.endsWith('：')) return label.slice(prefix.length, -1)
  return label
}

export function createAgentMentionToken(agent: {
  id: string
  name: string
  avatar: string
  prompt?: string
  parametersSchema?: string
  agentRuntime?: AixAgentDeclaration
  cloudBinding?: AixCloudBinding
  conversationId?: string
  modelAlias?: string
}): ComposerDraftToken {
  const phrase = agentMentionPhrase(agent.name)
  const instructions = agent.prompt?.trim()
  const payload = {
    ...(agent.parametersSchema ? { parametersSchema: agent.parametersSchema } : {}),
    ...(agent.agentRuntime ? { agentRuntime: agent.agentRuntime } : {}),
    ...(agent.cloudBinding ? { cloudBinding: agent.cloudBinding } : {}),
    ...(agent.conversationId ? { conversationId: agent.conversationId } : {}),
    ...(agent.modelAlias ? { modelAlias: agent.modelAlias } : {})
  }
  return {
    id: `${AGENT_MENTION_TOKEN_PREFIX}${agent.id}:${crypto.randomUUID()}`,
    kind: 'reference',
    label: phrase,
    icon: agent.avatar || DEFAULT_AGENT_AVATAR,
    promptText: instructions ? `${phrase}\n${instructions}` : phrase,
    ...(Object.keys(payload).length > 0 ? { payload } : {})
  }
}

export function isAgentMentionToken(token: { id: string; kind: string }) {
  return token.kind === 'reference' && token.id.startsWith(AGENT_MENTION_TOKEN_PREFIX)
}

export function agentMentionAvatar(token: { icon?: string }) {
  const avatar = token.icon?.trim()
  return avatar || DEFAULT_AGENT_AVATAR
}
