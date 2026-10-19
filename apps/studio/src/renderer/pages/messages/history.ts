import { readableChatText } from '@renderer/im/forwardRecord'

export type HistoryKind = 'text' | 'file' | 'image' | 'voice' | 'link'
export type HistoryFilter = 'all' | 'file' | 'image' | 'link'
export type HistoryRange = { start: string; end: string }

export type HistoryRecord = {
  id: string
  key: string
  at: number
  senderName: string
  phone?: string
  avatar?: string | null
  colorKey?: string
  kind: HistoryKind
  text: string
  thumb: boolean
}

const LINK = /https?:\/\/[^\s<>]+/i

export function textHasLink(text: string): boolean {
  return LINK.test(text)
}

/** Local calendar day, `YYYY-MM-DD`. */
export function dayKey(at: number): string {
  const date = new Date(at)
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${date.getFullYear()}-${month}-${day}`
}

export function toHistoryRecord(
  source: Omit<HistoryRecord, 'key' | 'thumb' | 'kind'> & { key?: string; kind: Exclude<HistoryKind, 'link'> }
): HistoryRecord {
  const text = source.kind === 'text' ? readableChatText(source.text) : source.text
  const kind: HistoryKind = source.kind === 'text' && textHasLink(text) ? 'link' : source.kind
  return {
    id: source.id,
    key: source.key ?? source.id,
    at: source.at,
    senderName: source.senderName,
    phone: source.phone,
    avatar: source.avatar,
    colorKey: source.colorKey,
    kind,
    text,
    thumb: source.kind === 'image'
  }
}

export function filterHistory(
  records: readonly HistoryRecord[],
  query: string,
  filter: HistoryFilter,
  range: HistoryRange | null
): HistoryRecord[] {
  const needle = query.trim().toLocaleLowerCase()
  const start = range?.start ?? ''
  const end = range?.end || start
  return records
    .filter((record) => {
      if (filter === 'file' && record.kind !== 'file') return false
      if (filter === 'image' && record.kind !== 'image') return false
      if (filter === 'link' && record.kind !== 'link') return false
      if (start) {
        const day = dayKey(record.at)
        if (day < start || day > end) return false
      }
      return true
    })
    .filter((record) => !needle || `${record.senderName} ${record.text}`.toLocaleLowerCase().includes(needle))
    .slice()
    .sort((left, right) => right.at - left.at || right.key.localeCompare(left.key))
}

export function highlightParts(text: string, query: string): Array<{ text: string; hit: boolean }> {
  const needle = query.trim()
  if (!needle) return [{ text, hit: false }]
  const lower = text.toLocaleLowerCase()
  const want = needle.toLocaleLowerCase()
  const parts: Array<{ text: string; hit: boolean }> = []
  let from = 0
  while (from < text.length) {
    const at = lower.indexOf(want, from)
    if (at < 0) {
      parts.push({ text: text.slice(from), hit: false })
      break
    }
    if (at > from) parts.push({ text: text.slice(from, at), hit: false })
    parts.push({ text: text.slice(at, at + want.length), hit: true })
    from = at + want.length
    if (want.length === 0) break
  }
  return parts.filter((part) => part.text)
}
