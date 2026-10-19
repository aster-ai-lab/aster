import { File } from 'node:buffer'
import { createHash, webcrypto } from 'node:crypto'

import { afterEach, describe, expect, it, vi } from 'vitest'

import { makeWorkbenchProof } from '@shared/types/__tests__/workbenchProof.fixture'

import {
  connectionSchema,
  jsonBody,
  librarySchema,
  createPaperloomClient,
  PaperloomError,
  validatePaperPdf
} from '../paperloomApi'
import { paperloomPageRange } from '../PaperloomProcessDialog'

const memberId = '00000000-0000-4000-8000-000000000001'
const { paperloom, savePaperloomArtifact } = createPaperloomClient(memberId)

vi.mock('@shared/studioRuntime', () => ({
  readStudioRuntime: () => ({ apiBaseUrl: 'http://127.0.0.1:1234', token: 'shell-only' })
}))

afterEach(() => vi.unstubAllGlobals())

describe('PaperLoom host API boundary', () => {
  it('sends the shell bearer and validates the upstream envelope', async () => {
    const fetch = vi
      .fn()
      .mockResolvedValue(new Response(JSON.stringify({ code: 0, data: { documents: [], total: 0 } })))
    vi.stubGlobal('fetch', fetch)
    expect(await paperloom('documents', librarySchema)).toEqual({ documents: [], total: 0 })
    expect(fetch).toHaveBeenCalledWith(
      `http://127.0.0.1:1234/api/paperloom/documents?studio_member=${memberId}`,
      expect.objectContaining({
        headers: { authorization: 'Bearer shell-only' },
        cache: 'no-store'
      })
    )
    fetch.mockResolvedValueOnce(new Response(JSON.stringify({ code: 1, data: { documents: [], total: 0 } })))
    await expect(paperloom('documents', librarySchema)).rejects.toThrow()
    fetch.mockResolvedValueOnce(new Response(JSON.stringify({ code: 0, data: { documents: [], total: -1 } })))
    await expect(paperloom('documents', librarySchema)).rejects.toThrow()
  })

  it('keeps connection errors and malformed responses distinct from an empty library', async () => {
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('{"error":"connection_required"}', { status: 503 })))
    await expect(paperloom('documents', librarySchema)).rejects.toMatchObject({
      status: 503,
      code: 'connection_required'
    })
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(new Response(JSON.stringify({ endpoint: '', apiKeyConfigured: false, revision: '' })))
    )
    expect(await paperloom('connection', connectionSchema)).toMatchObject({
      apiKeyConfigured: false
    })
    expect(jsonBody({ document_id: 'explicit-id' }, 'PATCH')).toMatchObject({
      method: 'PATCH',
      body: '{"document_id":"explicit-id"}'
    })
  })

  it('accepts explicit valid PDF bytes and refuses false extensions, empty and oversized inputs', async () => {
    await expect(
      validatePaperPdf(new File(['%PDF-1.7\n'], 'paper.pdf') as unknown as globalThis.File)
    ).resolves.toBeUndefined()
    for (const file of [
      new File(['not a PDF'], 'paper.pdf'),
      new File(['%PDF-1.7'], 'paper.exe'),
      new File([], 'empty.pdf')
    ]) {
      await expect(validatePaperPdf(file as unknown as globalThis.File)).rejects.toBeInstanceOf(PaperloomError)
    }
    await expect(
      validatePaperPdf({ name: 'huge.pdf', size: 20 * 1024 * 1024 + 1 } as globalThis.File)
    ).rejects.toMatchObject({ code: 'invalid_pdf' })
  })

  it('keeps page selection one-based and bounded by the selected document', () => {
    expect(paperloomPageRange(2, 4, 5)).toEqual([2, 3, 4])
    for (const [first, last, count] of [
      [0, 2, 5],
      [3, 2, 5],
      [1, 6, 5],
      [1.1, 2, 5],
      [1, Infinity, 5],
      [1, 9000, 9000]
    ])
      expect(paperloomPageRange(first, last, count)).toBeNull()
  })

  it('saves exact manifest-verified bytes and refuses changed artifacts or unsafe names', async () => {
    const bytes = new Uint8Array([1, 2, 3])
    const digest = createHash('sha256').update(bytes).digest('hex')
    const save = vi.fn().mockResolvedValue('/chosen/paper.pdf')
    vi.stubGlobal('crypto', webcrypto)
    vi.stubGlobal('window', { api: { file: { save } } })
    vi.stubGlobal(
      'fetch',
      vi.fn().mockImplementation(async () => new Response(bytes))
    )
    await expect(savePaperloomArtifact('jobs/job/artifacts/pdf', 'paper.pdf', `sha256:${digest}`)).resolves.toBe(
      '/chosen/paper.pdf'
    )
    expect(save).toHaveBeenCalledWith('paper.pdf', bytes)
    await expect(savePaperloomArtifact('jobs/job/artifacts/pdf', 'paper.pdf', '0'.repeat(64))).rejects.toMatchObject({
      code: 'artifact_changed'
    })
    await expect(savePaperloomArtifact('jobs/job/artifacts/pdf', '../paper.pdf', digest)).rejects.toMatchObject({
      code: 'invalid_artifact_name'
    })
    expect(save).toHaveBeenCalledTimes(1)
  })
})

