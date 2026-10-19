import { dataApiService } from '@renderer/data/DataApiService'
import { readStudioRuntime } from '@shared/studioRuntime'
import {
  AixResourceRefusedError,
  aixDocumentProjectPageSchema,
  aixDocumentProjectSchema,
  aixDocumentReceiptSchema,
  aixDocumentSnapshotSchema,
  aixResourceLeaseSchema,
  aixDraftSnapshotSchema,
  type AixResourceOperation
} from '@shared/types/aixResource'

/** Fixed native routes only; the isolated document receives no bearer or filesystem root. */
export async function invokeAixResource(
  token: string,
  operation: AixResourceOperation,
  signal: AbortSignal
): Promise<unknown> {
  const runtime = readStudioRuntime()
  if (!runtime || !/^[a-f0-9]{64}$/.test(token) || signal.aborted) throw new AixResourceRefusedError('closed')
  if (operation.operation === 'workspaces') {
    const access = await fetch(`${runtime.apiBaseUrl}/api/aix/views/sessions/${token}/resources/access`, {
      signal,
      headers: { Authorization: `Bearer ${runtime.token}` }
    })
    if (!access.ok)
      throw new AixResourceRefusedError(access.status === 404 || access.status === 401 ? 'not_found' : 'unavailable')
    const permission: unknown = await access.json()
    if (!permission || typeof permission !== 'object' || !('canRead' in permission) || permission.canRead !== true)
      throw new AixResourceRefusedError('not_found')
    const workspaces = await dataApiService.get('/agent-workspaces')
    if (signal.aborted) throw new AixResourceRefusedError('closed')
    return workspaces.filter((workspace) => workspace.type === 'user').map(({ id, name }) => ({ id, name }))
  }
  const base = `/api/aix/views/sessions/${token}`
  let path = `${base}/resources`
  let method = 'POST'
  let body: unknown
  const { operation: kind, ...input } = operation
  switch (operation.operation) {
    case 'read-draft':
      path += `/drafts/${operation.key}`
      method = 'GET'
      break
    case 'save-draft':
      path += `/drafts/${operation.key}`
      method = 'PUT'
      body = {
        expectedRevision: operation.expectedRevision,
        operationId: operation.operationId,
        content: operation.content
      }
      break
    case 'projects':
      path = `${base}/projects?workspace=${operation.workspaceId}&page=${operation.page}&limit=${operation.limit}`
      method = 'GET'
      break
    case 'create-project':
      path = `${base}/projects`
      body = input
      break
    case 'open':
      path += '/leases'
      body = { protocolVersion: 1, ...input }
      break
    case 'close':
      path += `/leases/${operation.lease}`
      method = 'DELETE'
      break
    default: {
      const { lease, ...request } = operation
      body = { protocolVersion: 1, lease, request }
    }
  }
  const writes = ['create-project', 'save', 'store-asset', 'save-draft'].includes(kind)
  let response: Response
  try {
    response = await fetch(runtime.apiBaseUrl + path, {
      method,
      signal,
      headers: { Authorization: `Bearer ${runtime.token}`, 'Content-Type': 'application/json' },
      body: body === undefined ? undefined : JSON.stringify(body)
    })
  } catch {
    throw new AixResourceRefusedError(writes ? 'outcome_unknown' : 'unavailable')
  }
  if (!response.ok) {
    const reason =
      response.status === 404 || response.status === 401
        ? 'not_found'
        : response.status === 409
          ? 'conflict'
          : response.status === 400 || response.status === 422
            ? 'invalid_request'
            : writes
              ? 'outcome_unknown'
              : 'unavailable'
    throw new AixResourceRefusedError(reason)
  }
  const envelope: unknown = await response.json().catch(() => null)
  const value = envelope && typeof envelope === 'object' && 'data' in envelope ? envelope.data : envelope
  try {
    switch (operation.operation) {
      case 'read-draft':
      case 'save-draft':
        return aixDraftSnapshotSchema.parse(value)
      case 'projects':
        return aixDocumentProjectPageSchema.parse(value)
      case 'create-project': {
        const project = aixDocumentProjectSchema.parse(value)
        if (project.id !== operation.id) throw new Error('Project identity mismatch')
        return project
      }
      case 'open': {
        const lease = aixResourceLeaseSchema.parse(value)
        if (
          lease.workspaceId !== operation.workspaceId ||
          lease.projectId !== operation.projectId ||
          lease.access !== operation.access ||
          JSON.stringify([...lease.resources].sort()) !== JSON.stringify([...operation.resources].sort())
        )
          throw new Error('Lease identity mismatch')
        return lease
      }
      case 'read': {
        const snapshot = aixDocumentSnapshotSchema.parse(value)
        if (snapshot.key !== operation.key || (operation.revision !== null && snapshot.revision !== operation.revision))
          throw new Error('Document identity mismatch')
        return snapshot
      }
      case 'read-receipt':
      case 'save': {
        const receipt = aixDocumentReceiptSchema.parse(value)
        if (
          receipt.operationId !== operation.operationId ||
          (operation.operation === 'save' && receipt.document.key !== operation.key)
        )
          throw new Error('Receipt identity mismatch')
        return receipt
      }
      case 'close':
        return null
      default:
        return value
    }
  } catch {
    throw new AixResourceRefusedError(writes ? 'outcome_unknown' : 'unavailable')
  }
}
