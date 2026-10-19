import assert from 'node:assert/strict'
import { File } from 'node:buffer'
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import { createHash, webcrypto } from 'node:crypto'
import { copyFile, mkdir, mkdtemp, readFile, rename, rm, stat, unlink, utimes, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'

import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { useEffect } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { parseReviewFindings, publishActiveReviewFinding } from '@renderer/pages/agents/taskConversation/reviewFindings'
import { ReviewOfficeDocument } from '@renderer/pages/agents/taskConversation/ReviewOfficeDocument'
import type { AbsoluteFilePath } from '@shared/types/file'

import finding from '../fixtures/review-observation/boyue-finding.json'

// Keep the actual Word importer, HTTP routes and OS watcher. This boundary records
// read-only editor props and annotations; it does not certify a native Office window.
vi.mock('@a3s-lab/office/react', () => ({
  DocumentEditor: ({
    content,
    preview,
    onEditorReady
  }: {
    content: { html: string; comments?: { id: string; text: string }[] }
    preview: boolean
    onEditorReady: (editor: unknown) => void
  }) => {
    useEffect(() => {
      const source = new DOMParser().parseFromString(content.html, 'text/html').body.textContent ?? ''
      onEditorReady({
        isDestroyed: false,
        state: {
          doc: {
            descendants: (visitor: (node: unknown, pos: number) => void) => visitor({ isText: true, text: source }, 1)
          }
        },
        commands: { insertDocumentCommentThread: () => true }
      })
    }, [content.html, onEditorReady])
    return (
      <div data-testid="native-review-editor" contentEditable={!preview} suppressContentEditableWarning>
        {content.html}
        {content.comments?.map((comment) => (
          <p key={comment.id}>{comment.text}</p>
        ))}
      </div>
    )
  }
}))

const httpFetch = globalThis.fetch.bind(globalThis)
let root: string
let workspace: string
let file: AbsoluteFilePath
let original: Buffer
let processHost: ChildProcessWithoutNullStreams | undefined
let ended: Promise<void>
let exit: { code: number | null; signal: NodeJS.Signals | null } | undefined
let failure: Error | undefined
let stderr = ''
let watchRequests: number
let snapshotRequests: number

async function startAuthority() {
  const executable = process.env.A3S_STUDIO_NATIVE_TEST
  assert(executable, 'Run scripts/test-workspace-documents.ts to build the native HTTP fixture')
  await rm(path.join(root, 'stop'), { force: true })
  exit = undefined
  failure = undefined
  stderr = ''
  processHost = spawn(
    executable,
    [
      'authority::review_observation_tests::serve_review_observation',
      '--exact',
      '--ignored',
      '--nocapture',
      '--test-threads=1'
    ],
    {
      stdio: 'pipe',
      env: { ...process.env, A3S_STUDIO_REVIEW_OBSERVATION_DIR: root }
    }
  )
  processHost.stdout.resume()
  processHost.stderr.on('data', (chunk: Buffer) => {
    stderr = (stderr + chunk.toString()).slice(-64 * 1024)
  })
  ended = new Promise((resolve) => {
    processHost!.once('exit', (code, signal) => {
      exit = { code, signal }
      resolve()
    })
    processHost!.once('error', (error) => {
      failure = error
      resolve()
    })
  })
  const deadline = Date.now() + 10_000
  while (Date.now() < deadline) {
    if (failure) throw failure
    if (exit) throw new Error(`Native HTTP fixture exited before readiness: ${stderr}`)
    let runtime: { apiBaseUrl: string; token: string } | undefined
    try {
      runtime = JSON.parse(await readFile(path.join(root, 'runtime.json'), 'utf8'))
    } catch {
      /* not ready */
    }
    if (runtime) {
      const url = new URL(runtime.apiBaseUrl)
      assert(url.protocol === 'http:' && url.hostname === '127.0.0.1' && url.port && runtime.token.length === 64)
      Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', { configurable: true, value: runtime })
      return
    }
    await new Promise((resolve) => setTimeout(resolve, 25))
  }
  throw new Error(`Native HTTP fixture readiness timed out: ${stderr}`)
}

async function closeAuthority() {
  if (!processHost) return
  await writeFile(path.join(root, 'stop'), '')
  const child = processHost
  const timer = setTimeout(() => child.kill('SIGKILL'), 5000)
  try {
    await ended
    assert(!failure && exit?.code === 0 && !exit.signal, `Native HTTP fixture did not close: ${stderr}`)
    await expect(readFile(path.join(root, 'runtime.json'))).rejects.toThrow()
  } finally {
    clearTimeout(timer)
    processHost = undefined
  }
}

beforeEach(async () => {
  root = await mkdtemp(path.join(tmpdir(), 'a3s-review-observation-'))
  workspace = path.join(root, 'workspace')
  await mkdir(workspace)
  await writeFile(path.join(root, '.owned'), '')
  file = path.join(workspace, 'b.docx') as AbsoluteFilePath
  await copyFile('tests/e2e/reference-alignment/cases/review-selected-b/inputs/b.docx', file)
  const stamp = new Date('2026-10-16T00:00:00Z')
  await utimes(file, stamp, stamp)
  original = await readFile(file)
  expect(finding.model).toBe('boyue/deepseek-v4-flash')
  expect(createHash('sha256').update(original).digest('hex')).toBe(finding.sourceSha256)
  expect(parseReviewFindings(finding.response)).toHaveLength(1)
  watchRequests = 0
  snapshotRequests = 0
  vi.stubGlobal('crypto', webcrypto)
  vi.stubGlobal('File', File)
  // Native File buffers and external JSZip must share byte-array constructors.
  const ByteArray = Object.getPrototypeOf(Buffer.prototype).constructor as Uint8ArrayConstructor
  vi.stubGlobal('Uint8Array', ByteArray)
  vi.stubGlobal('ArrayBuffer', new ByteArray().buffer.constructor)
  vi.stubGlobal('fetch', async (...args: Parameters<typeof globalThis.fetch>) => {
    const url = new URL(String(args[0]))
    if (url.pathname === '/api/local-documents/changes') watchRequests += 1
    if (url.pathname === '/api/workspace-file/snapshot') snapshotRequests += 1
    return httpFetch(...args)
  })
  publishActiveReviewFinding(null)
  await startAuthority()
  const runtime = (globalThis as unknown as { __A3S_STUDIO_RUNTIME__: { apiBaseUrl: string; token: string } })
    .__A3S_STUDIO_RUNTIME__
  const query = new URLSearchParams({ root: workspace, path: file })
  const snapshot = await httpFetch(`${runtime.apiBaseUrl}/api/workspace-file/snapshot?${query}`, {
    headers: { Authorization: `Bearer ${runtime.token}` }
  })
  expect(snapshot.status).toBe(200)
  expect(
    createHash('sha256')
      .update(new Uint8Array(await snapshot.arrayBuffer()))
      .digest('hex')
  ).toBe(finding.sourceSha256)
  const { importOfficeFile } = await import('@a3s-lab/office/core')
  const imported = await importOfficeFile(new File([original], 'b.docx') as unknown as globalThis.File)
  expect(imported.content.type).toBe('document')
})

afterEach(async () => {
  cleanup()
  try {
    await closeAuthority()
  } finally {
    vi.unstubAllGlobals()
    delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
    await rm(root, { recursive: true, force: true })
  }
})

function editor() {
  return (
    <ReviewOfficeDocument
      filePath={file}
      fileName={path.basename(file)}
      findings={parseReviewFindings(finding.response)}
      revision={finding.sourceSha256}
    />
  )
}

describe('actual native HTTP source observation', () => {
  it('withdraws after source deletion and restores only the exact reviewed bytes', async () => {
    render(editor())
    await screen.findByTestId('native-review-editor', {}, { timeout: 10_000 })
    await unlink(file)
    await screen.findByText('文件不可用', {}, { timeout: 5000 })
    expect(screen.queryByTestId('native-review-editor')).not.toBeInTheDocument()
    await writeFile(file, original)
    await screen.findByTestId('native-review-editor', {}, { timeout: 5000 })
    await waitFor(() => expect(screen.getByTestId('native-review-editor').textContent).toContain('将“以经”改为“已经”'))
    expect(await readFile(file)).toEqual(original)
  }, 20_000)

  it.each(['b.docx', '.b.docx'])(
    'withdraws a Boyue finding after a same-length, same-mtime Word replacement: %s',
    async (name) => {
      if (name !== path.basename(file)) {
        const next = path.join(workspace, name) as AbsoluteFilePath
        await rename(file, next)
        file = next
      }
      render(editor())
      const view = await screen.findByTestId('native-review-editor', {}, { timeout: 10_000 })
      await waitFor(() => expect(view.textContent).toContain('将“以经”改为“已经”'))
      expect(view).toHaveAttribute('contenteditable', 'false')
      const before = await stat(file)
      const replacement = Buffer.from(original)
      expect(replacement.readUInt32LE(0)).toBe(0x04034b50)
      replacement[10] ^= 1 // ZIP timestamp changes; document text and file length remain identical.
      await writeFile(file, replacement)
      await utimes(file, before.atime, before.mtime)
      expect((await stat(file)).size).toBe(before.size)
      expect((await stat(file)).mtimeMs).toBe(before.mtimeMs)
      await screen.findByText('原文版本已变更', {}, { timeout: 5000 })
      expect(screen.queryByTestId('native-review-editor')).not.toBeInTheDocument()
      expect(watchRequests).toBe(1)
      expect(snapshotRequests).toBeGreaterThanOrEqual(2)
      expect(await readFile(file)).toEqual(replacement)
    },
    20_000
  )

  it('withdraws on an actual server exit and rechecks bytes after a server restart', async () => {
    render(editor())
    await screen.findByTestId('native-review-editor', {}, { timeout: 10_000 })
    await closeAuthority()
    await screen.findByText('无法持续确认原文版本', {}, { timeout: 5000 })
    expect(screen.queryByTestId('native-review-editor')).not.toBeInTheDocument()
    const replacement = Buffer.from(original)
    replacement[10] ^= 1
    await writeFile(file, replacement)
    await startAuthority()
    await screen.findByText('原文版本已变更', {}, { timeout: 6000 })
    expect(screen.queryByTestId('native-review-editor')).not.toBeInTheDocument()
    await act(async () => {
      await writeFile(file, original)
    })
    await screen.findByTestId('native-review-editor', {}, { timeout: 5000 })
    await waitFor(() => expect(screen.getByTestId('native-review-editor').textContent).toContain('将“以经”改为“已经”'))
    expect(watchRequests).toBeGreaterThanOrEqual(2)
    expect(await readFile(file)).toEqual(original)
  }, 25_000)
})