const source = {
  root: '/workspace',
  path: '/workspace/paper.pdf',
  name: 'paper.pdf',
  bytes: new TextEncoder().encode('%PDF-test'),
  digest: 'a'.repeat(64),
  revision: '12'.repeat(32)
}
const savedArtifact = { jobId: 'paper-job', key: 'translated-pdf', attempt: 1, checksum: `sha256:${'b'.repeat(64)}` }
function serviceProof() {
  const base = makeWorkbenchProof({
    sessionId: 'paperloom-workspace',
    policy: { reviewMode: false, artifactTrace: true }
  })
  base.artifacts[0] = {
    ...base.artifacts[0],
    path: '/workspace/translated.pdf',
    contentDigest: savedArtifact.checksum,
    origin: {
      kind: 'paperloom-service',
      memberId,
      workspaceRoot: source.root,
      sourcePath: source.path,
      sourceDigest: `sha256:${source.digest}`,
      sourceRevision: source.revision,
      savedRevision: '34'.repeat(32),
      jobId: savedArtifact.jobId,
      jobStatus: 'succeeded',
      attempt: 1,
      artifactKey: savedArtifact.key,
      manifestDigest: `sha256:${'c'.repeat(64)}`,
      connectionRevision: '56'.repeat(32),
      packageDigest: `sha256:${'d'.repeat(64)}`
    }
  }
  return base
}

describe('workspace artifact source boundary', () => {
  it('requests host verification with identities, and refuses a receipt owned by a different member', async () => {
    const client = createPaperloomClient(memberId)
    const proof = serviceProof()
    const fetch = vi.fn().mockImplementation(async () => new Response(JSON.stringify({ code: 0, data: proof })))
    vi.stubGlobal('fetch', fetch)
    await expect(
      client.registerSavedArtifact(source, '/workspace/translated.pdf', savedArtifact)
    ).resolves.toMatchObject({ sessionId: 'paperloom-workspace' })
    const body = JSON.parse(fetch.mock.calls[0][1].body)
    expect(body).toMatchObject({
      sourcePath: source.path,
      sourceRevision: source.revision,
      sourceDigest: source.digest,
      savedPath: '/workspace/translated.pdf',
      artifactKey: 'translated-pdf',
      attempt: 1
    })
    expect(body).not.toHaveProperty('bytes')
    proof.artifacts[0].origin!.memberId = 'other-member'
    await expect(
      client.registerSavedArtifact(source, '/workspace/translated.pdf', savedArtifact)
    ).rejects.toMatchObject({ code: 'provenance_invalid' })
  })

  it('rejects an old source revision when reopening saved receipts', async () => {
    const proof = serviceProof()
    proof.artifacts[0].origin!.sourceRevision = '78'.repeat(32)
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(JSON.stringify({ code: 0, data: [proof] }))))
    await expect(createPaperloomClient(memberId).lookupSourceProvenance(source)).rejects.toMatchObject({
      code: 'provenance_invalid'
    })
  })

  it('refuses history for another saved path and snapshots with another artifact identity', async () => {
    const client = createPaperloomClient(memberId)
    const proof = serviceProof()
    const transport = client.provenanceTransport(proof)
    const foreign = serviceProof()
    foreign.artifacts[0].path = '/workspace/foreign.pdf'
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(JSON.stringify({ code: 0, data: [foreign] }))))
    await expect(transport.loadProofs(proof.sessionId)).rejects.toMatchObject({ code: 'provenance_invalid' })
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            code: 0,
            data: {
              id: 'foreign',
              name: 'other.pdf',
              mediaType: 'application/pdf',
              contentDigest: savedArtifact.checksum,
              data: 'AA=='
            }
          })
        )
      )
    )
    await expect(
      transport.loadArtifact(proof.sessionId, proof.artifacts[0].id, new AbortController().signal)
    ).rejects.toMatchObject({ code: 'provenance_invalid' })
  })
})
