import { PDF_TOOL_BYTE_LIMIT, PDF_TOOL_FILE_LIMIT, intake, isPdfToolKind, type PdfToolKind } from '../pdfToolsJob'

export const DOCUMENT_ITEM_STATES = [
  'rejected',
  'ready',
  'queued',
  'running',
  'review',
  'accepted',
  'failed',
  'cancelling',
  'cancelled'
] as const
export type DocumentItemState = (typeof DOCUMENT_ITEM_STATES)[number]
export const DOCUMENT_SAVE_STATES = ['unsaved', 'saving', 'saved', 'failed'] as const
export type DocumentSaveState = (typeof DOCUMENT_SAVE_STATES)[number]

export class DocumentTaskError extends Error {
  constructor(
    readonly code:
      | 'invalid-source'
      | 'duplicate-source'
      | 'busy'
      | 'empty-selection'
      | 'invalid-selection'
      | 'stale-candidate'
      | 'invalid-receipt'
      | 'invalid-task'
  ) {
    super(`Document task refused: ${code}`)
    this.name = 'DocumentTaskError'
  }
}

export function parseDocumentItemState(value: unknown): DocumentItemState {
  if (typeof value !== 'string' || !DOCUMENT_ITEM_STATES.includes(value as DocumentItemState)) {
    throw new Error('Invalid document item state')
  }
  return value as DocumentItemState
}

export function documentItemStateAsStr(value: DocumentItemState): string {
  return parseDocumentItemState(value)
}

export function parseDocumentSaveState(value: unknown): DocumentSaveState {
  if (typeof value !== 'string' || !DOCUMENT_SAVE_STATES.includes(value as DocumentSaveState)) {
    throw new Error('Invalid document save state')
  }
  return value as DocumentSaveState
}

export function documentSaveStateAsStr(value: DocumentSaveState): string {
  return parseDocumentSaveState(value)
}

export type DocumentSource = Readonly<{ id: string; name: string; type: string; size: number }>
export type DocumentSourceRefusal = 'empty' | 'type' | 'too-big' | 'too-many' | 'legacy-office'
export type DocumentAttempt = Readonly<{ taskId: string; revision: number; sequence: number }>
export type DocumentCandidate<T> = Readonly<{ revision: number; sequence: number; value: T }>
export type DocumentSaveReceipt = Readonly<{ destination: string; savedAt: string }>

export type DocumentTaskItem<T> = Readonly<{
  source: DocumentSource
  state: DocumentItemState
  refusal?: DocumentSourceRefusal
  failure?: string
  candidate?: DocumentCandidate<T>
  accepted?: DocumentCandidate<T>
  saveState: DocumentSaveState
  saveSequence: number
  receipt?: DocumentSaveReceipt
}>

/** Presentation state only. Source bytes, credentials and persistence stay in their owning ports. */
export type DocumentTask<T> = Readonly<{
  id: string
  kind: PdfToolKind
  revision: number
  sequence: number
  items: readonly DocumentTaskItem<T>[]
  attempt?: DocumentAttempt
}>

function validId(value: string): boolean {
  return value.trim().length > 0 && value.length <= 256 && !value.includes('\0')
}

function validateSources(sources: readonly DocumentSource[]): void {
  const ids = new Set<string>()
  for (const source of sources) {
    if (
      !validId(source.id) ||
      !source.name.trim() ||
      source.name.length > 1024 ||
      source.name.includes('\0') ||
      !Number.isSafeInteger(source.size) ||
      source.size < 0
    ) {
      throw new DocumentTaskError('invalid-source')
    }
    if (ids.has(source.id)) throw new DocumentTaskError('duplicate-source')
    ids.add(source.id)
  }
}

