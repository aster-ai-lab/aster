import { readLocalDocumentBytes } from '@renderer/pages/documents/useLocalDocuments'

import { PAPERLOOM_UPLOAD_LIMIT, PaperloomError, validatePaperPdf } from './paperloomApi'

export interface WorkspacePaperSnapshot {
  root: string
  path: string
  name: string
  revision: string
  digest: string
  bytes: Uint8Array
}

export async function readWorkspacePaper(
  root: string,
  path: string,
  signal?: AbortSignal
): Promise<WorkspacePaperSnapshot> {
  if (!root.trim() || !path.trim()) throw new PaperloomError(400, 'workspace_source_required')
  const name = path.replace(/\\/g, '/').split('/').at(-1) ?? ''
  if (!name.toLowerCase().endsWith('.pdf')) throw new PaperloomError(400, 'invalid_pdf')
  let revision = ''
  const bytes = await readLocalDocumentBytes(
    root,
    path,
    undefined,
    PAPERLOOM_UPLOAD_LIMIT,
    (value) => {
      revision = value
    },
    signal
  )
  await validatePaperPdf(new File([bytes.slice()], name, { type: 'application/pdf' }))
  const digest = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes.slice())), (value) =>
    value.toString(16).padStart(2, '0')
  ).join('')
  // Workspace revisions include file identity and metadata, not only content bytes.
  if (!revision) throw new PaperloomError(502, 'workspace_source_failed')
  return { root, path, name, revision, digest, bytes }
}

export async function verifyWorkspacePaper(source: WorkspacePaperSnapshot, signal?: AbortSignal): Promise<void> {
  const current = await readWorkspacePaper(source.root, source.path, signal)
  if (current.revision !== source.revision || current.digest !== source.digest)
    throw new PaperloomError(409, 'source_changed')
}
