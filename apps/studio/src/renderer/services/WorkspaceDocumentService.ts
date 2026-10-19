import type { PluginActivationContext } from '@renderer/types/workbenchPlugin'
import { readStudioRuntime, type StudioRuntimeDescriptor } from '@shared/studioRuntime'
import type {
  WorkspaceDiagnosticObservation,
  WorkspaceDiagnosticPublication,
  WorkspaceDocumentDiagnostic,
  WorkspaceDocumentLease,
  WorkspaceDocumentPermissions,
  WorkspaceDocumentReceipt,
  WorkspaceDocumentRecovery,
  WorkspaceDocumentRecoveryReview,
  WorkspaceDocumentRequest,
  WorkspaceDocumentSnapshot
} from '@shared/types/workspaceDocument'

import { diagnosticPublication } from './workspaceDiagnostics'

class ObservationChanged extends Error {}

function record(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Invalid native document response')
  return value as Record<string, unknown>
}

function flags(value: unknown): Omit<WorkspaceDocumentReceipt, 'retained'> {
  const row = record(value)
  if (
    !Number.isSafeInteger(row.version) ||
    Number(row.version) < 1 ||
    typeof row.dirty !== 'boolean' ||
    typeof row.diskConflict !== 'boolean'
  )
    throw new Error('Invalid native document response')
  return Object.freeze({ version: Number(row.version), dirty: row.dirty, diskConflict: row.diskConflict })
}

function receipt(value: unknown): WorkspaceDocumentReceipt {
  const row = record(value)
  if (typeof row.retained !== 'boolean') throw new Error('Invalid native document receipt')
  return Object.freeze({ ...flags(row), retained: row.retained })
}

function recovery(value: unknown): WorkspaceDocumentRecovery {
  const row = record(value)
  if (
    typeof row.id !== 'string' ||
    !row.id ||
    !Number.isSafeInteger(row.revision) ||
    Number(row.revision) < 1 ||
    !Number.isSafeInteger(row.baseVersion) ||
    Number(row.baseVersion) < 1 ||
    !Number.isSafeInteger(row.bytes) ||
    Number(row.bytes) < 0 ||
    Number(row.bytes) > 20 * 1024 * 1024 ||
    typeof row.canResolve !== 'boolean'
  )
    throw new Error('Invalid native document recovery')
  return Object.freeze({
    id: row.id,
    revision: Number(row.revision),
    baseVersion: Number(row.baseVersion),
    bytes: Number(row.bytes),
    canResolve: row.canResolve
  })
}

function snapshot(value: unknown): WorkspaceDocumentSnapshot {
  const row = record(value)
  if (
    typeof row.documentId !== 'string' ||
    !row.documentId ||
    typeof row.uri !== 'string' ||
    !row.uri.startsWith('file://') ||
    typeof row.text !== 'string'
  )
    throw new Error('Invalid native document response')
  return Object.freeze({ documentId: row.documentId, uri: row.uri, text: row.text, ...flags(row) })
}

function diagnosticObservation(value: unknown): WorkspaceDiagnosticObservation {
  const row = record(value)
  const model = snapshot(row.snapshot)
  if (!Array.isArray(row.diagnostics) || row.diagnostics.length > 16) throw new Error('Invalid native diagnostics')
  const diagnostics = Object.freeze(row.diagnostics.map((item) => diagnosticPublication(item, model)))
  if (
    new Set(diagnostics.map((item) => item.publisherId)).size !== diagnostics.length ||
    new globalThis.TextEncoder().encode(JSON.stringify(diagnostics)).length > 96 * 1024
  )
    throw new Error('Invalid native diagnostic publishers')
  return Object.freeze({ snapshot: model, diagnostics })
}

async function request(
  runtime: StudioRuntimeDescriptor,
  command: WorkspaceDocumentRequest,
  path = '/api/workspace-documents'
): Promise<unknown> {
  const response = await globalThis.fetch(`${runtime.apiBaseUrl}${path}`, {
    method: 'POST',
    headers: { Authorization: `Bearer ${runtime.token}`, 'Content-Type': 'application/json' },
    body: JSON.stringify(command),
    signal: globalThis.AbortSignal.timeout(30_000)
  })
  if (response.status === 404 && path === '/api/plugin-documents' && command.operation === 'close')
    return { closed: true }
  if (!response.ok) throw new Error(`Native document request refused (${response.status})`)
  return response.json()
}

