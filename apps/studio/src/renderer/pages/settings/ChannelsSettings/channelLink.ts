/** Builtin ToBot session. A channel with no agent is this handler. */
export const TOBOT_CHANNEL_AGENT_ID = 'tobot'

export function channelServesToBot(agentId: string | null | undefined): boolean {
  if (agentId == null) return true
  const trimmed = agentId.trim()
  return trimmed.length === 0 || trimmed === TOBOT_CHANNEL_AGENT_ID
}

/** An empty allow list accepts every chat. */
export function chatAllowed(allow: unknown, chatId: string): boolean {
  if (!Array.isArray(allow) || allow.length === 0) return true
  return allow.some((item) => String(item) === chatId)
}

export function channelPrompt(type: string, chatId: string, text: string): string {
  return `[${type} ${chatId}] ${text}`
}

type ChannelMessage = {
  role?: unknown
  status?: unknown
  searchableText?: unknown
  data?: { parts?: unknown }
}

function textOfParts(parts: unknown): string {
  if (!Array.isArray(parts)) return ''
  const lines: string[] = []
  for (const part of parts) {
    if (!part || typeof part !== 'object') continue
    const record = part as { type?: unknown; text?: unknown }
    if (record.type === 'text' && typeof record.text === 'string' && record.text.trim()) lines.push(record.text)
  }
  return lines.join('\n')
}

function messageText(message: ChannelMessage): string {
  const fromParts = textOfParts(message.data?.parts)
  if (fromParts) return fromParts
  return typeof message.searchableText === 'string' ? message.searchableText : ''
}

/**
 * Newest-first session page.
 * `undefined` means the assistant line is not ready.
 * `null` means the turn failed or another user landed first, so nothing is sent.
 */
export function replyAfterPrompt(items: unknown, prompt: string): string | null | undefined {
  if (!Array.isArray(items)) return undefined
  const userIndex = items.findIndex((item) => {
    if (!item || typeof item !== 'object') return false
    const message = item as ChannelMessage
    return message.role === 'user' && message.searchableText === prompt
  })
  if (userIndex < 0) return undefined
  for (let index = userIndex - 1; index >= 0; index -= 1) {
    const item = items[index]
    if (!item || typeof item !== 'object') continue
    const message = item as ChannelMessage
    if (message.role === 'user') return null
    if (message.role !== 'assistant') continue
    if (message.status === 'error' || message.status === 'failed') return null
    if (message.status !== 'success') return undefined
    const text = messageText(message).trim()
    return text.length > 0 ? text : null
  }
  return undefined
}
