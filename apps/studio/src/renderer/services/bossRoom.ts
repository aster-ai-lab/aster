/** One team ToBot lane runs one turn. Each group has its own session, apart from the private chat. */

/** The team bot is named ToBot. Messages that still say @ToBoss address the same bot. */
const LEGACY_TEAM_BOT_LABEL = 'ToBoss'

export function teamBotMentionLabels(displayName: string, canonical: string): string[] {
  const labels = displayName === canonical ? [displayName] : [displayName, canonical]
  if (!labels.includes(LEGACY_TEAM_BOT_LABEL)) labels.push(LEGACY_TEAM_BOT_LABEL)
  return labels
}

import { decodeBossWire, visibleText } from '@renderer/im/quote'
import { textMentionsNamedBot } from '@renderer/utils/messageMentions'

export const BOSS_QUEUE_CAP = 3
export const BOSS_GAP_MS = 10_000
export const BOSS_CATCHUP_MS = 24 * 60 * 60 * 1000
export const BOSS_ROOM_DIRECTIVE =
  'This turn is a group. The user message is the transcript and the ask. Answer the ask from the transcript. Do not continue a private reply.'

/** Marks a user line that carries a group transcript. The private thread omits that turn. */
export const BOSS_ROOM_PROMPT_NOTE = 'Answer from this transcript. Do not continue an earlier private reply.'

const BOSS_LINE_CAP = 24
const BOSS_LINE_CHARS = 180
const BOSS_TRANSCRIPT_CHARS = 3500
const BOSS_ASK_CHARS = 500
const BOSS_DONE_CAP = 20

export type BossRoomMessage = {
  id: string
  createdAt: number
  recalled: boolean
  senderId: string
  senderName: string
  text: string | null
}

export type BossJob = {
  key: string
  conversationId: string
  senderId: string
  ask: string
  prompt: string
  files?: globalThis.File[]
}

export type BossDecision = 'start' | 'queued' | 'replaced' | 'soon' | 'full' | 'duplicate'

type BossDone = {
  conversationId: string
  senderId: string
  ask: string
  at: number
}

export type BossLane = {
  running: BossJob | null
  queue: BossJob[]
  acceptedAt: Record<string, number>
  done: BossDone[]
}

export function emptyBossLane(): BossLane {
  return { running: null, queue: [], acceptedAt: {}, done: [] }
}

function senderKey(conversationId: string, senderId: string): string {
  return `${conversationId}:${senderId}`
}

function sameAsk(job: BossJob, conversationId: string, senderId: string, ask: string): boolean {
  return job.conversationId === conversationId && job.senderId === senderId && job.ask === ask
}

/** One running turn, one waiting slot per sender, a short gap, and a queue of three. */
export function offerBossJob(lane: BossLane, job: BossJob, now: number): { lane: BossLane; decision: BossDecision } {
  const occupied = [lane.running, ...lane.queue].filter((item): item is BossJob => item !== null)
  if (occupied.some((item) => sameAsk(item, job.conversationId, job.senderId, job.ask))) {
    return { lane, decision: 'duplicate' }
  }
  const queuedAt = lane.queue.findIndex(
    (item) => item.conversationId === job.conversationId && item.senderId === job.senderId
  )
  if (queuedAt >= 0) {
    const queue = lane.queue.slice()
    queue[queuedAt] = job
    return { lane: { ...lane, queue }, decision: 'replaced' }
  }
  const key = senderKey(job.conversationId, job.senderId)
  const accepted = lane.acceptedAt[key]
  if (accepted !== undefined && now - accepted < BOSS_GAP_MS) return { lane, decision: 'soon' }
  if (!lane.running) {
    return {
      lane: { ...lane, running: job, acceptedAt: { ...lane.acceptedAt, [key]: now } },
      decision: 'start'
    }
  }
  if (lane.queue.length >= BOSS_QUEUE_CAP) return { lane, decision: 'full' }
  return {
    lane: { ...lane, queue: [...lane.queue, job], acceptedAt: { ...lane.acceptedAt, [key]: now } },
    decision: 'queued'
  }
}

