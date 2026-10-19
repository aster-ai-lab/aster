/** A reply quote stored at the end of a text body. The live IM API only accepts text. */
export type MessageQuote = {
  messageId: string
  senderId: string
  senderName: string
  excerpt: string
  kind: 'text' | 'file' | 'image' | 'voice'
}

const MARK = '\u2063A3SQ1:'
/** Suffix that marks a ToBoss join, leave, or group reply. The visible text stays in front. */
export const BOSS_MARK = '\u2063A3SB1:'
/** IM rejects a text body above this size. The mark stays on the kept prefix. */
const BOSS_WIRE_MAX_BYTES = 16 * 1024

export type BossWireKind = 'join' | 'leave' | 'reply'

function clipToBytes(text: string, maxBytes: number): string {
  const encoder = new TextEncoder()
  if (encoder.encode(text).length <= maxBytes) return text
  let end = text.length
  while (end > 0 && encoder.encode(text.slice(0, end)).length > maxBytes) {
    end = Math.max(0, end - 32)
  }
  if (end > 0) {
    const code = text.charCodeAt(end - 1)
    if (code >= 0xd800 && code <= 0xdbff) end -= 1
  }
  return text.slice(0, end)
}

/** Join, leave, or a group answer. The visible sentence stays in front of the mark. */
export function encodeBossWire(text: string, kind: BossWireKind): string {
  const suffix = `${BOSS_MARK}${encodeURIComponent(JSON.stringify({ kind }))}`
  const room = BOSS_WIRE_MAX_BYTES - new TextEncoder().encode(suffix).length
  return `${clipToBytes(text, Math.max(0, room))}${suffix}`
}

export function decodeBossWire(raw: string): { text: string; kind: BossWireKind } | null {
  const at = raw.lastIndexOf(BOSS_MARK)
  if (at < 0) return null
  try {
    const parsed = JSON.parse(decodeURIComponent(raw.slice(at + BOSS_MARK.length))) as { kind?: unknown }
    if (parsed?.kind !== 'join' && parsed?.kind !== 'leave' && parsed?.kind !== 'reply') return null
    return { text: raw.slice(0, at), kind: parsed.kind }
  } catch {
    return null
  }
}

function withoutBoss(raw: string): string {
  const at = raw.lastIndexOf(BOSS_MARK)
  return at < 0 ? raw : raw.slice(0, at)
}

type WireQuote = { i?: unknown; s?: unknown; n?: unknown; e?: unknown; k?: unknown }

function quoteKind(value: unknown): MessageQuote['kind'] {
  if (value === 'file' || value === 'image' || value === 'voice') return value
  return 'text'
}

/** Visible message text, without a quote or ToBoss suffix. A cut-off preview still hides the suffix. */
export function visibleText(raw: string): string {
  const text = withoutBoss(raw)
  const at = text.lastIndexOf(MARK)
  return at < 0 ? text : text.slice(0, at)
}

export function decodeQuotedText(raw: string): { text: string; quote: MessageQuote | null } {
  const text = withoutBoss(raw)
  const at = text.lastIndexOf(MARK)
  if (at < 0) return { text, quote: null }
  try {
    const parsed = JSON.parse(decodeURIComponent(text.slice(at + MARK.length))) as WireQuote
    if (!parsed || typeof parsed.i !== 'string' || typeof parsed.e !== 'string' || !parsed.e.trim()) {
      return { text, quote: null }
    }
    return {
      text: text.slice(0, at),
      quote: {
        messageId: parsed.i,
        senderId: typeof parsed.s === 'string' ? parsed.s : '',
        senderName: typeof parsed.n === 'string' ? parsed.n : '',
        excerpt: parsed.e,
        kind: quoteKind(parsed.k)
      }
    }
  } catch {
    return { text: raw, quote: null }
  }
}

export function encodeQuotedText(text: string, quote: MessageQuote): string {
  const payload = encodeURIComponent(
    JSON.stringify({
      i: quote.messageId,
      s: quote.senderId,
      n: [...quote.senderName].slice(0, 40).join(''),
      e: [...quote.excerpt].slice(0, 80).join(''),
      k: quote.kind
    })
  )
  return `${text}${MARK}${payload}`
}

export function makeQuote(
  messageId: string,
  senderId: string,
  senderName: string,
  kind: MessageQuote['kind'],
  excerpt: string
): MessageQuote {
  const text = excerpt.replace(/\s+/g, ' ').trim()
  return {
    messageId,
    senderId,
    senderName: senderName.trim() || '…',
    excerpt: [...text].slice(0, 80).join('') || '…',
    kind
  }
}
