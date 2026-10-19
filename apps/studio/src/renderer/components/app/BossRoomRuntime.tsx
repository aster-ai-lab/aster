import { useEffect, useSyncExternalStore } from 'react'
import { useTranslation } from 'react-i18next'

import { dataApiService } from '@data/DataApiService'
import type { ImMessage } from '@renderer/im/imApi'
import { BUILTIN_TOBOSS_ID, sharedImStore } from '@renderer/im/imStore'
import { conversationTitle, personName } from '@renderer/im/imView'
import { encodeBossWire } from '@renderer/im/quote'
import {
  TOBOSS_PROFILE_KEY,
  tobotDisplayName,
  tobotHostContext,
  useBotProfile,
  type ToBotHostContext
} from '@renderer/im/tobotProfile'
import {
  acceptBossCatchUp,
  BOSS_ROOM_DIRECTIVE,
  bossRoomSessionId,
  BOSS_WARM_CAP,
  bossJobRunner,
  bossRoomNeedsWarm,
  currentBossSession,
  teamBotMentionLabels,
  finishSharedBossJob,
  noteBossSession,
  readBossGroups,
  writeBossGroups,
  type BossJob,
  type BossRoomMessage
} from '@renderer/services/bossRoom'
import { withHarnessComponents } from '@renderer/services/harnessAssemble'
import { sessionIdFromEnsure } from '@renderer/services/tobotStream'

function wait(ms: number): Promise<void> {
  return new Promise((resolve) => globalThis.window.setTimeout(resolve, ms))
}

function planeRejected(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error)
  return message === 'Load failed' || message === 'Failed to fetch' || message.includes('already running')
}

type AgentReply = { status: string; text: string }

/** Oldest-first lines. A pending assistant is not an answer yet. */
function answerAfter(page: unknown, prompt: string): AgentReply | null {
  const items =
    page && typeof page === 'object' && Array.isArray((page as { items?: unknown }).items)
      ? (page as { items: unknown[] }).items
      : []
  const lines: Array<{ role: 'user' | 'assistant'; text: string; status: string }> = []
  for (const item of items) {
    if (!item || typeof item !== 'object') continue
    const record = item as { role?: unknown; status?: unknown; searchableText?: unknown; data?: { parts?: unknown } }
    const role = record.role === 'assistant' ? 'assistant' : record.role === 'user' ? 'user' : null
    if (!role) continue
    const parts = Array.isArray(record.data?.parts) ? record.data.parts : []
    const fromParts = parts
      .map((part) => {
        if (!part || typeof part !== 'object') return ''
        const typed = part as { type?: unknown; text?: unknown }
        return typed.type === 'text' && typeof typed.text === 'string' ? typed.text : ''
      })
      .filter(Boolean)
      .join('\n')
    lines.push({
      role,
      text: fromParts || (typeof record.searchableText === 'string' ? record.searchableText : ''),
      status: typeof record.status === 'string' ? record.status : 'success'
    })
  }
  lines.reverse()
  let userAt = -1
  lines.forEach((line, index) => {
    if (line.role === 'user' && line.text === prompt) userAt = index
  })
  if (userAt < 0) return null
  return lines.slice(userAt + 1).find((line) => line.role === 'assistant' && line.status !== 'pending') ?? null
}

async function bossTurnRunning(sessionId: string): Promise<boolean> {
  try {
    const body = await dataApiService.get(`/agent-sessions/${sessionId}/turn`)
    return 'status' in body && body.status === 'attached'
  } catch {
    return false
  }
}

async function postBossTurn(
  conversationId: string,
  prompt: string,
  host: ToBotHostContext | null,
  orgId: string
): Promise<string> {
  const deadline = Date.now() + 90_000
  let lastError: unknown = new Error('Load failed')
  const body = await withHarnessComponents({
    userMessageParts: [{ type: 'text', text: prompt }],
    orgId,
    ...(host ? { hostContext: host } : {}),
    turnContext: { directives: [BOSS_ROOM_DIRECTIVE] }
  })
  while (Date.now() <= deadline) {
    try {
      const ready = await dataApiService.post('/agent-sessions/toboss/ensure', {
        body: await withHarnessComponents(host ? { hostContext: host } : {})
      })
      const id = sessionIdFromEnsure(ready)
      if (id) noteBossSession(id)
      const sessionId = bossRoomSessionId(currentBossSession(), conversationId)
      if (!sessionId) throw new Error('Load failed')
      await dataApiService.post('/agent-sessions/toboss/room/ensure', {
        body: await withHarnessComponents({
          conversationId,
          ...(host ? { hostContext: host } : {})
        })
      })
      await dataApiService.post(`/agent-sessions/${sessionId}/turns`, { body })
      return sessionId
    } catch (error) {
      lastError = error
      if (!planeRejected(error) || Date.now() > deadline) break
      await wait(400)
    }
  }
  throw lastError
}

