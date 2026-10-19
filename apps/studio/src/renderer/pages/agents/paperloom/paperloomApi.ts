import * as z from 'zod'

import { readStudioRuntime } from '@shared/studioRuntime'
import { workbenchProofSchema, type WorkbenchProof } from '@shared/types/workbenchProof'

import type { WorkspacePaperSnapshot } from './paperloomSource'

export const PAPERLOOM_PACKAGE = 'use/a3s/paperloom'
export const PAPERLOOM_SLUG = 'a3s-paperloom'
export const PAPERLOOM_OPEN = 'studio:paperloom-open'
export const PAPERLOOM_UPLOAD_LIMIT = 20 * 1024 * 1024

export interface PaperloomWorkspaceSource {
  root: string
  paths: readonly string[]
}

export interface PaperloomOpenRequest {
  requestId: string
  question: string
  source?: PaperloomWorkspaceSource
  onSourceReady?: () => void
}

export function openPaperloom(question = '', source?: PaperloomWorkspaceSource, onSourceReady?: () => void) {
  const event = new CustomEvent<PaperloomOpenRequest>(PAPERLOOM_OPEN, {
    cancelable: true,
    detail: { requestId: crypto.randomUUID(), question, source, onSourceReady }
  })
  window.dispatchEvent(event)
  return event.defaultPrevented
}

export const connectionSchema = z.object({
  endpoint: z.string(),
  apiKeyConfigured: z.boolean(),
  revision: z.string()
})
export type PaperloomConnection = z.infer<typeof connectionSchema>

const id = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[A-Za-z0-9_.-]+$/)
export const documentSchema = z.object({
  document_id: id,
  title: z.string(),
  source_filename: z.string(),
  page_count: z.number().int().nonnegative(),
  bytes: z.number().nonnegative(),
  active_job_id: id.nullish(),
  reading_status: z.enum(['unread', 'reading', 'done']),
  added_at: z.string(),
  updated_at: z.string(),
  tags: z.array(z.string()).default([])
})
export type Paper = z.infer<typeof documentSchema>
export const librarySchema = z.object({
  documents: z.array(documentSchema),
  total: z.number().int().nonnegative()
})
export const progressSchema = z.object({
  current: z.number().nullish(),
  total: z.number().nullish(),
  percent: z.number().nullish(),
  unit: z.string().nullish()
})
const stageSchema = z.object({
  state: z.enum(['reused', 'queued', 'pending', 'in_progress', 'completed', 'failed', 'skipped']),
  progress: progressSchema
})
export const jobSchema = z.object({
  job_id: id,
  display_name: z.string().optional(),
  workflow: z.enum(['book', 'ocr', 'translate', 'render']),
  status: z.enum(['queued', 'running', 'succeeded', 'failed', 'canceled']),
  attempt: z.number().int().optional(),
  stages: z.object({ ocr: stageSchema, translation: stageSchema, render: stageSchema }),
  output_pdf_ready: z.boolean().optional(),
  markdown_ready: z.boolean().optional(),
  bundle_ready: z.boolean().optional(),
  created_at: z.string().optional(),
  updated_at: z.string().optional()
})
export type PaperJob = z.infer<typeof jobSchema>
export const jobsSchema = z.object({ items: z.array(jobSchema) })
export class PaperloomError extends Error {
  constructor(
    readonly status: number,
    readonly code: string
  ) {
    super(code)
  }
}

