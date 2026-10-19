import {
  acceptDocumentCandidate,
  appendDocumentSources,
  removeDocumentSources,
  reorderDocumentSources,
  acknowledgeDocumentCancellation,
  failDocumentItem,
  failDocumentSave,
  finishDocumentSave,
  markDocumentItemRunning,
  publishDocumentCandidate,
  requestDocumentCancellation,
  reviseDocumentTask,
  reviseDocumentItems,
  startDocumentAttempt,
  startDocumentSave
} from './task'
import type { DocumentCandidate, DocumentSaveReceipt, DocumentSource, DocumentTask } from './task'

export type DocumentEngine<T> = (
  source: DocumentSource,
  signal: InstanceType<typeof globalThis.AbortSignal>
) => Promise<T>
export type DocumentWriter<T> = (value: T) => Promise<DocumentSaveReceipt>

/** One bounded queue per window. Bytes and credentials remain in the injected engine/writer. */
export class DocumentTaskApplication<T> {
  private snapshot: DocumentTask<T>
  private readonly listeners = new Set<() => void>()
  private controller?: InstanceType<typeof globalThis.AbortController>
  private disposed = false

  constructor(
    task: DocumentTask<T>,
    private engine: DocumentEngine<T>,
    private readonly automatic = false
  ) {
    this.snapshot = task
  }

  getSnapshot = (): DocumentTask<T> => this.snapshot

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => this.listeners.delete(listener)
  }

  private publish(task: DocumentTask<T>): void {
    if (this.disposed || task === this.snapshot) return
    this.snapshot = task
    for (const listener of this.listeners) listener()
  }

  /** All selected work is sequential; cancellation waits for the active engine to settle. */
  async run(selectedIds: readonly string[]): Promise<void> {
    if (this.disposed) return
    const started = startDocumentAttempt(this.snapshot, selectedIds)
    const attempt = started.attempt
    if (!attempt) return
    const controller = new globalThis.AbortController()
    this.controller = controller
    this.publish(started)
    try {
      for (const item of started.items) {
        if (controller.signal.aborted || this.disposed) break
        if (item.state !== 'queued') continue
        this.publish(markDocumentItemRunning(this.snapshot, attempt, item.source.id))
        try {
          const value = await this.engine(item.source, controller.signal)
          if (controller.signal.aborted || this.disposed) break
          const completed = publishDocumentCandidate(this.snapshot, attempt, item.source.id, value)
          const candidate = completed.items.find((entry) => entry.source.id === item.source.id)?.candidate
          this.publish(
            this.automatic && candidate ? acceptDocumentCandidate(completed, item.source.id, candidate) : completed
          )
        } catch (error) {
          if (controller.signal.aborted || this.disposed) break
          this.publish(failDocumentItem(this.snapshot, attempt, item.source.id, failureOf(error)))
        }
      }
    } finally {
      if (controller.signal.aborted) this.publish(acknowledgeDocumentCancellation(this.snapshot, attempt))
      if (this.controller === controller) this.controller = undefined
    }
  }

  cancel(): void {
    this.publish(requestDocumentCancellation(this.snapshot))
    this.controller?.abort()
  }

  /** Accept only the exact candidate the review surface rendered; never call the engine here. */
  accept(sourceId: string, candidate: DocumentCandidate<T>): void {
    this.publish(acceptDocumentCandidate(this.snapshot, sourceId, candidate))
  }

  revise(engine: DocumentEngine<T>, sourceIds?: readonly string[]): void {
    const revised = sourceIds ? reviseDocumentItems(this.snapshot, sourceIds) : reviseDocumentTask(this.snapshot)
    this.engine = engine
    this.publish(revised)
  }

  append(sources: readonly DocumentSource[]): void {
    if (this.disposed) return
    this.publish(appendDocumentSources(this.snapshot, sources))
  }

  remove(sourceIds: readonly string[]): void {
    if (this.disposed) return
    this.publish(removeDocumentSources(this.snapshot, sourceIds))
  }

  reorder(sourceIds: readonly string[]): void {
    if (this.disposed) return
    this.publish(reorderDocumentSources(this.snapshot, sourceIds))
  }

  async save(sourceId: string, writer: DocumentWriter<T>): Promise<void> {
    const { task, save } = startDocumentSave(this.snapshot, sourceId)
    this.publish(task)
    try {
      const receipt = await writer(save.artifact.value)
      this.publish(finishDocumentSave(this.snapshot, save, receipt))
    } catch (error) {
      this.publish(failDocumentSave(this.snapshot, save, failureOf(error)))
    }
  }

  dispose(): void {
    this.cancel()
    this.disposed = true
    this.listeners.clear()
  }
}

function failureOf(error: unknown): string {
  return error instanceof Error && error.message.trim() ? error.message : 'workbench.feature.pdf.tools.job.failed'
}
