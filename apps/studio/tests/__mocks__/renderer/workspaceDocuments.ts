import type { WorkspaceDocumentRequest, WorkspaceDocumentSnapshot } from '@shared/types/workspaceDocument'

type DiskResponse = Pick<Response, 'ok'> &
  Partial<Pick<Response, 'status' | 'arrayBuffer'>> & { headers?: Pick<Headers, 'get'> }
type DiskFetch = (url: string, init?: RequestInit) => Promise<DiskResponse>
type Model = { snapshot: WorkspaceDocumentSnapshot; root: string; path: string; saved: string; revision: string }

/** Wire fixture for existing UI regressions. Native authority proofs use the Rust subprocess. */
export function workspaceDocumentsFixture(diskFetch: DiskFetch) {
  const models = new Map<string, Model>()
  const leases = new Map<string, Model>()
  let identifier = 0
  let tail: Promise<unknown> = Promise.resolve()
  const response = (value: unknown, status = 200) => new Response(JSON.stringify(value), { status })
  const url = (root: string, path: string) =>
    `http://127.0.0.1:9/api/workspace-file/snapshot?${new URLSearchParams({ root, path })}`
  const read = async (root: string, path: string) => {
    const disk = await diskFetch(url(root, path))
    if (!disk.ok || !disk.arrayBuffer) throw new Error('Fixture document missing')
    const text = new TextDecoder('utf-8', { fatal: true }).decode(await disk.arrayBuffer())
    const revision = disk.headers?.get('etag')
    if (!revision) throw new Error('Fixture snapshot missing')
    return { text, revision }
  }
  const dispatch = async (init?: RequestInit) => {
    const command = JSON.parse(String(init?.body)) as WorkspaceDocumentRequest
    if (command.operation === 'open') {
      const key = JSON.stringify([command.root, command.path])
      let model = models.get(key)
      if (!model) {
        const disk = await read(command.root, command.path)
        model = {
          root: command.root,
          path: command.path,
          saved: disk.text,
          revision: disk.revision,
          snapshot: {
            documentId: `fixture-document-${++identifier}`,
            uri: `file://${command.path}`,
            version: 1,
            text: disk.text,
            dirty: false,
            diskConflict: false
          }
        }
        models.set(key, model)
      }
      const lease = `fixture-lease-${++identifier}`
      leases.set(lease, model)
      return response({ lease, snapshot: model.snapshot })
    }
    const model = leases.get(command.lease)
    if (!model) return response({}, 404)
    if (command.operation === 'recoveries') return response([])
    if (
      command.operation === 'review_recovery' ||
      command.operation === 'resolve_recovery' ||
      command.operation === 'delegate' ||
      command.operation === 'delegate_diagnostics' ||
      command.operation === 'publish_diagnostics'
    )
      return response({}, 404)
    if (command.operation === 'read') return response(model.snapshot)
    if (command.operation === 'diagnostics') return response({ snapshot: model.snapshot, diagnostics: [] })
    if (command.operation === 'watch')
      return response(
        command.version === model.snapshot.version && command.diskConflict === model.snapshot.diskConflict
          ? { changed: false }
          : { changed: true, snapshot: model.snapshot }
      )
    if (command.operation === 'close') {
      leases.delete(command.lease)
      if (!model.snapshot.dirty && !model.snapshot.diskConflict && ![...leases.values()].includes(model))
        models.delete(JSON.stringify([model.root, model.path]))
      return response({ closed: true })
    }
    if (command.version !== model.snapshot.version) return response({}, 409)
    if (command.operation === 'edit' || command.operation === 'input') {
      model.snapshot = {
        ...model.snapshot,
        text: command.text,
        dirty: command.text !== model.saved,
        version: model.snapshot.version + 1
      }
    } else if (command.operation === 'save') {
      if (!model.snapshot.dirty && !model.snapshot.diskConflict) {
        model.snapshot = { ...model.snapshot, version: model.snapshot.version + 1 }
        const { version, dirty, diskConflict } = model.snapshot
        return response({ version, dirty, diskConflict, retained: false })
      }
      const saved = await diskFetch(url(model.root, model.path), {
        method: 'PUT',
        headers: { 'If-Match': model.revision },
        body: new TextEncoder().encode(model.snapshot.text)
      })
      if (!saved.ok) {
        model.snapshot = { ...model.snapshot, diskConflict: true }
        return response({}, saved.status || 500)
      }
      const revision = saved.headers?.get('etag')
      if (!revision) throw new Error('Fixture saved snapshot missing')
      model.saved = model.snapshot.text
      model.revision = revision
      model.snapshot = { ...model.snapshot, dirty: false, diskConflict: false, version: model.snapshot.version + 1 }
    } else {
      const disk = await read(model.root, model.path)
      model.saved = disk.text
      model.revision = disk.revision
      model.snapshot = {
        ...model.snapshot,
        text: disk.text,
        dirty: false,
        diskConflict: false,
        version: model.snapshot.version + 1
      }
    }
    const { version, dirty, diskConflict } = model.snapshot
    return response({ version, dirty, diskConflict, retained: false })
  }
  return (init?: RequestInit): Promise<Response> => {
    const next = tail.then(() => dispatch(init))
    tail = next.catch(() => undefined)
    return next
  }
}
