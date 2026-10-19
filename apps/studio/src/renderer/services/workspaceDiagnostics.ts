import type {
  WorkspaceDiagnosticPublication,
  WorkspaceDocumentDiagnostic,
  WorkspaceDocumentSnapshot
} from '@shared/types/workspaceDocument'

export function diagnosticPublication(
  value: unknown,
  snapshot: WorkspaceDocumentSnapshot
): WorkspaceDiagnosticPublication {
  const row = value as WorkspaceDiagnosticPublication | null
  if (
    !row ||
    typeof row !== 'object' ||
    typeof row.publisherId !== 'string' ||
    !/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/.test(row.publisherId) ||
    !Number.isSafeInteger(row.sequence) ||
    row.sequence < 1 ||
    row.version !== snapshot.version ||
    !Array.isArray(row.items) ||
    row.items.length > 128 ||
    new globalThis.TextEncoder().encode(JSON.stringify(row.items)).length > 16 * 1024 ||
    new globalThis.TextEncoder().encode(snapshot.text).length > 64 * 1024 ||
    Object.keys(row).sort().join(',') !== 'items,publisherId,sequence,version'
  )
    throw new Error('Invalid native diagnostic publication')
  return Object.freeze({
    publisherId: row.publisherId,
    sequence: row.sequence,
    version: row.version,
    items: Object.freeze(row.items.map((item: WorkspaceDocumentDiagnostic) => diagnostic(item, snapshot.text)))
  })
}

function diagnostic(item: WorkspaceDocumentDiagnostic, text: string): WorkspaceDocumentDiagnostic {
  if (
    !item ||
    !['error', 'warning', 'information', 'hint'].includes(item.severity) ||
    typeof item.message !== 'string' ||
    !item.message ||
    item.message.includes('\0') ||
    new globalThis.TextEncoder().encode(item.message).length > 2048 ||
    Object.keys(item).sort().join(',') !== 'end,message,severity,start' ||
    !positionValid(item.start, text) ||
    !positionValid(item.end, text) ||
    item.start.line > item.end.line ||
    (item.start.line === item.end.line && item.start.character > item.end.character)
  )
    throw new Error('Invalid native diagnostic range or message')
  return Object.freeze({
    start: Object.freeze({ ...item.start }),
    end: Object.freeze({ ...item.end }),
    severity: item.severity,
    message: item.message
  })
}

function positionValid(position: WorkspaceDocumentDiagnostic['start'], text: string): boolean {
  if (
    !position ||
    !Number.isSafeInteger(position.line) ||
    position.line < 0 ||
    !Number.isSafeInteger(position.character) ||
    position.character < 0 ||
    Object.keys(position).sort().join(',') !== 'character,line'
  )
    return false
  const line = text.split('\n')[position.line]?.replace(/\r$/, '')
  if (line === undefined || position.character > line.length) return false
  const before = line.charCodeAt(position.character - 1)
  const after = line.charCodeAt(position.character)
  return !(before >= 0xd800 && before <= 0xdbff && after >= 0xdc00 && after <= 0xdfff)
}
