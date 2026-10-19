import { useEffect } from 'react'

import { useAgentChatRuntimeState } from '@renderer/pages/agents/useAgentChatRuntimeState'

const sentSessions = new Set<string>()
const inflight = new Map<string, Promise<boolean>>()
let sender: ((text: string) => Promise<boolean>) | null = null
let queued: { sessionId: string; text: string; resolve: (sent: boolean) => void } | null = null

/** The page registers this while it is mounted. Dialog close does not clear it. */
export function bindKnowledgeCompileSender(next: ((text: string) => Promise<boolean>) | null) {
  sender = next
  if (!next || !queued) return
  const turn = queued
  queued = null
  void deliver(turn)
}

/**
 * Send one compile instruction for this session.
 * A second call for the same session does nothing.
 * Without a sender yet, the call waits until the page binds one.
 */
export function dispatchKnowledgeCompileTurn(sessionId: string, text: string): Promise<boolean> {
  if (sentSessions.has(sessionId)) return Promise.resolve(true)
  const existing = inflight.get(sessionId)
  if (existing) return existing
  let resolveTurn: (sent: boolean) => void = () => {}
  const promise = new Promise<boolean>((resolve) => {
    resolveTurn = resolve
  })
  inflight.set(sessionId, promise)
  const turn = { sessionId, text, resolve: resolveTurn }
  if (sender) void deliver(turn)
  else queued = turn
  return promise
}

async function deliver(turn: { sessionId: string; text: string; resolve: (sent: boolean) => void }) {
  if (sentSessions.has(turn.sessionId)) {
    inflight.delete(turn.sessionId)
    turn.resolve(true)
    return
  }
  const current = sender
  if (!current) {
    queued = turn
    return
  }
  sentSessions.add(turn.sessionId)
  let sent = false
  try {
    sent = await current(turn.text)
  } catch {
    sent = false
  }
  if (!sent) sentSessions.delete(turn.sessionId)
  inflight.delete(turn.sessionId)
  turn.resolve(sent)
}

/** Binds the live session sender. Rendered by the knowledge page, not the dialog. */
export function KnowledgeCompileTurn({ sessionId }: { sessionId: string }) {
  const runtime = useAgentChatRuntimeState({
    sessionId,
    sessionMessagesEnabled: true,
    reservedMessages: []
  })
  useEffect(() => {
    bindKnowledgeCompileSender((text) => runtime.sendMessage({ text }))
    return () => bindKnowledgeCompileSender(null)
  }, [runtime.sendMessage])
  return null
}