async function waitBossAnswer(sessionId: string, prompt: string): Promise<string | undefined> {
  const deadline = Date.now() + 180_000
  let idle = 0
  for (;;) {
    const page = await dataApiService
      .get(`/agent-sessions/${sessionId}/messages`, { query: { limit: 50 } })
      .catch(() => null)
    const reply = page ? answerAfter(page, prompt) : null
    if (reply?.status === 'success' && reply.text.trim()) return reply.text
    if (reply && reply.status !== 'pending') return undefined
    if (Date.now() > deadline) return undefined
    idle = (await bossTurnRunning(sessionId)) ? 0 : idle + 1
    if (idle >= 2) return undefined
    await wait(400)
  }
}

async function runUnattendedBossJob(
  job: BossJob,
  host: ToBotHostContext | null,
  orgId: string,
  send: (conversationId: string, text: string) => Promise<void>
): Promise<void> {
  let answered = false
  try {
    const sessionId = await postBossTurn(job.conversationId, job.prompt, host, orgId)
    const answer = (await waitBossAnswer(sessionId, job.prompt))?.trim() ?? ''
    if (!answer) return
    await send(job.conversationId, encodeBossWire(answer, 'reply'))
    answered = true
  } catch {
    answered = false
  } finally {
    const next = finishSharedBossJob(Date.now(), answered)
    if (next) {
      const runner = bossJobRunner()
      if (runner) runner(next)
      else void runUnattendedBossJob(next, host, orgId, send)
    }
  }
}

function roomMessages(
  messages: readonly ImMessage[],
  members: readonly { userId: string; left: boolean; nickname?: string | null }[],
  names: (userId: string) => string,
  systemName: string
): BossRoomMessage[] {
  return messages.map((message) => {
    const senderId = message.sender.kind === 'member' ? message.sender.id : ''
    const nickname = members.find((member) => member.userId === senderId && !member.left)?.nickname?.trim()
    return {
      id: message.id,
      createdAt: message.createdAt,
      recalled: message.recalled,
      senderId,
      senderName: message.sender.kind === 'service' ? systemName : nickname || names(senderId),
      text: message.body.kind === 'text' ? message.body.text : null
    }
  })
}

/** Group @ lane for the signed-in creator. It stays up when the messages page is closed. */
export function BossRoomRuntime() {
  const { t } = useTranslation()
  const store = sharedImStore()
  const snapshot = useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot)
  const profile = useBotProfile(TOBOSS_PROFILE_KEY)
  const bossLabel = tobotDisplayName(profile, t('messages.toboss'))
  const bossListed = snapshot.conversations.some((entry) => entry.id === BUILTIN_TOBOSS_ID)
  const groups = readBossGroups(snapshot.orgId, currentBossSession())
  const labels = teamBotMentionLabels(bossLabel, t('messages.toboss'))
  const warmKey = snapshot.conversations
    .filter((entry) =>
      bossRoomNeedsWarm(
        {
          kind: entry.kind,
          previewText: entry.preview && !entry.preview.recalled ? entry.preview.text : null
        },
        Boolean(snapshot.threads[entry.id]?.loaded),
        groups,
        labels,
        entry.id
      )
    )
    .slice(0, BOSS_WARM_CAP)
    .map((entry) => entry.id)
    .join('\n')

  useEffect(() => {
    if (!bossListed || snapshot.status !== 'ready' || !warmKey) return
    const ids = warmKey.split('\n')
    let cancelled = false
    void (async () => {
      for (const id of ids) {
        if (cancelled) return
        await store.preload(id).catch(() => undefined)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [bossListed, snapshot.status, store, warmKey])

  useEffect(() => {
    if (!bossListed || snapshot.status !== 'ready' || !snapshot.orgId) return
    const orgId = snapshot.orgId
    const names = teamBotMentionLabels(bossLabel, t('messages.toboss'))
    const rooms = snapshot.conversations
      .filter((entry) => entry.kind === 'group' && snapshot.threads[entry.id]?.loaded)
      .map((entry) => ({
        id: entry.id,
        title: conversationTitle(entry, snapshot, t),
        messages: roomMessages(
          snapshot.threads[entry.id]?.messages ?? [],
          snapshot.details[entry.id]?.members ?? [],
          (userId) => personName(snapshot, userId),
          t('messages.system_sender')
        )
      }))
    const plan = acceptBossCatchUp({
      now: Date.now(),
      labels: names,
      bossName: bossLabel,
      bossListed,
      memberGroupIds: readBossGroups(orgId, currentBossSession()),
      rooms
    })
    let ids = readBossGroups(orgId, currentBossSession())
    let changed = false
    for (const item of plan.membership) {
      if (ids.includes(item.id) === item.present) continue
      ids = item.present ? [...ids, item.id] : ids.filter((id) => id !== item.id)
      changed = true
    }
    if (changed) writeBossGroups(orgId, currentBossSession(), ids)
    const runner = bossJobRunner()
    const host = tobotHostContext(profile, t('messages.toboss'))
    for (const job of plan.started) {
      if (runner) runner(job)
      else void runUnattendedBossJob(job, host, orgId, (conversationId, text) => store.sendText(conversationId, text))
    }
  }, [bossLabel, bossListed, profile, snapshot, store, t])

  return null
}
