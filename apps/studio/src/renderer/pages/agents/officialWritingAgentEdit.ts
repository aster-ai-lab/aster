import { officialWritingDocumentFromTask } from './officialWriting'

export interface OfficialWritingInsertion {
  at: number
  text: string
  /** Characters at `at` that the new text replaces. Zero is a pure insertion. */
  replaced?: number
}

/** One span, revealed one character at a time. */
export interface OfficialWritingTyping {
  caret: number
  /** Characters of the current document that this span replaces. */
  replaced: number
  incoming: string
  frames: readonly string[]
}

export type OfficialWritingFollowUp =
  | { ok: true; kept: string; typing: OfficialWritingTyping; steps: readonly OfficialWritingTyping[] }
  | { ok: false }

interface WritingHunk {
  at: number
  removed: number
  text: string
}

/**
 * Frames are growing prefixes of the new span.
 * A multi-character span therefore has more than one frame.
 */
export function planOfficialWritingTyping(
  document: string,
  insertion: OfficialWritingInsertion
): OfficialWritingTyping {
  const caret = Math.min(Math.max(insertion.at, 0), document.length)
  const room = Math.max(document.length - caret, 0)
  const replaced = Math.min(Math.max(insertion.replaced ?? 0, 0), room)
  const incoming = insertion.text
  const frames: string[] = []
  for (let index = 1; index <= incoming.length; index += 1) frames.push(incoming.slice(0, index))
  return { caret, replaced, incoming, frames }
}

/** Place one revealed frame at the caret. Text outside the edited span stays. */
export function revealOfficialWritingFrame(document: string, typing: OfficialWritingTyping, frame: string): string {
  const replaced = Math.max(typing.replaced, 0)
  return document.slice(0, typing.caret) + frame + document.slice(typing.caret + replaced)
}

/**
 * A later document keeps the earlier text and changes one or more spans.
 * A placeholder such as （待填写） can become the sentence for that span.
 * A draft that keeps none of the earlier document is not a collaboration edit.
 */
export function planOfficialWritingFollowUp(previous: string, next: string): OfficialWritingFollowUp {
  const hunks = writingHunks(previous, next)
  if (!hunks) return { ok: false }
  let delta = 0
  const steps = hunks.map((hunk) => {
    const typing = framesFor(hunk.at + delta, hunk.removed, hunk.text)
    delta += hunk.text.length - hunk.removed
    return typing
  })
  const typing = steps[0]
  if (!typing) return { ok: false }
  return { ok: true, kept: previous, typing, steps }
}

function framesFor(caret: number, replaced: number, incoming: string): OfficialWritingTyping {
  const frames: string[] = []
  for (let index = 1; index <= incoming.length; index += 1) frames.push(incoming.slice(0, index))
  return { caret, replaced, incoming, frames }
}

function writingHunks(previous: string, next: string): WritingHunk[] | null {
  if (previous.length === 0 || previous === next) return null
  const hunks: WritingHunk[] = []
  collectWritingHunks(previous, next, 0, hunks)
  if (hunks.length === 0 || hunks.some((hunk) => hunk.text.length === 0)) return null
  let kept = previous
  for (const hunk of [...hunks].reverse()) {
    kept = kept.slice(0, hunk.at) + kept.slice(hunk.at + hunk.removed)
  }
  if (kept.trim().length === 0) return null
  let cursor = previous
  let delta = 0
  for (const hunk of hunks) {
    const at = hunk.at + delta
    cursor = cursor.slice(0, at) + hunk.text + cursor.slice(at + hunk.removed)
    delta += hunk.text.length - hunk.removed
  }
  if (cursor !== next) return null
  if (dropsTextOutsideSpan(previous, next, hunks)) return null
  return hunks
}

/** A prefix snapshot deletes the closer. Only the edited line may leave the draft. */
function dropsTextOutsideSpan(previous: string, next: string, hunks: WritingHunk[]): boolean {
  for (const hunk of hunks) {
    const removed = previous.slice(hunk.at, hunk.at + hunk.removed)
    const lines = removed
      .split('\n')
      .map((line) => line.trim())
      .filter((line) => line.length > 0)
    if (lines.length <= 1) continue
    for (const line of lines.slice(1)) {
      if (!next.includes(line)) return true
    }
  }
  return false
}