export function completeBossJob(lane: BossLane, now: number, recorded = true): { lane: BossLane; job: BossJob | null } {
  const finished = lane.running
  const done =
    finished && recorded
      ? [
          ...lane.done,
          { conversationId: finished.conversationId, senderId: finished.senderId, ask: finished.ask, at: now }
        ].slice(-BOSS_DONE_CAP)
      : lane.done
  const next = lane.queue[0] ?? null
  return { lane: { ...lane, running: next, queue: lane.queue.slice(1), done }, job: next }
}

/** A finished ask is settled until a later message repeats it. */
export function bossAskSettled(
  lane: BossLane,
  conversationId: string,
  senderId: string,
  ask: string,
  createdAt: number
): boolean {
  return lane.done.some(
    (item) =>
      item.conversationId === conversationId && item.senderId === senderId && item.ask === ask && createdAt <= item.at
  )
}

/** The latest join or leave wins. No mark keeps the local memory. */
export function bossMembership(messages: readonly BossRoomMessage[]): 'in' | 'out' | null {
  let state: 'in' | 'out' | null = null
  for (const message of messages) {
    if (message.recalled || !message.text) continue
    const wire = decodeBossWire(message.text)
    if (wire?.kind === 'join') state = 'in'
    else if (wire?.kind === 'leave') state = 'out'
  }
  return state
}

function oneLine(value: string, max = BOSS_LINE_CHARS): string {
  const flat = value.replace(/\s+/g, ' ').trim()
  const chars = [...flat]
  return chars.length <= max ? flat : chars.slice(0, max).join('')
}

/** Speakers and text, newest last. Join, leave, and recalled rows stay out. */
export function bossTranscript(
  messages: readonly BossRoomMessage[],
  bossName: string
): Array<{ speaker: string; text: string }> {
  const lines: Array<{ speaker: string; text: string }> = []
  for (const message of messages) {
    if (message.recalled || !message.text) continue
    const wire = decodeBossWire(message.text)
    if (wire?.kind === 'join' || wire?.kind === 'leave') continue
    const text = oneLine(wire ? wire.text : visibleText(message.text))
    if (!text) continue
    lines.push({ speaker: wire?.kind === 'reply' ? bossName : message.senderName, text })
  }
  return lines
}

/** The room transcript, then the ask. Older lines drop first. */
export function bossRoomPrompt(input: {
  room: string
  asker: string
  ask: string
  lines: readonly { speaker: string; text: string }[]
}): string {
  const kept: Array<{ speaker: string; text: string }> = []
  let total = 0
  for (let index = input.lines.length - 1; index >= 0 && kept.length < BOSS_LINE_CAP; index -= 1) {
    const text = oneLine(input.lines[index]?.text ?? '')
    const speaker = oneLine(input.lines[index]?.speaker ?? '', 40)
    if (!text || !speaker) continue
    const row = `${speaker}: ${text}`
    if (kept.length > 0 && total + row.length > BOSS_TRANSCRIPT_CHARS) break
    kept.push({ speaker, text })
    total += row.length + 1
  }
  kept.reverse()
  const body = kept.map((line) => `${line.speaker}: ${line.text}`).join('\n')
  const ask = oneLine(input.ask, BOSS_ASK_CHARS)
  const parts = [`Group: ${oneLine(input.room, 80)}`, BOSS_ROOM_PROMPT_NOTE]
  if (body) parts.push('', body)
  parts.push('', `${oneLine(input.asker, 40)} asked: ${ask}`)
  return parts.join('\n')
}

/** A group job posted into the shared ToBoss session. Private chat does not show it. */
export function isBossRoomPrompt(text: string): boolean {
  const trimmed = text.trimStart()
  return trimmed.startsWith('Group:') && trimmed.includes(BOSS_ROOM_PROMPT_NOTE)
}

/** Drop each group prompt and the assistant lines that answer it. */
export function omitBossRoomTurns<T extends { role: string; text: string }>(lines: readonly T[]): T[] {
  const hidden = hiddenBossRoomIndexes(lines)
  if (hidden.size === 0) return lines as T[]
  return lines.filter((_, index) => !hidden.has(index))
}

