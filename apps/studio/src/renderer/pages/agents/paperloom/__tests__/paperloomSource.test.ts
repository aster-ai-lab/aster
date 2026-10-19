import { File } from 'node:buffer'
import { createHash, webcrypto } from 'node:crypto'

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { readLocalDocumentBytes } from '@renderer/pages/documents/useLocalDocuments'

import { PAPERLOOM_UPLOAD_LIMIT } from '../paperloomApi'
import { readWorkspacePaper, verifyWorkspacePaper } from '../paperloomSource'

vi.mock('@renderer/pages/documents/useLocalDocuments', () => ({ readLocalDocumentBytes: vi.fn() }))
const bytes = new TextEncoder().encode('%PDF-1.7\nsource one')
const revision = '12'.repeat(32)
const digest = (value: Uint8Array) => createHash('sha256').update(value).digest('hex')

beforeEach(() => {
  vi.stubGlobal('File', File)
  vi.stubGlobal('crypto', webcrypto)
  vi.mocked(readLocalDocumentBytes).mockImplementation(async (_root, _path, _progress, _limit, onSnapshot) => {
    onSnapshot?.(revision)
    return bytes.slice()
  })
})
afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('workspace paper source authority', () => {
  it('binds exact workspace paths and actual bytes to the snapshot revision', async () => {
    const abort = new AbortController()
    const source = await readWorkspacePaper('/workspace/task-one', '/workspace/task-one/论文.PDF', abort.signal)
    expect(source).toMatchObject({
      root: '/workspace/task-one',
      path: '/workspace/task-one/论文.PDF',
      name: '论文.PDF',
      revision,
      digest: digest(bytes)
    })
    expect(source.bytes).toEqual(bytes)
    expect(readLocalDocumentBytes).toHaveBeenCalledWith(
      '/workspace/task-one',
      '/workspace/task-one/论文.PDF',
      undefined,
      PAPERLOOM_UPLOAD_LIMIT,
      expect.any(Function),
      abort.signal
    )
  })

  it('requires a workspace revision without mistaking it for a content checksum', async () => {
    vi.mocked(readLocalDocumentBytes).mockResolvedValueOnce(bytes.slice())
    await expect(readWorkspacePaper('/workspace', '/workspace/paper.pdf')).rejects.toMatchObject({
      code: 'workspace_source_failed'
    })
    const source = await readWorkspacePaper('/workspace', '/workspace/paper.pdf')
    expect(source.revision).toBe(revision)
    expect(source.digest).toBe(digest(bytes))
    expect(source.digest).not.toBe(source.revision)
  })

  it('refuses a new file identity even when the content bytes are unchanged', async () => {
    const source = await readWorkspacePaper('/workspace', '/workspace/paper.pdf')
    vi.mocked(readLocalDocumentBytes).mockImplementationOnce(async (_r, _p, _a, _b, onSnapshot) => {
      onSnapshot?.('34'.repeat(32))
      return bytes.slice()
    })
    await expect(verifyWorkspacePaper(source)).rejects.toMatchObject({ code: 'source_changed' })
  })

  it('rechecks source bytes and refuses an in-place replacement before execution', async () => {
    const source = await readWorkspacePaper('/workspace', '/workspace/paper.pdf')
    await expect(verifyWorkspacePaper(source)).resolves.toBeUndefined()
    const replacement = new TextEncoder().encode('%PDF-1.7\nsource two')
    vi.mocked(readLocalDocumentBytes).mockImplementationOnce(async (_r, _p, _a, _b, onSnapshot) => {
      onSnapshot?.(digest(replacement))
      return replacement
    })
    await expect(verifyWorkspacePaper(source)).rejects.toMatchObject({ code: 'source_changed' })
  })

  it('preserves workspace permission failures and does not infer another path', async () => {
    const source = await readWorkspacePaper('/workspace', '/workspace/paper.pdf')
    vi.mocked(readLocalDocumentBytes).mockRejectedValueOnce(new Error('access revoked'))
    await expect(verifyWorkspacePaper(source)).rejects.toThrow('access revoked')
    expect(vi.mocked(readLocalDocumentBytes).mock.calls.at(-1)?.slice(0, 2)).toEqual([
      '/workspace',
      '/workspace/paper.pdf'
    ])
  })

  it('requires an explicit PDF source and refuses non-PDF bytes', async () => {
    await expect(readWorkspacePaper('', '/paper.pdf')).rejects.toMatchObject({ code: 'workspace_source_required' })
    await expect(readWorkspacePaper('/workspace', '/workspace/paper.txt')).rejects.toMatchObject({
      code: 'invalid_pdf'
    })
    vi.mocked(readLocalDocumentBytes).mockResolvedValueOnce(new TextEncoder().encode('not a PDF'))
    await expect(readWorkspacePaper('/workspace', '/workspace/paper.pdf')).rejects.toMatchObject({
      code: 'invalid_pdf'
    })
  })
})