/** One editor's access lease. The native model can outlive this renderer owner. */
export class WorkspaceDocumentClient {
  private tail: Promise<unknown> = Promise.resolve()
  private closing?: Promise<void>
  private stopped = false
  private sequence = 0
  private revision = 0
  private failure?: Error
  private desired: string
  private retainedText?: string
  private readonly reviews = new WeakMap<WorkspaceDocumentRecoveryReview, number>()

  private constructor(
    private readonly runtime: StudioRuntimeDescriptor,
    private readonly lease: string,
    private accepted: WorkspaceDocumentSnapshot
  ) {
    this.desired = accepted.text
  }

  static async open(root: string, path: string): Promise<WorkspaceDocumentClient> {
    const runtime = readStudioRuntime()
    if (!runtime) throw new Error('Native documents unavailable')
    const value = record(await request(runtime, { operation: 'open', root, path }))
    if (typeof value.lease !== 'string' || !value.lease || value.lease.length > 128)
      throw new Error('Invalid native document lease')
    try {
      const opened: WorkspaceDocumentLease = { lease: value.lease, snapshot: snapshot(value.snapshot) }
      return new WorkspaceDocumentClient(runtime, opened.lease, opened.snapshot)
    } catch (error) {
      await request(runtime, { operation: 'close', lease: value.lease })
      throw error
    }
  }

  get observation(): WorkspaceDocumentSnapshot {
    return this.accepted
  }
  get text(): string {
    return this.desired
  }

  delegate(context: PluginActivationContext, permissions: WorkspaceDocumentPermissions): Promise<PluginDocumentClient> {
    const approved = Object.freeze({ edit: permissions.edit, save: permissions.save })
    return this.delegateClient(
      context,
      { operation: 'delegate', lease: this.lease, permissions: approved },
      approved,
      false
    )
  }

  delegateDiagnostics(context: PluginActivationContext): Promise<PluginDocumentClient> {
    return this.delegateClient(
      context,
      { operation: 'delegate_diagnostics', lease: this.lease },
      { edit: false, save: false },
      true
    )
  }

  private delegateClient(
    context: PluginActivationContext,
    command: WorkspaceDocumentRequest,
    approved: WorkspaceDocumentPermissions,
    diagnostics: boolean
  ): Promise<PluginDocumentClient> {
    return this.enqueue(async () => {
      if (!context.isCurrent()) throw new ObservationChanged('Plugin document owner closed')
      const value = record(await request(this.runtime, command))
      if (typeof value.lease !== 'string' || !/^[a-f0-9]{64}$/.test(value.lease))
        throw new Error('Invalid native document delegation')
      const runtime = { apiBaseUrl: this.runtime.apiBaseUrl, token: value.lease }
      try {
        const opened = snapshot(value.snapshot)
        if (
          opened.documentId !== this.accepted.documentId ||
          opened.uri !== this.accepted.uri ||
          opened.version < this.accepted.version
        )
          throw new Error('Invalid native document delegation')
        return await PluginDocumentClient.owned(runtime, opened, context, approved, diagnostics)
      } catch (error) {
        await request(runtime, { operation: 'close', lease: runtime.token }, '/api/plugin-documents')
        throw error
      }
    })
  }

  replace(text: string): Promise<WorkspaceDocumentSnapshot> {
    if (this.stopped) return Promise.reject(new Error('Native document lease closed'))
    this.revision++
    this.desired = text
    return this.enqueue(
      async () => {
        const next = receipt(
          await request(this.runtime, {
            operation: 'input',
            lease: this.lease,
            sequence: ++this.sequence,
            version: this.accepted.version,
            text
          })
        )
        if (next.retained) {
          if (next.version < this.accepted.version) throw new Error('Invalid native input receipt')
          this.retainedText = text
          throw new Error('Document changed; user input retained')
        }
        this.advance(next, text)
        this.retainedText = undefined
        this.failure = undefined
        return this.accepted
      },
      true,
      true
    )
  }

  recoveries(): Promise<readonly WorkspaceDocumentRecovery[]> {
    return this.enqueue(
      async () => {
        const value = await request(this.runtime, { operation: 'recoveries', lease: this.lease })
        if (!Array.isArray(value) || value.length > 128) throw new Error('Invalid native document recoveries')
        return Object.freeze(value.map(recovery))
      },
      false,
      true
    )
  }

