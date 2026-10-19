export type OpenUiSegment = {
  kind: 'markdown' | 'openui'
  source: string
  start: number
  incomplete?: boolean
}

/** Keep a streamed OpenUI fence at the same position when its closing fence arrives. */
export function splitOpenUiFences(source: string): OpenUiSegment[] {
  const segments: OpenUiSegment[] = []
  let cursor = 0
  let offset = 0
  let open: { marker: string; length: number; start: number; body: number } | null = null
  let outer: { marker: string; length: number } | null = null
  for (const line of source.split(/(?<=\n)/)) {
    const match = line.replace(/\r?\n$/, '').match(/^ {0,3}(`{3,}|~{3,})([^`~]*)$/)
    if (match) {
      const marker = match[1][0]
      const length = match[1].length
      const info = match[2].trim()
      if (outer) {
        if (marker === outer.marker && length >= outer.length && !info) outer = null
      } else if (!open) {
        if (info === 'openui') {
          open = { marker, length, start: offset, body: offset + line.length }
        } else {
          outer = { marker, length }
        }
      } else if (marker === open.marker && length >= open.length && !info) {
        if (cursor < open.start)
          segments.push({ kind: 'markdown', source: source.slice(cursor, open.start), start: cursor })
        segments.push({
          kind: 'openui',
          source: source.slice(open.body, offset),
          start: open.start,
          incomplete: false
        })
        cursor = offset + line.length
        open = null
      }
    }
    offset += line.length
  }
  if (open) {
    if (cursor < open.start)
      segments.push({ kind: 'markdown', source: source.slice(cursor, open.start), start: cursor })
    segments.push({ kind: 'openui', source: source.slice(open.body), start: open.start, incomplete: true })
    cursor = source.length
  }
  if (cursor < source.length) segments.push({ kind: 'markdown', source: source.slice(cursor), start: cursor })
  return segments
}