export function createDocumentTask<T>(
  id: string,
  kind: PdfToolKind,
  sources: readonly DocumentSource[]
): DocumentTask<T> {
  if (!validId(id) || !isPdfToolKind(kind)) throw new DocumentTaskError('invalid-task')
  validateSources(sources)
  let validCount = 0
  const items = sources.map((input): DocumentTaskItem<T> => {
    const source = Object.freeze({ ...input })
    let refusal: DocumentSourceRefusal | undefined
    const extension = source.name.split('.').pop()?.toLowerCase()
    if (source.size === 0) refusal = 'empty'
    else if (source.size > PDF_TOOL_BYTE_LIMIT) refusal = 'too-big'
    else if (extension === 'doc' || extension === 'xls') refusal = 'legacy-office'
    else {
      const decision = intake(kind, [source])
      if (decision.state !== 'run') refusal = 'type'
      else if (validCount >= PDF_TOOL_FILE_LIMIT) refusal = 'too-many'
      else validCount++
    }
    return Object.freeze({
      source,
      state: refusal ? 'rejected' : 'ready',
      refusal,
      saveState: 'unsaved',
      saveSequence: 0
    })
  })
  return Object.freeze({ id, kind, revision: 1, sequence: 0, items: Object.freeze(items) })
}

function idle<T>(task: DocumentTask<T>): void {
  if (task.attempt || task.items.some((item) => item.saveState === 'saving')) throw new DocumentTaskError('busy')
}

/** Changing options invalidates review, while the previous accepted artifact remains recoverable. */
export function reviseDocumentTask<T>(task: DocumentTask<T>): DocumentTask<T> {
  idle(task)
  return freezeTask({
    ...task,
    revision: task.revision + 1,
    items: task.items.map((item) =>
      item.state === 'rejected'
        ? item
        : {
            ...item,
            state: 'ready',
            candidate: undefined,
            failure: undefined
          }
    )
  })
}

/** Only addressed source options changed. Current untouched bytes remain valid, old references do not. */
export function reviseDocumentItems<T>(task: DocumentTask<T>, sourceIds: readonly string[]): DocumentTask<T> {
  idle(task)
  const selected = new Set(sourceIds)
  if (
    !selected.size ||
    selected.size !== sourceIds.length ||
    sourceIds.some((id) => !task.items.some((item) => item.source.id === id && item.state !== 'rejected'))
  ) {
    throw new DocumentTaskError('invalid-selection')
  }
  const revision = task.revision + 1
  const rebind = (value?: DocumentCandidate<T>): DocumentCandidate<T> | undefined =>
    value?.revision === task.revision ? Object.freeze({ ...value, revision }) : value
  return freezeTask({
    ...task,
    revision,
    items: task.items.map((item) =>
      selected.has(item.source.id)
        ? { ...item, state: 'ready', candidate: undefined, failure: undefined }
        : { ...item, candidate: rebind(item.candidate), accepted: rebind(item.accepted) }
    )
  })
}

/** Source edits revalidate capacity and fence every result from the previous ordered input. */
function replaceDocumentSources<T>(task: DocumentTask<T>, sources: readonly DocumentSource[]): DocumentTask<T> {
  idle(task)
  const validated = createDocumentTask<T>(task.id, task.kind, sources)
  const previous = new Map(task.items.map((item) => [item.source.id, item]))
  return freezeTask({
    ...task,
    revision: task.revision + 1,
    items: validated.items.map((item) => {
      const old = previous.get(item.source.id)
      return old
        ? {
            ...item,
            accepted: old.accepted,
            saveState: old.saveState,
            saveSequence: old.saveSequence,
            receipt: old.receipt
          }
        : item
    })
  })
}

export function appendDocumentSources<T>(task: DocumentTask<T>, sources: readonly DocumentSource[]): DocumentTask<T> {
  idle(task)
  if (sources.length === 0) return task
  return replaceDocumentSources(task, [...task.items.map((item) => item.source), ...sources])
}

export function removeDocumentSources<T>(task: DocumentTask<T>, sourceIds: readonly string[]): DocumentTask<T> {
  idle(task)
  const selected = new Set(sourceIds)
  if (
    selected.size === 0 ||
    selected.size !== sourceIds.length ||
    sourceIds.some((id) => !task.items.some((item) => item.source.id === id))
  ) {
    throw new DocumentTaskError('invalid-selection')
  }
  return replaceDocumentSources(
    task,
    task.items.filter((item) => !selected.has(item.source.id)).map((item) => item.source)
  )
}