function collectWritingHunks(previous: string, next: string, base: number, hunks: WritingHunk[]) {
  if (previous === next) return
  const limit = Math.min(previous.length, next.length)
  let prefix = 0
  while (prefix < limit && previous.charCodeAt(prefix) === next.charCodeAt(prefix)) prefix += 1
  let suffix = 0
  while (
    suffix < previous.length - prefix &&
    suffix < next.length - prefix &&
    previous.charCodeAt(previous.length - 1 - suffix) === next.charCodeAt(next.length - 1 - suffix)
  ) {
    suffix += 1
  }
  const midPrevious = previous.slice(prefix, previous.length - suffix)
  const midNext = next.slice(prefix, next.length - suffix)
  const anchor = sharedWritingLine(midPrevious, midNext)
  if (anchor) {
    const previousAt = boundedLineIndex(midPrevious, anchor)
    const nextAt = boundedLineIndex(midNext, anchor)
    if (previousAt >= 0 && nextAt >= 0) {
      collectWritingHunks(midPrevious.slice(0, previousAt), midNext.slice(0, nextAt), base + prefix, hunks)
      collectWritingHunks(
        midPrevious.slice(previousAt + anchor.length),
        midNext.slice(nextAt + anchor.length),
        base + prefix + previousAt + anchor.length,
        hunks
      )
      return
    }
  }
  hunks.push({ at: base + prefix, removed: midPrevious.length, text: midNext })
}

/** A whole line that still sits in both sides, so two edits stay two spans. */
function sharedWritingLine(previous: string, next: string): string | null {
  if (previous.length === 0 || next.length === 0) return null
  let best = ''
  for (const line of previous.split('\n')) {
    if (line.trim().length === 0 || line.length <= best.length) continue
    if (countBoundedLine(previous, line) !== 1 || countBoundedLine(next, line) !== 1) continue
    best = line
  }
  if (best.length === 0) return null
  return best
}

function boundedLineIndex(source: string, line: string): number {
  let from = 0
  while (from <= source.length - line.length) {
    const at = source.indexOf(line, from)
    if (at < 0) return -1
    const end = at + line.length
    const before = at === 0 || source.charCodeAt(at - 1) === 10
    const after = end === source.length || source.charCodeAt(end) === 10
    if (before && after) return at
    from = at + 1
  }
  return -1
}

function countBoundedLine(source: string, line: string): number {
  let count = 0
  let from = 0
  while (from <= source.length - line.length) {
    const at = boundedLineIndex(source.slice(from), line)
    if (at < 0) break
    count += 1
    from += at + line.length
  }
  return count
}

export interface OfficialWritingSession {
  instruction: string
  /** Full draft. Collaboration checks read this even while the caret is still typing it. */
  document: string
  update: string | null
  /** True until the opening caret has painted the full draft. */
  opening: boolean
  /** Task that owns this paper. Absent in tests that arm a document without a session. */
  sessionId: string | null
}

let session: OfficialWritingSession | null = null
let viewingSessionId: string | null = null
const listeners = new Set<() => void>()

function emit() {
  for (const listener of listeners) listener()
}

export function subscribeOfficialWritingSession(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function readOfficialWritingSession(): OfficialWritingSession | null {
  return session
}

export function resetOfficialWritingSessionForTests() {
  session = null
  viewingSessionId = null
  emit()
}

/** The conversation currently showing the writing paper. Follow-ups from other tasks stay off it. */
export function setOfficialWritingView(sessionId: string | null) {
  viewingSessionId = sessionId
}

/** Open the task document from a confirmed writing instruction. */
export function armOfficialWritingDocument(instruction: string, sessionId?: string) {
  const document = officialWritingDocumentFromTask(instruction)
  if (!document) return
  const owner = sessionId ?? null
  if (session?.instruction === instruction && session.document === document && session.sessionId === owner) return
  session = { instruction, document, update: null, opening: true, sessionId: owner }
  emit()
}

/** The opening caret has painted the armed draft. Queued follow-ups can start. */
export function finishOfficialWritingOpening() {
  if (!session?.opening) return
  session = { ...session, opening: false }
  emit()
}

export function offerOfficialWritingAgentDocument(next: string): 'applied' | 'rejected' | 'idle' {
  if (!session) return 'idle'
  if (session.update === next || session.document === next) return 'idle'
  if (!planOfficialWritingFollowUp(session.document, next).ok) return 'rejected'
  session = { ...session, update: next }
  emit()
  return 'applied'
}

/** Assistant prose in the open writing task. Chatter that drops the draft is ignored. */
export function noteOfficialWritingAgentProse(text: string): 'applied' | 'rejected' | 'idle' {
  const next = text.trim()
  if (!next) return 'idle'
  if (session?.sessionId && viewingSessionId !== session.sessionId) return 'idle'
  return offerOfficialWritingAgentDocument(next)
}

export function commitOfficialWritingDocument(next: string) {
  if (!session || session.document === next) return
  session = { ...session, document: next, update: null, opening: false }
  emit()
}
