import { loggerService } from '@logger'
import { openRoute } from '@renderer/services/mainWindowNavigation'

const logger = loggerService.withContext('MessageAttention')

export const OPEN_MESSAGE_EVENT = 'studio:open-message'
const HOST_EVENT = 'message-attention-open'

export type MessageFocus = { selectedId: string | null; lookedAtId: string | null }

type Invoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>
type Internals = {
  invoke?: Invoke
  transformCallback?: (callback: (event: { payload?: { conversationId?: string | null } }) => void) => number
}

let focus: MessageFocus = { selectedId: null, lookedAtId: null }
const focusListeners = new Set<() => void>()
let pendingId: string | null = null
let sentCount = -1

export function messageFocus(): MessageFocus {
  return focus
}

export function subscribeMessageFocus(listener: () => void): () => void {
  focusListeners.add(listener)
  return () => {
    focusListeners.delete(listener)
  }
}

export function setMessageFocus(next: MessageFocus): void {
  if (next.selectedId === focus.selectedId && next.lookedAtId === focus.lookedAtId) return
  focus = next
  for (const listener of focusListeners) listener()
}

/** The messages page may not be mounted when a banner or the status item is clicked. */
export function requestOpenMessage(id: string): void {
  pendingId = id
  window.dispatchEvent(new CustomEvent<{ id: string }>(OPEN_MESSAGE_EVENT, { detail: { id } }))
}

export function consumePendingMessage(): string | null {
  const id = pendingId
  pendingId = null
  return id
}

function internals(): Internals | undefined {
  return (globalThis as { __TAURI_INTERNALS__?: Internals }).__TAURI_INTERNALS__
}

async function invoke(command: string, args: Record<string, unknown>): Promise<boolean> {
  const call = internals()?.invoke
  if (!call) return true
  try {
    await call(command, args)
    return true
  } catch (error) {
    logger.warn(`Message attention command failed: ${command}`, error as Error)
    return false
  }
}

export function setMessageAttention(count: number): void {
  if (sentCount === count) return
  sentCount = count
  void invoke('set_message_attention', { count }).then((ok) => {
    if (!ok) sentCount = -1
  })
}

export function publishMessageNotification(id: string, title: string, body: string): void {
  void invoke('publish_message_notification', { id, title, body })
}

export function withdrawMessageNotification(id: string): void {
  void invoke('withdraw_message_notification', { id })
}

export function listenMessageAttention(onOpen: (conversationId: string | null) => void): () => void {
  const bridge = internals()
  if (!bridge?.invoke || !bridge.transformCallback) return () => undefined
  let stopped = false
  let eventId: number | null = null
  const handler = bridge.transformCallback((event) => {
    const id = event?.payload?.conversationId
    onOpen(typeof id === 'string' && id ? id : null)
  })
  void bridge.invoke('plugin:event|listen', { event: HOST_EVENT, target: { kind: 'Any' }, handler }).then((id) => {
    if (typeof id !== 'number') return
    if (stopped) {
      void bridge.invoke?.('plugin:event|unlisten', { event: HOST_EVENT, eventId: id })
      return
    }
    eventId = id
  })
  return () => {
    stopped = true
    if (eventId != null) void bridge.invoke?.('plugin:event|unlisten', { event: HOST_EVENT, eventId })
  }
}

export function openMessageFromAttention(conversationId: string | null): void {
  openRoute('/app/messages')
  if (conversationId) requestOpenMessage(conversationId)
}
