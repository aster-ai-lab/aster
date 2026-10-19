import { webcrypto } from 'node:crypto'

import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import { useEffect } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { AbsoluteFilePath } from '@shared/types/file'

import { publishActiveReviewFinding, publishReviewDocument } from '../reviewFindings'
import { ReviewOfficeDocument } from '../ReviewOfficeDocument'

type EditorContent = { type: 'document'; html: string; comments: { text: string }[] }
const editorCallbacks: {
  onEditorReady: (editor: unknown) => void
  onChange: (content: EditorContent) => void
}[] = []

vi.mock('@a3s-lab/office/core', () => ({
  importOfficeFile: async () => ({ content: { type: 'document', html: '<p>付款以经完成。</p>', comments: [] } })
}))

vi.mock('@a3s-lab/office/react', () => ({
  DocumentEditor: ({
    content,
    preview,
    onEditorReady,
    onChange
  }: {
    content: { html: string; comments: { text: string }[] }
    preview: boolean
    onEditorReady: (editor: unknown) => void
    onChange: (content: EditorContent) => void
  }) => {
    useEffect(() => {
      editorCallbacks.push({ onEditorReady, onChange })
      onEditorReady({
        isDestroyed: false,
        state: {
          doc: {
            descendants: (visitor: (node: unknown, pos: number) => void) =>
              visitor({ isText: true, text: '付款以经完成。' }, 1)
          }
        },
        commands: { insertDocumentCommentThread: () => true }
      })
    }, [onEditorReady, onChange])
    return (
      <div data-testid="source-editor" contentEditable={!preview} suppressContentEditableWarning>
        {content.html}
        {content.comments.map((comment, index) => (
          <p key={index}>{comment.text}</p>
        ))}
      </div>
    )
  }
}))

const path = '/tmp/source.docx' as AbsoluteFilePath
const revision = '039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81'
const changed = '787c798e39a5bc1910355bae6d0cd87a36b2e10fd0202a83e3bb6b005da83472'
const findings = [{ id: 'f', title: '错字', evidenceQuote: '以经', summary: '用字错误', guidance: '已经' }]

function snapshot(digest: string) {
  return new Response(new Uint8Array(digest === revision ? [1, 2, 3] : [4, 5, 6]), {
    headers: { etag: `"${digest}"`, 'content-length': '3' }
  })
}

function transport(read: () => Response | Promise<Response> = () => snapshot(revision), head?: Promise<void>) {
  const streams: {
    controller: ReadableStreamDefaultController<Uint8Array>
    signal?: AbortSignal | null
    after: number
  }[] = []
  const reads: (AbortSignal | null | undefined)[] = []
  const fetcher = vi.fn(async (url: string, init?: RequestInit) => {
    if (url.includes('/api/local-documents/changes?')) {
      if (head) await head
      if (init?.signal?.aborted) throw new DOMException('aborted', 'AbortError')
      let onAbort: () => void = () => undefined
      const body = new ReadableStream<Uint8Array>({
        start(controller) {
          onAbort = () => controller.error(new DOMException('aborted', 'AbortError'))
          init?.signal?.addEventListener('abort', onAbort, { once: true })
          streams.push({ controller, signal: init?.signal, after: Number(new URL(url).searchParams.get('after')) })
        },
        cancel() {
          init?.signal?.removeEventListener('abort', onAbort)
        }
      })
      return new Response(body, { headers: { 'content-type': 'text/event-stream' } })
    }
    expect(url).toContain('/api/workspace-file/snapshot?')
    reads.push(init?.signal)
    return read()
  })
  vi.stubGlobal('fetch', fetcher)
  return {
    fetcher,
    streams,
    reads,
    change(change: { revision: number; paths: string[]; fullScan: boolean }) {
      streams.at(-1)!.controller.enqueue(new TextEncoder().encode(`data: ${JSON.stringify(change)}\n\n`))
    }
  }
}