  reviewRecovery(id: string): Promise<WorkspaceDocumentRecoveryReview> {
    const revision = this.revision
    return this.enqueue(
      async () => {
        if (revision !== this.revision) throw new ObservationChanged('Native document review changed')
        const value = record(await request(this.runtime, { operation: 'review_recovery', lease: this.lease, id }))
        const model = snapshot(value.snapshot)
        const draft = recovery(value.recovery)
        if (
          model.documentId !== this.accepted.documentId ||
          model.uri !== this.accepted.uri ||
          model.version < this.accepted.version ||
          draft.id !== id ||
          typeof value.text !== 'string'
        )
          throw new Error('Invalid native document review')
        const review = Object.freeze({ snapshot: model, recovery: draft, text: value.text })
        this.reviews.set(review, revision)
        return review
      },
      false,
      true
    )
  }

  resolveRecovery(review: WorkspaceDocumentRecoveryReview, restore: boolean): Promise<WorkspaceDocumentSnapshot> {
    const revision = this.revision
    return this.enqueue(
      async () => {
        if (!review.recovery.canResolve || this.reviews.get(review) !== revision || revision !== this.revision)
          throw new ObservationChanged('Native document recovery is stale')
        const localOnly = !restore && review.recovery.id !== this.lease && this.desired !== this.accepted.text
        const result = receipt(
          await request(this.runtime, {
            operation: 'resolve_recovery',
            lease: this.lease,
            sequence: ++this.sequence,
            version: review.snapshot.version,
            id: review.recovery.id,
            revision: review.recovery.revision,
            restore
          })
        )
        if (result.retained || result.version !== review.snapshot.version + 1)
          throw new Error('Invalid native recovery receipt')
        this.reviews.delete(review)
        const next = snapshot(await request(this.runtime, { operation: 'read', lease: this.lease }))
        if (
          next.documentId !== this.accepted.documentId ||
          next.uri !== this.accepted.uri ||
          next.version < result.version
        )
          throw new Error('Invalid native document recovery')
        if (!localOnly) {
          this.accepted = next
          if (revision === this.revision) {
            this.desired = next.text
            this.revision++
            this.retainedText = undefined
          }
          this.failure = undefined
        }
        return next
      },
      false,
      true
    )
  }

  async read(): Promise<WorkspaceDocumentSnapshot> {
    const revision = this.revision
    return this.enqueue(async () => {
      const next = snapshot(await request(this.runtime, { operation: 'read', lease: this.lease }))
      if (
        revision !== this.revision ||
        next.documentId !== this.accepted.documentId ||
        next.uri !== this.accepted.uri ||
        next.version < this.accepted.version
      )
        throw new ObservationChanged('Native document observation changed')
      this.accepted = next
      this.desired = next.text
      return next
    })
  }

  observe(): Promise<WorkspaceDocumentSnapshot | undefined> {
    const revision = this.revision
    return this.enqueue(async () => {
      if (revision !== this.revision || this.desired !== this.accepted.text) return undefined
      const value = record(
        await request(this.runtime, {
          operation: 'watch',
          lease: this.lease,
          version: this.accepted.version,
          diskConflict: this.accepted.diskConflict
        })
      )
      if (value.changed === false) return undefined
      if (value.changed !== true) throw new Error('Invalid native document observation')
      const next = snapshot(value.snapshot)
      if (
        next.documentId !== this.accepted.documentId ||
        next.uri !== this.accepted.uri ||
        next.version < this.accepted.version
      )
        throw new Error('Invalid native document observation')
      if (revision !== this.revision) return undefined
      this.accepted = next
      this.desired = next.text
      return next
    })
  }

  diagnostics(): Promise<WorkspaceDiagnosticObservation | undefined> {
    const revision = this.revision
    const expected = this.accepted
    return this.enqueue(async () => {
      if (
        revision !== this.revision ||
        expected !== this.accepted ||
        this.desired !== expected.text ||
        expected.text.length > 64 * 1024 ||
        new globalThis.TextEncoder().encode(expected.text).length > 64 * 1024
      )
        return undefined
      try {
        const observed = diagnosticObservation(
          await request(this.runtime, { operation: 'diagnostics', lease: this.lease })
        )
        const model = observed.snapshot
        if (
          model.documentId !== expected.documentId ||
          model.uri !== expected.uri ||
          model.version < expected.version ||
          (model.version === expected.version && model.text !== expected.text)
        )
          throw new Error('Invalid native diagnostic document')
        if (
          this.stopped ||
          revision !== this.revision ||
          expected !== this.accepted ||
          model.version !== expected.version
        )
          return undefined
        return observed
      } catch {
        throw new ObservationChanged('Native document diagnostics unavailable')
      }
    })
  }

