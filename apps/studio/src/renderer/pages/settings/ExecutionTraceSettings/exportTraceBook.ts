import { type TraceBook, type TrainingRecord } from './extractExecutionTrace'

export interface TraceExport {
  text: string
  measured: number
}

/** One JSON object per stored record. A missing duration stays off the line. */
export function exportTraceDocument(book: TraceBook): TraceExport {
  let measured = 0
  const lines: string[] = []
  for (const session of book.sessions) {
    for (const event of session.events) {
      const record = exportRecord(event.record)
      if (hasMeasuredDuration(record)) measured += 1
      lines.push(JSON.stringify(record))
    }
  }
  return { text: lines.length === 0 ? '' : `${lines.join('\n')}\n`, measured }
}

export function saveTraceFile(filename: string, text: string): void {
  try {
    const urlApi = globalThis.URL
    if (typeof urlApi?.createObjectURL !== 'function') return
    const blob = new Blob([text], { type: 'application/x-ndjson' })
    const url = urlApi.createObjectURL(blob)
    const link = document.createElement('a')
    link.href = url
    link.download = filename
    document.body.appendChild(link)
    link.click()
    link.remove()
    urlApi.revokeObjectURL(url)
  } catch {
    // The page still reports whether the text is a training sample.
  }
}

function exportRecord(record: TrainingRecord): TrainingRecord {
  const payload = { ...record.payload }
  const signals = asRecord(payload.signals)
  if (signals) {
    const next = { ...signals }
    if (finiteNumber(next.duration_ms) === undefined) delete next.duration_ms
    payload.signals = next
  }
  if (finiteNumber(payload.duration_ms) === undefined) delete payload.duration_ms
  return { ...record, payload }
}

function hasMeasuredDuration(record: TrainingRecord): boolean {
  const signals = asRecord(record.payload.signals)
  if (signals && finiteNumber(signals.duration_ms) !== undefined) return true
  return finiteNumber(record.payload.duration_ms) !== undefined
}

function finiteNumber(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null
  return value as Record<string, unknown>
}