beforeEach(() => {
  vi.stubGlobal('crypto', webcrypto)
  ;(globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__ = {
    apiBaseUrl: 'http://127.0.0.1:4318',
    token: 'ab'.repeat(32)
  }
  publishActiveReviewFinding(null)
  editorCallbacks.length = 0
})

afterEach(() => {
  cleanup()
  publishReviewDocument(null)
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

describe('version-bound review document', () => {
  it('reports only placed comments and distinguishes an empty or unmatched result', async () => {
    transport()
    const view = render(
      <ReviewOfficeDocument
        filePath={path}
        fileName="source.docx"
        findings={[]}
        revision={revision}
        phase="reviewing"
      />
    )
    await screen.findByTestId('source-editor')
    expect(view.container.querySelector('[data-review-outcome]')).toHaveTextContent('已批注 0 条')
    expect(view.container.querySelector('[data-review-outcome]')).toHaveTextContent('正在审查此文档')
    view.rerender(<ReviewOfficeDocument filePath={path} fileName="source.docx" findings={[]} revision={revision} />)
    expect(view.container.querySelector('[data-review-outcome]')).toHaveTextContent('未产生可定位批注')
    view.rerender(
      <ReviewOfficeDocument
        filePath={path}
        fileName="source.docx"
        findings={[{ ...findings[0], evidenceQuote: '不存在的原文' }]}
        revision={revision}
      />
    )
    expect(view.container.querySelector('[data-review-outcome]')).toHaveTextContent('已批注 0 条')
    expect(view.container.querySelector('[data-review-outcome]')).toHaveTextContent('1 条待定位或无法唯一定位')
  })

  it.each(['failed', 'cancelled'] as const)('retains real placed comments after %s', async (phase) => {
    transport()
    const view = render(
      <ReviewOfficeDocument
        filePath={path}
        fileName="source.docx"
        findings={findings}
        revision={revision}
        phase="reviewing"
      />
    )
    await screen.findByText('建议：已经', { exact: false })
    view.rerender(
      <ReviewOfficeDocument
        filePath={path}
        fileName="source.docx"
        findings={findings}
        revision={revision}
        phase={phase}
      />
    )
    expect(view.container.querySelector('[data-review-outcome]')).toHaveAttribute('data-review-outcome', phase)
    expect(view.container.querySelector('[data-review-outcome]')).toHaveTextContent('已批注 1 条')
    expect(screen.getByText('建议：已经', { exact: false })).toBeInTheDocument()
  })
  it.each([
    ['/source.docx', '/'],
    ['C:\\source.docx', 'C:\\'],
    ['C:/source.docx', 'C:/']
  ])('watches the actual parent of %s', async (filePath, parent) => {
    const source = transport()
    render(
      <ReviewOfficeDocument
        filePath={filePath as AbsoluteFilePath}
        fileName="source.docx"
        findings={findings}
        revision={revision}
      />
    )
    await screen.findByTestId('source-editor')
    for (const [url] of source.fetcher.mock.calls) expect(new URL(url).searchParams.get('root')).toBe(parent)
  })

  it('anchors verified findings into a read-only view of the matching snapshot', async () => {
    const source = transport()
    const { WorkbenchFileEditor } = await import('../../WorkbenchFileEditor')
    publishReviewDocument({ path, revision, findings })
    render(<WorkbenchFileEditor root="/tmp" path={path} />)
    const editor = await screen.findByTestId('source-editor')
    expect(editor).toHaveAttribute('contenteditable', 'false')
    await waitFor(() => expect(editor).toHaveTextContent('建议：已经'))
    expect(source.streams).toHaveLength(1)
    expect(source.reads).toHaveLength(1)
  })

  it('refuses changed bytes even when the evidence quote would still match', async () => {
    transport(
      () => new Response(new Uint8Array([4, 5, 6]), { headers: { etag: `"${revision}"`, 'content-length': '3' } })
    )
    render(<ReviewOfficeDocument filePath={path} fileName="source.docx" findings={findings} revision={revision} />)
    expect(await screen.findByText('原文版本已变更')).toBeInTheDocument()
    expect(screen.queryByTestId('source-editor')).not.toBeInTheDocument()
    expect(screen.queryByText('建议：已经')).not.toBeInTheDocument()
  })

  it('requires a revision for historical findings instead of attaching them to the current file', async () => {
    const fetcher = vi.fn()
    vi.stubGlobal('fetch', fetcher)
    render(<ReviewOfficeDocument filePath={path} fileName="source.docx" findings={findings} />)
    expect(await screen.findByText('无法确认校核原文版本')).toBeInTheDocument()
    expect(fetcher).not.toHaveBeenCalled()
    expect(screen.queryByTestId('source-editor')).not.toBeInTheDocument()
  })

  it('discards an older pending load when the review revision changes at the same path', async () => {
    let resolve: (value: Response) => void = () => undefined
    const first = new Promise<Response>((done) => {
      resolve = done
    })
    const read = vi
      .fn()
      .mockReturnValueOnce(first)
      .mockImplementation(async () => snapshot(changed))
    const source = transport(read)
    const view = render(
      <ReviewOfficeDocument filePath={path} fileName="source.docx" findings={findings} revision={revision} />
    )
    await waitFor(() => expect(source.reads).toHaveLength(1))
    view.rerender(<ReviewOfficeDocument filePath={path} fileName="source.docx" findings={[]} revision={changed} />)
    await screen.findByTestId('source-editor')
    await act(async () => {
      resolve(snapshot(revision))
      await first
    })
    expect(screen.queryByText('建议：已经')).not.toBeInTheDocument()
    expect(screen.getByTestId('source-editor')).toHaveAttribute('contenteditable', 'false')
    expect(source.streams[0].signal?.aborted).toBe(true)
    expect(source.reads[0]?.aborted).toBe(true)
  })

  it('waits for the watch handshake before reading or showing annotations', async () => {
    let release: () => void = () => undefined
    const head = new Promise<void>((resolve) => {
      release = resolve
    })
    const source = transport(undefined, head)
    render(<ReviewOfficeDocument filePath={path} fileName="source.docx" findings={findings} revision={revision} />)
    await waitFor(() => expect(source.fetcher).toHaveBeenCalledTimes(1))
    expect(source.reads).toHaveLength(0)
    expect(screen.queryByTestId('source-editor')).not.toBeInTheDocument()
    await act(async () => {
      release()
      await head
    })
    await screen.findByText('建议：已经', { exact: false })
  })

  it('withdraws annotations during an active source check and rejects changed bytes', async () => {
    let release: (value: Response) => void = () => undefined
    const pending = new Promise<Response>((resolve) => {
      release = resolve
    })
    let next: () => Response | Promise<Response> = () => snapshot(revision)
    const source = transport(() => next())
    render(<ReviewOfficeDocument filePath={path} fileName="source.docx" findings={findings} revision={revision} />)
    await screen.findByText('建议：已经', { exact: false })
    next = () => pending
    await act(async () => {
      source.change({ revision: 1, paths: [''], fullScan: false })
    })
    await waitFor(() => expect(source.reads).toHaveLength(2))
    expect(screen.queryByTestId('source-editor')).not.toBeInTheDocument()
    expect(screen.queryByText('建议：已经', { exact: false })).not.toBeInTheDocument()
    await act(async () => {
      release(snapshot(changed))
      await pending
    })
    await screen.findByText('原文版本已变更')
    expect(screen.queryByTestId('source-editor')).not.toBeInTheDocument()
  })

  it('ignores nested directory hints and checks a full rescan before restoring unchanged annotations', async () => {
    const source = transport()
    render(<ReviewOfficeDocument filePath={path} fileName="source.docx" findings={findings} revision={revision} />)
    await screen.findByText('建议：已经', { exact: false })
    await act(async () => {
      source.change({ revision: 1, paths: ['other'], fullScan: false })
    })
    expect(source.reads).toHaveLength(1)
    await act(async () => {
      source.change({ revision: 8, paths: [], fullScan: true })
    })
    await waitFor(() => expect(source.reads).toHaveLength(2))
    await screen.findByText('建议：已经', { exact: false })
    const old = editorCallbacks[0]
    const insert = vi.fn()
    act(() => {
      old.onChange({ type: 'document', html: 'retired editor content', comments: [] })
      old.onEditorReady({ isDestroyed: false, commands: { insertDocumentCommentThread: insert } })
    })
    expect(insert).not.toHaveBeenCalled()
    expect(screen.queryByText('retired editor content')).not.toBeInTheDocument()
    expect(screen.getByTestId('source-editor')).toHaveAttribute('contenteditable', 'false')
  })

  it.each(['eof', 'malformed'])(
    'withdraws on %s and revalidates after reconnecting from the last cursor',
    async (fault) => {
      const source = transport()
      const view = render(
        <ReviewOfficeDocument filePath={path} fileName="source.docx" findings={findings} revision={revision} />
      )
      await screen.findByText('建议：已经', { exact: false })
      await act(async () => {
        source.change({ revision: 7, paths: ['other'], fullScan: false })
        if (fault === 'eof') source.streams[0].controller.close()
        else source.streams[0].controller.enqueue(new TextEncoder().encode('data: {"revision":"invalid"}\n\n'))
      })
      await screen.findByText('无法持续确认原文版本')
      expect(screen.queryByTestId('source-editor')).not.toBeInTheDocument()
      expect(source.reads[0]?.aborted).toBe(true)
      await screen.findByText('建议：已经', { exact: false }, { timeout: 4000 })
      expect(source.streams).toHaveLength(2)
      expect(source.streams[1].after).toBe(7)
      expect(source.reads).toHaveLength(2)
      view.unmount()
      expect(source.streams[1].signal?.aborted).toBe(true)
      expect(source.reads[1]?.aborted).toBe(true)
    }
  )

  it('keeps annotations hidden when watch admission fails and recovers only after a fresh read', async () => {
    const source = transport()
    source.fetcher.mockResolvedValueOnce(new Response('', { status: 503 }))
    render(<ReviewOfficeDocument filePath={path} fileName="source.docx" findings={findings} revision={revision} />)
    await screen.findByText('无法持续确认原文版本')
    expect(source.reads).toHaveLength(0)
    expect(screen.queryByTestId('source-editor')).not.toBeInTheDocument()
    await screen.findByText('建议：已经', { exact: false }, { timeout: 4000 })
    expect(source.reads).toHaveLength(1)
  })

  it('discards a retired read when a newer disk change is already rejected', async () => {
    let release: (value: Response) => void = () => undefined
    const first = new Promise<Response>((resolve) => {
      release = resolve
    })
    const read = vi
      .fn()
      .mockReturnValueOnce(first)
      .mockImplementation(async () => snapshot(changed))
    const source = transport(read)
    const view = render(
      <ReviewOfficeDocument filePath={path} fileName="source.docx" findings={findings} revision={revision} />
    )
    await waitFor(() => expect(source.reads).toHaveLength(1))
    await act(async () => {
      source.change({ revision: 1, paths: [''], fullScan: false })
    })
    await screen.findByText('原文版本已变更')
    await act(async () => {
      release(snapshot(revision))
      await first
    })
    expect(screen.getByText('原文版本已变更')).toBeInTheDocument()
    expect(screen.queryByTestId('source-editor')).not.toBeInTheDocument()
    expect(source.reads[0]?.aborted).toBe(true)
    view.unmount()
    expect(source.streams[0].signal?.aborted).toBe(true)
  })
})