  apply(expected: WorkspaceDocumentSnapshot, replacement: string): Promise<WorkspaceDocumentSnapshot> {
    if (expected !== this.accepted || this.desired !== expected.text || this.stopped)
      return Promise.reject(new ObservationChanged('Native document edit is stale'))
    const revision = this.revision
    return this.enqueue(async () => {
      if (revision !== this.revision || expected !== this.accepted)
        throw new ObservationChanged('Native document edit is stale')
      const next = receipt(
        await request(this.runtime, {
          operation: 'edit',
          lease: this.lease,
          sequence: ++this.sequence,
          version: expected.version,
          text: replacement
        })
      )
      this.advance(next, replacement)
      if (revision === this.revision) this.desired = replacement
      return this.accepted
    })
  }

  save(): Promise<WorkspaceDocumentSnapshot> {
    return this.enqueue(async () => {
      const next = receipt(
        await request(this.runtime, {
          operation: 'save',
          lease: this.lease,
          sequence: ++this.sequence,
          version: this.accepted.version
        })
      )
      this.advance(next, this.accepted.text)
      return this.accepted
    })
  }

  discard(): Promise<WorkspaceDocumentSnapshot> {
    return this.enqueue(
      async () => {
        const revision = this.revision
        receipt(
          await request(this.runtime, {
            operation: 'discard',
            lease: this.lease,
            sequence: ++this.sequence,
            version: this.accepted.version
          })
        )
        const next = snapshot(await request(this.runtime, { operation: 'read', lease: this.lease }))
        if (
          next.documentId !== this.accepted.documentId ||
          next.uri !== this.accepted.uri ||
          next.version <= this.accepted.version
        )
          throw new Error('Native document observation changed')
        this.accepted = next
        if (revision === this.revision) {
          this.desired = next.text
          this.revision++
          this.retainedText = undefined
        }
        this.failure = undefined
        return next
      },
      false,
      true
    )
  }

  close(): Promise<void> {
    if (this.closing) return this.closing
    this.stopped = true
    this.closing = this.tail.then(async () => {
      const result = record(await request(this.runtime, { operation: 'close', lease: this.lease }))
      if (result.closed !== true) throw new Error('Native document close not acknowledged')
      if (this.desired !== this.accepted.text && this.desired !== this.retainedText)
        throw new Error('Pending user input was not retained')
    })
    return this.closing
  }

  private advance(next: WorkspaceDocumentReceipt, text: string): void {
    if (next.retained || next.version !== this.accepted.version + 1)
      throw new Error('Invalid native document mutation receipt')
    this.accepted = Object.freeze({ ...this.accepted, ...flags(next), text })
  }

  private enqueue<T>(operation: () => Promise<T>, drain = false, recover = false): Promise<T> {
    if (this.stopped) return Promise.reject(new Error('Native document lease closed'))
    const next = this.tail.then(() => {
      if (this.stopped && !drain) throw new Error('Native document lease closed')
      if (this.failure && !recover) throw this.failure
      return operation()
    })
    this.tail = next.then(
      () => undefined,
      (error: unknown) => {
        if (!this.stopped && !(error instanceof ObservationChanged))
          this.failure = error instanceof Error ? error : new Error(String(error))
      }
    )
    return next
  }
}

/** One scoped document capability; its transport never carries the shell bearer. */
export class PluginDocumentClient {
  private tail: Promise<unknown> = Promise.resolve()
  private sequence = 0
  private diagnosticSequence = 0
  private stopped = false
  private closing?: Promise<void>

  private constructor(
    private readonly runtime: StudioRuntimeDescriptor,
    private accepted: WorkspaceDocumentSnapshot,
    private readonly context: PluginActivationContext,
    private readonly permissions: WorkspaceDocumentPermissions,
    private readonly canPublishDiagnostics: boolean
  ) {}

  static async owned(
    runtime: StudioRuntimeDescriptor,
    observation: WorkspaceDocumentSnapshot,
    context: PluginActivationContext,
    permissions: WorkspaceDocumentPermissions,
    diagnostics = false
  ): Promise<PluginDocumentClient> {
    const client = new PluginDocumentClient(
      Object.freeze({ ...runtime }),
      observation,
      context,
      Object.freeze({ ...permissions }),
      diagnostics
    )
    try {
      if (!context.isCurrent()) throw new ObservationChanged('Plugin document owner closed')
      context.defer(() => client.close())
      return client
    } catch (error) {
      await client.close()
      throw error
    }
  }