export function reorderDocumentSources<T>(task: DocumentTask<T>, order: readonly string[]): DocumentTask<T> {
  idle(task)
  if (order.length !== task.items.length || new Set(order).size !== order.length)
    throw new DocumentTaskError('invalid-selection')
  const byId = new Map(task.items.map((item) => [item.source.id, item.source]))
  const sources = order.map((id) => {
    const source = byId.get(id)
    if (!source) throw new DocumentTaskError('invalid-selection')
    return source
  })
  if (order.every((id, index) => task.items[index].source.id === id)) return task
  return replaceDocumentSources(task, sources)
}

export function startDocumentAttempt<T>(task: DocumentTask<T>, selectedIds: readonly string[]): DocumentTask<T> {
  idle(task)
  if (selectedIds.length === 0) throw new DocumentTaskError('empty-selection')
  const selected = new Set(selectedIds)
  if (
    selected.size !== selectedIds.length ||
    selectedIds.some(
      (id) => !task.items.some((item) => item.source.id === id && ['ready', 'failed', 'cancelled'].includes(item.state))
    )
  ) {
    throw new DocumentTaskError('invalid-selection')
  }
  const attempt = Object.freeze({
    taskId: task.id,
    revision: task.revision,
    sequence: task.sequence + 1
  })
  return freezeTask({
    ...task,
    sequence: attempt.sequence,
    attempt,
    items: task.items.map((item) =>
      selected.has(item.source.id) ? { ...item, state: 'queued', failure: undefined, candidate: undefined } : item
    )
  })
}

function currentAttempt<T>(task: DocumentTask<T>, attempt: DocumentAttempt): boolean {
  return (
    task.attempt?.taskId === attempt.taskId &&
    task.id === attempt.taskId &&
    task.revision === attempt.revision &&
    task.attempt.sequence === attempt.sequence
  )
}

export function markDocumentItemRunning<T>(
  task: DocumentTask<T>,
  attempt: DocumentAttempt,
  sourceId: string
): DocumentTask<T> {
  if (!currentAttempt(task, attempt)) return task
  return updateItem(task, sourceId, (item) => (item.state === 'queued' ? { ...item, state: 'running' } : item))
}

export function publishDocumentCandidate<T>(
  task: DocumentTask<T>,
  attempt: DocumentAttempt,
  sourceId: string,
  value: T
): DocumentTask<T> {
  if (!currentAttempt(task, attempt)) return task
  const changed = updateItem(task, sourceId, (item) =>
    item.state === 'running'
      ? {
          ...item,
          state: 'review',
          candidate: Object.freeze({
            revision: attempt.revision,
            sequence: attempt.sequence,
            value
          })
        }
      : item
  )
  return settleAttempt(changed)
}

export function failDocumentItem<T>(
  task: DocumentTask<T>,
  attempt: DocumentAttempt,
  sourceId: string,
  failure: string
): DocumentTask<T> {
  if (!currentAttempt(task, attempt)) return task
  const changed = updateItem(task, sourceId, (item) =>
    item.state === 'running' || item.state === 'queued'
      ? {
          ...item,
          state: 'failed',
          candidate: undefined,
          failure
        }
      : item
  )
  return settleAttempt(changed)
}

/** Request and acknowledgement are separate: a requested cancel is not proof of stopped work. */
export function requestDocumentCancellation<T>(task: DocumentTask<T>): DocumentTask<T> {
  if (!task.attempt) return task
  return freezeTask({
    ...task,
    items: task.items.map((item) =>
      item.state === 'running' || item.state === 'queued' ? { ...item, state: 'cancelling' } : item
    )
  })
}

export function acknowledgeDocumentCancellation<T>(task: DocumentTask<T>, attempt: DocumentAttempt): DocumentTask<T> {
  if (!currentAttempt(task, attempt) || !task.items.some((item) => item.state === 'cancelling')) return task
  return freezeTask({
    ...task,
    attempt: undefined,
    items: task.items.map((item) => (item.state === 'cancelling' ? { ...item, state: 'cancelled' } : item))
  })
}

