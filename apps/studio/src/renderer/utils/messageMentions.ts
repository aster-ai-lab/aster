/** Group @ tokens. The stored message stays plain text, and names are matched when it is shown. */

import type { ImInboxEntry, ImMessage, ImSender } from '@renderer/im/imApi'
import { visibleText } from '@renderer/im/quote'

export type MentionTarget = 'all' | 'member' | 'bot'

export type MentionLabel = {
  label: string
  me: boolean
  target: MentionTarget
}

export type MentionPiece =
  | { kind: 'text'; text: string }
  | { kind: 'mention'; text: string; me: boolean; target: MentionTarget }

export type MentionQuery = { start: number; end: number; filter: string }

const CANONICAL_EVERYONE = ['所有人', 'everyone']
const NAME_CHAR = /[\p{L}\p{N}_'’·-]/u

function isNameChar(char: string | undefined): boolean {
  return char !== undefined && NAME_CHAR.test(char)
}

function atChar(char: string | undefined): boolean {
  return char === '@' || char === '＠'
}

function boundaryBefore(text: string, index: number): boolean {
  return index === 0 || /\s/u.test(text[index - 1] ?? '')
}

/** The open @ query at the caret. An address such as name@mail.com is not one. */
export function mentionQuery(text: string, caret: number): MentionQuery | null {
  if (caret < 1 || caret > text.length) return null
  let index = caret - 1
  while (index >= 0) {
    const char = text[index]
    if (char !== undefined && /\s/u.test(char)) return null
    if (atChar(char)) {
      if (!boundaryBefore(text, index)) return null
      return { start: index, end: caret, filter: text.slice(index + 1, caret) }
    }
    index -= 1
  }
  return null
}

/** Prefix matches stay ahead of substring matches. Equal ranks keep the given order. */
export function filterMentions<T extends { label: string }>(candidates: readonly T[], filter: string): T[] {
  const needle = filter.trim().toLocaleLowerCase()
  if (!needle) return [...candidates]
  const ranked = candidates.flatMap((item) => {
    const label = item.label.toLocaleLowerCase()
    if (label.startsWith(needle)) return [{ item, rank: 0 }]
    if (label.includes(needle)) return [{ item, rank: 1 }]
    return []
  })
  return ranked.sort((left, right) => left.rank - right.rank).map((row) => row.item)
}

export function applyMention(text: string, query: MentionQuery, label: string): { text: string; caret: number } {
  const insert = `@${label} `
  return { text: text.slice(0, query.start) + insert + text.slice(query.end), caret: query.start + insert.length }
}

export function highlightLabels(input: {
  everyone: string
  myName: string
  bot: string | null
  bots?: readonly string[]
  people: readonly { label: string; me: boolean }[]
}): MentionLabel[] {
  const byKey = new Map<string, MentionLabel>()
  const add = (label: string, me: boolean, target: MentionTarget) => {
    const trimmed = label.trim()
    if (!trimmed) return
    const key = trimmed.toLocaleLowerCase()
    const existing = byKey.get(key)
    if (!existing) {
      byKey.set(key, { label: trimmed, me, target })
      return
    }
    if (me) existing.me = true
  }
  for (const person of input.people) add(person.label, person.me, 'member')
  if (input.myName.trim()) add(input.myName, true, 'member')
  if (input.bot) add(input.bot, false, 'bot')
  for (const label of input.bots ?? []) add(label, false, 'bot')
  for (const label of [input.everyone, ...CANONICAL_EVERYONE]) add(label, true, 'all')
  return [...byKey.values()]
}

export function splitMentions(text: string, labels: readonly MentionLabel[]): MentionPiece[] {
  const sorted = [...labels]
    .filter((item) => item.label.length > 0)
    .sort((left, right) => right.label.length - left.label.length)
  const pieces: MentionPiece[] = []
  let cursor = 0
  let index = 0
  while (index < text.length) {
    if (!atChar(text[index]) || !boundaryBefore(text, index)) {
      index += 1
      continue
    }
    const rest = text.slice(index + 1)
    const hit = sorted.find((item) => {
      if (!rest.toLocaleLowerCase().startsWith(item.label.toLocaleLowerCase())) return false
      return !isNameChar(rest[item.label.length])
    })
    if (!hit) {
      index += 1
      continue
    }
    if (index > cursor) pieces.push({ kind: 'text', text: text.slice(cursor, index) })
    const taken = 1 + hit.label.length
    pieces.push({ kind: 'mention', text: text.slice(index, index + taken), me: hit.me, target: hit.target })
    index += taken
    cursor = index
  }
  if (cursor < text.length) pieces.push({ kind: 'text', text: text.slice(cursor) })
  if (pieces.length === 0) pieces.push({ kind: 'text', text })
  return pieces
}

export function textMentionsYou(text: string, identity: { myName: string; everyone: string }): boolean {
  const labels = highlightLabels({ everyone: identity.everyone, myName: identity.myName, bot: null, people: [] })
  return splitMentions(text, labels).some((piece) => piece.kind === 'mention' && piece.me)
}

export function textMentionsBot(text: string, botLabel: string, everyone: string): boolean {
  const labels = highlightLabels({ everyone, myName: '', bot: botLabel, people: [] })
  return splitMentions(visibleText(text), labels).some(
    (piece) => piece.kind === 'mention' && (piece.target === 'bot' || piece.target === 'all')
  )
}

/** Explicit @name. @所有人 does not wake ToBoss. */
export function textMentionsNamedBot(text: string, botLabel: string): boolean {
  const labels = highlightLabels({ everyone: '', myName: '', bot: botLabel, people: [] })
  return splitMentions(visibleText(text), labels).some((piece) => piece.kind === 'mention' && piece.target === 'bot')
}

/** A completed @ at the caret deletes in one key, including the space inserted after it. */
export function mentionBeforeCaret(
  text: string,
  caret: number,
  labels: readonly MentionLabel[]
): { start: number; end: number } | null {
  let offset = 0
  for (const piece of splitMentions(text, labels)) {
    const next = offset + piece.text.length
    if (piece.kind === 'mention') {
      const spaceAfter = text[next] === ' '
      if (caret === next || (spaceAfter && caret === next + 1)) return { start: offset, end: caret }
    }
    offset = next
  }
  return null
}

function fromOther(sender: ImSender | null, myId: string | null): boolean {
  return Boolean(myId && sender && sender.kind === 'member' && sender.id !== myId)
}

export function entryMentionsYou(
  entry: Pick<ImInboxEntry, 'kind' | 'unread' | 'lastReadSequence' | 'preview'>,
  messages: readonly ImMessage[] | undefined,
  identity: { myId: string | null; myName: string; everyone: string }
): boolean {
  if (entry.kind !== 'group' || entry.unread <= 0) return false
  const addressed = (text: string) => textMentionsYou(text, identity)
  if (
    messages?.some(
      (message) =>
        message.sequence > entry.lastReadSequence &&
        !message.recalled &&
        message.body.kind === 'text' &&
        fromOther(message.sender, identity.myId) &&
        addressed(visibleText(message.body.text))
    )
  ) {
    return true
  }
  const preview = entry.preview
  if (
    !preview ||
    preview.recalled ||
    preview.kind !== 'text' ||
    !preview.text ||
    !fromOther(preview.sender, identity.myId)
  ) {
    return false
  }
  return addressed(visibleText(preview.text))
}