  get observation(): WorkspaceDocumentSnapshot {
    return this.accepted
  }

  read(): Promise<WorkspaceDocumentSnapshot> {
    return this.enqueue(async () => {
      const next = snapshot(await this.command({ operation: 'read', lease: this.runtime.token }))
      this.adopt(next)
      return next
    })
  }

  diagnostics(): Promise<WorkspaceDiagnosticObservation> {
    return this.enqueue(async () => {
      const observed = diagnosticObservation(
        await this.command({ operation: 'diagnostics', lease: this.runtime.token })
      )
      this.adopt(observed.snapshot)
      return observed
    })
  }

  publishDiagnostics(
    expected: WorkspaceDocumentSnapshot,
    items: readonly WorkspaceDocumentDiagnostic[]
  ): Promise<WorkspaceDiagnosticPublication> {
    if (!this.canPublishDiagnostics || expected !== this.accepted)
      return Promise.reject(new ObservationChanged('Plugin diagnostic publication refused'))
    const frozen = items.map((item) =>
      Object.freeze({ ...item, start: Object.freeze({ ...item.start }), end: Object.freeze({ ...item.end }) })
    )
    return this.enqueue(async () => {
      if (expected !== this.accepted) throw new ObservationChanged('Plugin diagnostic observation is stale')
      const sequence = this.diagnosticSequence + 1
      const value = await this.command({
        operation: 'publish_diagnostics',
        lease: this.runtime.token,
        sequence,
        version: expected.version,
        items: frozen
      })
      const publication = diagnosticPublication(value, expected)
      if (!this.context.isCurrent() || this.stopped || publication.sequence !== sequence)
        throw new ObservationChanged('Plugin diagnostic publication not acknowledged')
      this.diagnosticSequence = sequence
      return publication
    })
  }

  apply(expected: WorkspaceDocumentSnapshot, replacement: string): Promise<WorkspaceDocumentSnapshot> {
    if (!this.permissions.edit || expected !== this.accepted)
      return Promise.reject(new ObservationChanged('Plugin document edit refused'))
    return this.enqueue(async () => {
      if (expected !== this.accepted) throw new ObservationChanged('Plugin document edit is stale')
      const next = receipt(
        await this.command({
          operation: 'edit',
          lease: this.runtime.token,
          sequence: ++this.sequence,
          version: expected.version,
          text: replacement
        })
      )
      if (next.retained || next.version !== expected.version + 1) throw new Error('Invalid plugin document receipt')
      this.adopt(Object.freeze({ ...expected, ...flags(next), text: replacement }))
      return this.accepted
    })
  }

  save(): Promise<WorkspaceDocumentSnapshot> {
    if (!this.permissions.save) return Promise.reject(new ObservationChanged('Plugin document save refused'))
    return this.enqueue(async () => {
      const previous = this.accepted
      const next = receipt(
        await this.command({
          operation: 'save',
          lease: this.runtime.token,
          sequence: ++this.sequence,
          version: previous.version
        })
      )
      if (next.retained || next.version !== previous.version + 1) throw new Error('Invalid plugin document receipt')
      this.adopt(Object.freeze({ ...previous, ...flags(next) }))
      return this.accepted
    })
  }

  close(): Promise<void> {
    if (!this.closing) {
      this.stopped = true
      this.closing = this.tail.then(async () => {
        const result = record(await this.command({ operation: 'close', lease: this.runtime.token }))
        if (result.closed !== true) throw new Error('Plugin document close not acknowledged')
      })
    }
    return this.closing
  }

  private command(command: WorkspaceDocumentRequest): Promise<unknown> {
    return request(this.runtime, command, '/api/plugin-documents')
  }

  private adopt(next: WorkspaceDocumentSnapshot): void {
    if (!this.context.isCurrent() || this.stopped) throw new ObservationChanged('Plugin document owner closed')
    if (
      next.documentId !== this.accepted.documentId ||
      next.uri !== this.accepted.uri ||
      next.version < this.accepted.version ||
      (next.version === this.accepted.version && next.text !== this.accepted.text)
    )
      throw new Error('Invalid plugin document observation')
    this.accepted = next
  }

  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    if (this.stopped || !this.context.isCurrent())
      return Promise.reject(new ObservationChanged('Plugin document owner closed'))
    const next = this.tail.then(() => {
      if (this.stopped || !this.context.isCurrent()) throw new ObservationChanged('Plugin document owner closed')
      return operation()
    })
    this.tail = next.then(
      () => undefined,
      () => undefined
    )
    return next
  }
}