/** The live anchor belongs to a group turn, including one whose assistant row has not arrived. */
export function isBossRoomTurnAnchor(
  lines: readonly { id: string; role: string; text: string }[],
  anchorId: string
): boolean {
  const hidden = hiddenBossRoomIndexes(lines)
  for (let index = 0; index < lines.length; index += 1) {
    if (hidden.has(index) && lines[index]?.id === anchorId) return true
  }
  if (lines.some((line) => line.id === anchorId)) return false
  const lastUser = [...lines].reverse().find((line) => line.role === 'user')
  return Boolean(lastUser && isBossRoomPrompt(lastUser.text))
}

function hiddenBossRoomIndexes(lines: readonly { role: string; text: string }[]): Set<number> {
  const hidden = new Set<number>()
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index]
    if (!line || line.role !== 'user' || !isBossRoomPrompt(line.text)) continue
    hidden.add(index)
    for (let next = index + 1; next < lines.length; next += 1) {
      if (lines[next]?.role === 'user') break
      hidden.add(next)
    }
  }
  return hidden
}

let sharedLane: BossLane = emptyBossLane()
const seenAsks = new Set<string>()
let jobRunner: ((job: BossJob) => void) | null = null
let bossSessionId = 'toboss'

/** Test isolation only. */
export function resetBossRuntimeForTests(): void {
  sharedLane = emptyBossLane()
  seenAsks.clear()
  jobRunner = null
  bossSessionId = 'toboss'
}

export function currentBossSession(): string {
  return bossSessionId
}

const BOSS_UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/

/** Session file for one group. The private ToBoss id is not this id. */
export function bossRoomSessionId(privateSessionId: string, conversationId: string): string | null {
  const room = conversationId.trim().toLowerCase()
  if (!BOSS_UUID.test(room)) return null
  if (privateSessionId === 'toboss') return `toboss-room-${room}`
  if (!privateSessionId.startsWith('toboss-')) return null
  const member = privateSessionId.slice('toboss-'.length).toLowerCase()
  if (!BOSS_UUID.test(member)) return null
  return `toboss-${member}-room-${room}`
}

export function noteBossSession(id: string): void {
  const trimmed = id.trim()
  if (trimmed) bossSessionId = trimmed
}

/** The open messages page owns the turn. The shell uses this when that page is closed. */
export function registerBossJobRunner(next: ((job: BossJob) => void) | null): void {
  jobRunner = next
}

export function bossJobRunner(): ((job: BossJob) => void) | null {
  return jobRunner
}

export function offerSharedBossJob(job: BossJob, now: number): { decision: BossDecision } {
  const result = offerBossJob(sharedLane, job, now)
  sharedLane = result.lane
  return { decision: result.decision }
}

export function finishSharedBossJob(now: number, recorded = true): BossJob | null {
  const step = completeBossJob(sharedLane, now, recorded)
  sharedLane = step.lane
  return step.job
}

export function bossGroupKey(orgId: string, sessionId: string): string {
  return sessionId === 'toboss' ? `messages.toboss.groups.${orgId}` : `messages.toboss.groups.${sessionId}.${orgId}`
}

function readStoredIds(key: string): string[] {
  if (typeof localStorage === 'undefined') return []
  try {
    const raw = globalThis.localStorage.getItem(key)
    const parsed = raw ? (JSON.parse(raw) as unknown) : []
    return Array.isArray(parsed) ? parsed.filter((id): id is string => typeof id === 'string') : []
  } catch {
    return []
  }
}

/** Canonical list plus any list saved after the session id changed. */
export function readBossGroups(orgId: string | null, sessionId: string): string[] {
  if (!orgId || typeof localStorage === 'undefined') return []
  const ids = new Set(readStoredIds(bossGroupKey(orgId, 'toboss')))
  if (sessionId && sessionId !== 'toboss') {
    for (const id of readStoredIds(bossGroupKey(orgId, sessionId))) ids.add(id)
  }
  const prefix = 'messages.toboss.groups.'
  const suffix = `.${orgId}`
  for (let index = 0; index < globalThis.localStorage.length; index += 1) {
    const key = globalThis.localStorage.key(index)
    if (!key?.startsWith(prefix) || !key.endsWith(suffix)) continue
    for (const id of readStoredIds(key)) ids.add(id)
  }
  return [...ids]
}