export function createPaperloomClient(memberId: string | null) {
  async function paperloomResponse(path: string, init: RequestInit = {}): Promise<Response> {
    if (!memberId) throw new PaperloomError(401, 'member_required')
    const runtime = readStudioRuntime()
    if (!runtime) throw new PaperloomError(503, 'runtime_unavailable')
    const scoped = `${path}${path.includes('?') ? '&' : '?'}studio_member=${encodeURIComponent(memberId)}`
    const response = await fetch(`${runtime.apiBaseUrl}/api/paperloom/${scoped}`, {
      ...init,
      cache: 'no-store',
      headers: {
        ...Object.fromEntries(new Headers(init.headers)),
        authorization: `Bearer ${runtime.token}`
      }
    })
    if (!response.ok) {
      const data = await response.json().catch(() => null)
      throw new PaperloomError(
        response.status,
        typeof data?.error === 'string' ? data.error : `http_${response.status}`
      )
    }
    return response
  }

  async function paperloom<T>(path: string, schema: z.ZodType<T>, init: RequestInit = {}): Promise<T> {
    const response = await paperloomResponse(path, init)
    const value = await response.json()
    const data = path === 'connection' ? value : z.object({ code: z.literal(0), data: z.unknown() }).parse(value).data
    return schema.parse(data)
  }

  async function paperloomArtifactBytes(path: string, name: string, checksum?: string | null, signal?: AbortSignal) {
    if (!name || name.length > 240 || name === '.' || name === '..' || /[\\/\u0000-\u001f]/u.test(name))
      throw new PaperloomError(400, 'invalid_artifact_name')
    const response = await paperloomResponse(path, signal ? { signal } : {})
    const bytes = new Uint8Array(await response.arrayBuffer())
    if (bytes.length === 0) throw new PaperloomError(502, 'empty_artifact')
    if (checksum) {
      const expected = checksum.startsWith('sha256:') ? checksum.slice(7) : checksum
      const actual = [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))]
        .map((byte) => byte.toString(16).padStart(2, '0'))
        .join('')
      if (actual !== expected) throw new PaperloomError(409, 'artifact_changed')
    }
    return bytes
  }

  async function savePaperloomArtifact(path: string, name: string, checksum?: string | null) {
    return window.api.file.save(name, await paperloomArtifactBytes(path, name, checksum))
  }
  async function registerSavedArtifact(
    source: WorkspacePaperSnapshot,
    savedPath: string,
    artifact: { jobId: string; key: string; attempt: number; checksum: string }
  ) {
    const proof = await paperloom(
      'provenance',
      workbenchProofSchema,
      jsonBody({
        root: source.root,
        sourcePath: source.path,
        sourceRevision: source.revision,
        sourceDigest: source.digest,
        savedPath,
        jobId: artifact.jobId,
        artifactKey: artifact.key,
        attempt: artifact.attempt,
        checksum: artifact.checksum
      })
    )
    const version = proof.artifacts[0]
    if (
      proof.artifacts.length !== 1 ||
      version?.origin?.memberId !== memberId ||
      version.origin.sourceDigest !== `sha256:${source.digest}` ||
      version.origin.sourceRevision !== source.revision ||
      version.origin.jobId !== artifact.jobId ||
      version.origin.attempt !== artifact.attempt ||
      version.origin.artifactKey !== artifact.key ||
      version.contentDigest !== `sha256:${artifact.checksum.replace(/^sha256:/u, '')}`
    )
      throw new PaperloomError(409, 'provenance_invalid')
    return proof
  }

  async function lookupSourceProvenance(source: WorkspacePaperSnapshot) {
    const query = new URLSearchParams({
      root: source.root,
      sourcePath: source.path,
      sourceRevision: source.revision,
      sourceDigest: source.digest
    })
    const proofs = await paperloom(`provenance?${query}`, z.array(workbenchProofSchema))
    if (
      proofs.some(
        (proof) =>
          proof.artifacts.length !== 1 ||
          proof.artifacts[0].origin?.memberId !== memberId ||
          proof.artifacts[0].origin?.sourceRevision !== source.revision ||
          proof.artifacts[0].origin?.sourceDigest !== `sha256:${source.digest}`
      )
    )
      throw new PaperloomError(409, 'provenance_invalid')
    return proofs
  }

  function provenanceTransport(proof: WorkbenchProof) {
    const version = proof.artifacts[0]
    const origin = version?.origin
    if (!origin || origin.memberId !== memberId) throw new PaperloomError(404, 'not_found')
    const query = new URLSearchParams({ root: origin.workspaceRoot, path: version.path })
    return {
      loadProofs: async (_scope: string) => {
        const history = await paperloom(
          `provenance/${encodeURIComponent(proof.messageId)}/history?${query}`,
          z.array(workbenchProofSchema)
        )
        if (
          history.some(
            (item) =>
              item.sessionId !== proof.sessionId ||
              item.artifacts.length !== 1 ||
              item.artifacts[0]?.origin?.memberId !== memberId ||
              item.artifacts[0].path !== version.path
          )
        )
          throw new PaperloomError(409, 'provenance_invalid')
        return history
      },
      loadArtifact: async (_scope: string, id: string, signal: AbortSignal) => {
        const snapshot = await paperloom(
          `provenance/${encodeURIComponent(id)}/snapshot?${query}`,
          z.object({
            id: z.string(),
            name: z.string(),
            mediaType: z.string(),
            contentDigest: z.string(),
            data: z.string().max(30 * 1024 * 1024),
            text: z.string().optional()
          }),
          { signal }
        )
        if (snapshot.id !== id) throw new PaperloomError(409, 'provenance_invalid')
        return { ...snapshot, text: snapshot.text }
      }
    }
  }
  return {
    paperloom,
    paperloomResponse,
    paperloomArtifactBytes,
    savePaperloomArtifact,
    registerSavedArtifact,
    provenanceTransport,
    lookupSourceProvenance
  }
}

export function jsonBody(value: unknown, method = 'POST'): RequestInit {
  return { method, headers: { 'content-type': 'application/json' }, body: JSON.stringify(value) }
}

export async function validatePaperPdf(file: File) {
  if (
    !file.name.toLowerCase().endsWith('.pdf') ||
    file.size < 6 ||
    file.size > PAPERLOOM_UPLOAD_LIMIT ||
    new TextDecoder().decode(await file.slice(0, 5).arrayBuffer()) !== '%PDF-'
  ) {
    throw new PaperloomError(400, 'invalid_pdf')
  }
}