export function acceptDocumentCandidate<T>(
  task: DocumentTask<T>,
  sourceId: string,
  expected: DocumentCandidate<T>
): DocumentTask<T> {
  const item = task.items.find((row) => row.source.id === sourceId)
  if (item?.accepted === expected && item.state === 'accepted' && expected.revision === task.revision) return task
  if (!item || item.state !== 'review' || item.candidate !== expected || expected.revision !== task.revision) {
    throw new DocumentTaskError('stale-candidate')
  }
  return updateItem(task, sourceId, (row) => ({
    ...row,
    state: 'accepted',
    accepted: expected,
    candidate: undefined,
    saveState: 'unsaved',
    receipt: undefined
  }))
}

export type DocumentSaveAttempt<T> = Readonly<{
  taskId: string
  sourceId: string
  sequence: number
  artifact: DocumentCandidate<T>
}>

export function startDocumentSave<T>(
  task: DocumentTask<T>,
  sourceId: string
): { task: DocumentTask<T>; save: DocumentSaveAttempt<T> } {
  const item = task.items.find((row) => row.source.id === sourceId)
  if (!item || item.state !== 'accepted' || !item.accepted || item.accepted.revision !== task.revision)
    throw new DocumentTaskError('stale-candidate')
  if (item.saveState === 'saving') throw new DocumentTaskError('busy')
  const sequence = item.saveSequence + 1
  const save = Object.freeze({ taskId: task.id, sourceId, sequence, artifact: item.accepted })
  return {
    task: updateItem(task, sourceId, (row) => ({
      ...row,
      saveState: 'saving',
      saveSequence: sequence,
      failure: undefined
    })),
    save
  }
}

function currentSave<T>(task: DocumentTask<T>, save: DocumentSaveAttempt<T>): boolean {
  const item = task.items.find((row) => row.source.id === save.sourceId)
  return (
    task.id === save.taskId &&
    item?.accepted === save.artifact &&
    item.saveSequence === save.sequence &&
    item.saveState === 'saving'
  )
}

export function finishDocumentSave<T>(
  task: DocumentTask<T>,
  save: DocumentSaveAttempt<T>,
  receipt: DocumentSaveReceipt
): DocumentTask<T> {
  if (!currentSave(task, save)) return task
  if (!receipt.destination.trim() || !Number.isFinite(Date.parse(receipt.savedAt)))
    throw new DocumentTaskError('invalid-receipt')
  return updateItem(task, save.sourceId, (row) => ({
    ...row,
    saveState: 'saved',
    receipt: Object.freeze({ ...receipt })
  }))
}

export function failDocumentSave<T>(
  task: DocumentTask<T>,
  save: DocumentSaveAttempt<T>,
  failure: string
): DocumentTask<T> {
  if (!currentSave(task, save)) return task
  return updateItem(task, save.sourceId, (row) => ({ ...row, saveState: 'failed', failure }))
}

export function documentTaskProgress<T>(task: DocumentTask<T>): {
  total: number
  completed: number
  failed: number
  reviewing: number
  accepted: number
  cancelling: number
} {
  const admitted = task.items.filter((item) => item.state !== 'rejected')
  return {
    total: admitted.length,
    completed: admitted.filter((item) => ['review', 'accepted', 'failed', 'cancelled'].includes(item.state)).length,
    failed: admitted.filter((item) => item.state === 'failed').length,
    reviewing: admitted.filter((item) => item.state === 'review').length,
    accepted: admitted.filter((item) => item.state === 'accepted').length,
    cancelling: admitted.filter((item) => item.state === 'cancelling').length
  }
}

function updateItem<T>(
  task: DocumentTask<T>,
  sourceId: string,
  update: (item: DocumentTaskItem<T>) => DocumentTaskItem<T>
): DocumentTask<T> {
  let changed = false
  const items = task.items.map((item) => {
    if (item.source.id !== sourceId) return item
    const next = update(item)
    if (next !== item) changed = true
    return next
  })
  return changed ? freezeTask({ ...task, items }) : task
}

function settleAttempt<T>(task: DocumentTask<T>): DocumentTask<T> {
  return task.items.some((item) => ['queued', 'running', 'cancelling'].includes(item.state))
    ? task
    : freezeTask({ ...task, attempt: undefined })
}

function freezeTask<T>(task: DocumentTask<T>): DocumentTask<T> {
  return Object.freeze({
    ...task,
    items: Object.freeze(task.items.map((item) => Object.freeze(item)))
  })
}