export function writeBossGroups(orgId: string, sessionId: string, ids: string[]): void {
  if (!orgId || typeof localStorage === 'undefined') return
  const raw = JSON.stringify(ids)
  globalThis.localStorage.setItem(bossGroupKey(orgId, 'toboss'), raw)
  if (sessionId && sessionId !== 'toboss') globalThis.localStorage.setItem(bossGroupKey(orgId, sessionId), raw)
}

export const BOSS_WARM_CAP = 8

/** A group ToBoss is in, or whose latest line names ToBoss, needs its first page. */
export function bossRoomNeedsWarm(
  entry: { kind: string; previewText: string | null },
  loaded: boolean,
  groups: readonly string[],
  labels: readonly string[],
  id: string
): boolean {
  if (loaded || entry.kind !== 'group') return false
  if (groups.includes(id)) return true
  return Boolean(entry.previewText && labels.some((label) => textMentionsNamedBot(entry.previewText ?? '', label)))
}

/** Claims each fresh @ once. A later call does not start that @ again. */
export function acceptBossCatchUp(input: {
  now: number
  labels: readonly string[]
  bossName: string
  bossListed: boolean
  memberGroupIds: readonly string[]
  rooms: readonly { id: string; title: string; messages: readonly BossRoomMessage[] }[]
}): { started: BossJob[]; membership: Array<{ id: string; present: boolean }> } {
  const started: BossJob[] = []
  const membership: Array<{ id: string; present: boolean }> = []
  for (const room of input.rooms) {
    const state = bossMembership(room.messages)
    if (state === 'in') membership.push({ id: room.id, present: true })
    else if (state === 'out') membership.push({ id: room.id, present: false })
    if (!input.bossListed || state === 'out') continue
    if (state !== 'in' && !input.memberGroupIds.includes(room.id)) continue
    for (const ask of openBossAsks(room.messages, input.labels, input.now)) {
      if (seenAsks.has(ask.id)) continue
      const askText = visibleText(ask.text ?? '').trim()
      if (!askText) {
        seenAsks.add(ask.id)
        continue
      }
      if (bossAskSettled(sharedLane, room.id, ask.senderId, askText, ask.createdAt)) {
        seenAsks.add(ask.id)
        continue
      }
      seenAsks.add(ask.id)
      const job: BossJob = {
        key: `${room.id}:${ask.senderId}:${ask.id}`,
        conversationId: room.id,
        senderId: ask.senderId,
        ask: askText,
        prompt: bossRoomPrompt({
          room: room.title,
          asker: ask.senderName,
          ask: askText,
          lines: bossTranscript(room.messages, input.bossName)
        })
      }
      const result = offerBossJob(sharedLane, job, input.now)
      sharedLane = result.lane
      if (result.decision === 'start') started.push(job)
    }
  }
  return { started, membership }
}

/** Mentions after the latest answer, inside the catch-up window, oldest first. */
export function openBossAsks(
  messages: readonly BossRoomMessage[],
  labels: readonly string[],
  now: number
): BossRoomMessage[] {
  const names = labels.map((label) => label.trim()).filter(Boolean)
  const found: BossRoomMessage[] = []
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    const message = messages[index]
    if (!message || message.recalled) continue
    if (now - message.createdAt > BOSS_CATCHUP_MS) break
    const wire = message.text ? decodeBossWire(message.text) : null
    if (wire?.kind === 'reply') break
    if (wire?.kind === 'join' || wire?.kind === 'leave' || !message.text) continue
    if (!names.some((label) => textMentionsNamedBot(message.text ?? '', label))) continue
    found.push(message)
    if (found.length >= BOSS_QUEUE_CAP) break
  }
  return found.reverse()
}
