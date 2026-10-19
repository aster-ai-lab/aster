import { createHash } from 'node:crypto'

import { MockUsePreferenceUtils } from '@test-mocks/renderer/usePreference'
import { workspaceDocumentsFixture } from '@test-mocks/renderer/workspaceDocuments'
import { act, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import DocumentsPage from '../DocumentsPage'
import { DOCUMENT_READ_BYTES, TEXT_READ_BYTES, VISUAL_TEXT_UNITS } from '../fileOpenBudget'

const ROOT = '/workspace/a3s-documents'
const FOLDER = '项目资料'
const EDITED_OFFICE_BYTES = [4, 3, 2, 1]
const EDITED_PDF_BYTES = [9, 8, 7]

type EditorName = 'DocumentEditor' | 'MarkdownEditor' | 'SpreadsheetEditor' | 'PresentationEditor'

type OfficeContent = { type: string; sourceBytes: number[] }

type OfficeMount =
  | { editor: EditorName; content: OfficeContent; presentation?: string }
  | {
      editor: 'PdfViewer'
      fileName?: string
      sourceKey?: string
      loadSource: () => Promise<Blob>
      onSave?: (pdf: Blob) => Promise<boolean>
    }

const office = vi.hoisted(() => {
  const mounts: OfficeMount[] = []
  const exported: number[][] = []
  return {
    mounts,
    exported,
    failSave: false,
    writes: 0,
    holdWrite: null as Promise<void> | null
  }
})

function blobWithBytes(bytes: readonly number[], type: string): Blob {
  const data = Uint8Array.from(bytes)
  const blob = new Blob([data], { type })
  if (typeof blob.arrayBuffer !== 'function') {
    Object.defineProperty(blob, 'arrayBuffer', {
      value: async () => data.slice().buffer
    })
  }
  return blob
}

vi.mock('@a3s-lab/office/core', async () => {
  const { previewKind: kindForName } = await import('../documentPreview')

  async function readFileBytes(file: Blob): Promise<Uint8Array> {
    if (typeof file.arrayBuffer === 'function') return new Uint8Array(await file.arrayBuffer())
    return await new Promise((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer))
      reader.onerror = () => reject(reader.error ?? new Error('read'))
      reader.readAsArrayBuffer(file)
    })
  }

  return {
    importOfficeFile: async (file: File) => {
      if (file.name.startsWith('损坏')) throw new Error('kind')
      const kind = kindForName(file.name)
      if (kind !== 'document' && kind !== 'markdown' && kind !== 'spreadsheet' && kind !== 'presentation') {
        throw new Error('kind')
      }
      const sourceBytes = Array.from(await readFileBytes(file))
      return {
        id: file.name,
        kind,
        title: file.name,
        favorite: false,
        createdAt: 0,
        updatedAt: 0,
        lastOpenedAt: 0,
        revision: 0,
        content: { type: kind, sourceBytes }
      }
    },
    createArtifactBlob: async (artifact: { content: OfficeContent }) => {
      const sourceBytes = Array.from(artifact.content.sourceBytes)
      office.exported.push(sourceBytes)
      const data = Uint8Array.from(sourceBytes)
      return { arrayBuffer: async () => data.slice().buffer }
    }
  }
})

vi.mock('@a3s-lab/office/react', async () => {
  const { createElement, useEffect } = await import('react')

  function recordEditor(editor: EditorName) {
    return function Editor(props: {
      content: OfficeContent
      presentation?: string
      onChange?: (content: OfficeContent | { type: 'markdown'; markdown: string }) => void
    }) {
      office.mounts.push({ editor, content: props.content, presentation: props.presentation })
      return createElement(
        'div',
        { 'data-editor': editor },
        props.content.type === 'markdown'
          ? createElement(
              'button',
              {
                type: 'button',
                onClick: () => props.onChange?.({ type: 'markdown', markdown: '# 草稿\n' })
              },
              '写入草稿'
            )
          : null,
        createElement(
          'button',
          {
            type: 'button',
            onClick: () => props.onChange?.({ ...props.content, sourceBytes: EDITED_OFFICE_BYTES })
          },
          '改内容'
        ),
        createElement(
          'button',
          {
            type: 'button',
            onClick: () => props.onChange?.({ ...props.content, sourceBytes: [4, 3, 2, 9] })
          },
          '再改'
        )
      )
    }
  }

  return {
    DocumentEditor: recordEditor('DocumentEditor'),
    MarkdownEditor: recordEditor('MarkdownEditor'),
    SpreadsheetEditor: recordEditor('SpreadsheetEditor'),
    PresentationEditor: recordEditor('PresentationEditor'),
    PdfViewer: (props: {
      fileName?: string
      sourceKey?: string
      loadSource: () => Promise<Blob>
      onSave?: (pdf: Blob) => Promise<boolean>
    }) => {
      office.mounts.push({
        editor: 'PdfViewer',
        fileName: props.fileName,
        sourceKey: props.sourceKey,
        loadSource: props.loadSource,
        onSave: props.onSave
      })
      useEffect(() => {
        const onKey = (event: KeyboardEvent) => {
          if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey) return
          if (event.key.toLowerCase() !== 's') return
          event.preventDefault()
          void props.onSave?.(blobWithBytes(EDITED_PDF_BYTES, 'application/pdf'))
        }
        window.addEventListener('keydown', onKey)
        return () => window.removeEventListener('keydown', onKey)
      }, [props.onSave])
      return createElement('div', { 'data-editor': 'pdf' })
    }
  }
})

beforeEach(() => {
  Element.prototype.scrollTo = () => undefined
  MockUsePreferenceUtils.resetMocks()
  office.mounts.length = 0
  office.exported.length = 0
  office.failSave = false
  office.writes = 0
  office.holdWrite = null
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

afterEach(() => {
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
  MockUsePreferenceUtils.resetMocks()
})

function encode(text: string) {
  return new TextEncoder().encode(text)
}

async function requestBytes(body: BodyInit | null | undefined): Promise<Uint8Array> {
  if (body == null) return new Uint8Array()
  if (typeof body === 'string') return encode(body)
  if (body instanceof ArrayBuffer) return new Uint8Array(body)
  if (ArrayBuffer.isView(body)) return new Uint8Array(body.buffer, body.byteOffset, body.byteLength)
  if (typeof (body as Blob).arrayBuffer === 'function') return new Uint8Array(await (body as Blob).arrayBuffer())
  return new Uint8Array()
}

function blobBytes(blob: Blob): Promise<Uint8Array> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer))
    reader.onerror = () => reject(reader.error)
    reader.readAsArrayBuffer(blob)
  })
}

function pathFor(name: string) {
  return `${ROOT}/${FOLDER}/${name}`
}

function renderWorkspace(files: Array<{ name: string; bytes?: Uint8Array; missing?: boolean }>) {
  const store = new Map<string, Uint8Array>()
  for (const file of files) {
    if (!file.missing && file.bytes) store.set(pathFor(file.name), Uint8Array.from(file.bytes))
  }
  const revision = (path: string) =>
    createHash('sha256')
      .update(store.get(path) ?? new Uint8Array())
      .digest('hex')
  const headers = (path: string) => ({
    get: (name: string) => (name.toLowerCase() === 'etag' ? `"${revision(path)}"` : null)
  })
  MockUsePreferenceUtils.setPreferenceValue('app.workspace.path', ROOT)
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    configurable: true,
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
  })
  const diskFetch = async (url: string, init?: RequestInit) => {
    const text = String(url)
    if (text.includes('/api/local-documents/entries')) {
      const path = new URL(text).searchParams.get('path') ?? ''
      if (path === `${ROOT}/${FOLDER}`) {
        return {
          ok: true,
          json: async () => ({
            entries: files.map((file, index) => ({
              name: file.name,
              path: pathFor(file.name),
              kind: 'file',
              sizeBytes: file.bytes?.byteLength ?? 0,
              modifiedMs: files.length - index
            })),
            capped: false
          })
        }
      }
      return {
        ok: true,
        json: async () => ({
          entries: [
            {
              name: FOLDER,
              path: `${ROOT}/${FOLDER}`,
              kind: 'directory',
              sizeBytes: 0,
              modifiedMs: files.length + 1
            }
          ],
          capped: false
        })
      }
    }
    if (text.includes('/api/workspace-search')) {
      const query = new URL(text).searchParams.get('q') ?? ''
      return {
        ok: true,
        json: async () => ({
          entries: files
            .filter((file) => file.name.includes(query))
            .map((file) => ({
              path: pathFor(file.name),
              relativePath: `${FOLDER}/${file.name}`,
              isDirectory: false
            }))
        })
      }
    }
    if (text.includes('/api/workspace-file/raw') || text.includes('/api/workspace-file/snapshot')) {
      const path = new URL(text).searchParams.get('path') ?? ''
      if (path.endsWith('/过大.txt') && (init?.method ?? 'GET').toUpperCase() !== 'PUT') {
        return {
          ok: true,
          status: 200,
          headers: {
            get: (name: string) => (name.toLowerCase() === 'content-length' ? String(TEXT_READ_BYTES + 5) : null)
          },
          body: { cancel: async () => undefined },
          json: async () => ({}),
          arrayBuffer: async () => {
            throw new Error('oversized body was read')
          }
        }
      }
      if (path.endsWith('/超限.docx') && (init?.method ?? 'GET').toUpperCase() !== 'PUT') {
        return {
          ok: true,
          status: 200,
          headers: {
            get: (name: string) => (name.toLowerCase() === 'content-length' ? String(DOCUMENT_READ_BYTES + 1) : null)
          },
          body: { cancel: async () => undefined },
          json: async () => ({}),
          arrayBuffer: async () => {
            throw new Error('oversized body was read')
          }
        }
      }
      if (path.endsWith('/揭榜.docx') && (init?.method ?? 'GET').toUpperCase() !== 'PUT') {
        const payload = Uint8Array.from([1, 2, 3, 4])
        let sent = false
        return {
          ok: true,
          status: 200,
          headers: {
            get: (name: string) =>
              name.toLowerCase() === 'content-length' ? String(Math.round(32.1 * 1024 * 1024)) : headers(path).get(name)
          },
          body: {
            getReader: () => ({
              read: async () => {
                if (sent) return { done: true, value: undefined }
                sent = true
                return { done: false, value: payload }
              },
              cancel: async () => undefined
            })
          },
          json: async () => ({}),
          arrayBuffer: async () => payload.buffer
        }
      }
      if ((init?.method ?? 'GET').toUpperCase() === 'PUT') {
        if (office.failSave) {
          return { ok: false, status: 500, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
        }
        office.writes += 1
        const gate = office.holdWrite
        office.holdWrite = null
        if (gate) await gate
        if (
          text.includes('/snapshot') &&
          new globalThis.Headers(init?.headers).get('if-match') !== `"${revision(path)}"`
        )
          return { ok: false, status: 409 }
        store.set(path, await requestBytes(init?.body))
        return {
          ok: true,
          headers: headers(path),
          json: async () => ({ ok: true }),
          arrayBuffer: async () => new ArrayBuffer(0)
        }
      }
      const bytes = store.get(path)
      if (!bytes) {
        return { ok: false, status: 404, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
      }
      const copy = Uint8Array.from(bytes)
      return {
        ok: true,
        status: 200,
        headers: headers(path),
        json: async () => ({}),
        arrayBuffer: async () => copy.buffer
      }
    }
    return { ok: false, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
  }
  const documents = workspaceDocumentsFixture(diskFetch)
  vi.stubGlobal(
    'fetch',
    vi.fn((url: string, init?: globalThis.RequestInit) =>
      String(url).includes('/api/workspace-documents') ? documents(init) : diskFetch(url, init)
    )
  )
  render(<DocumentsPage />)
  return store
}

async function openLocal(name: string) {
  const user = userEvent.setup()
  await user.click(await screen.findByRole('button', { name: FOLDER }))
  await user.dblClick(await screen.findByRole('button', { name }))
  return user
}

function latestMount(editor: OfficeMount['editor']) {
  return office.mounts.filter((mount) => mount.editor === editor).at(-1)
}

describe('DocumentsPage open view', () => {
  it('retains the draft and external bytes when the disk observation is stale', async () => {
    const store = renderWorkspace([{ name: '纪要.txt', bytes: encode('original') }])
    await openLocal('纪要.txt')
    const box = await screen.findByRole('textbox', { name: '纪要.txt' })
    fireEvent.change(box, { target: { value: 'local draft' } })
    store.set(pathFor('纪要.txt'), encode('external change'))
    fireEvent.keyDown(box, { key: 's', metaKey: true })
    expect(await screen.findByText('无法保存此文件。自动保存已暂停，请重试或放弃修改。')).toBeInTheDocument()
    expect(box).toHaveValue('local draft')
    expect(new globalThis.TextDecoder().decode(store.get(pathFor('纪要.txt')))).toBe('external change')
    expect(screen.queryByText(/最近修改 \d{2}:\d{2}/u)).toBeNull()
  })

  it('uses the accepted new observation for a second save', async () => {
    const store = renderWorkspace([{ name: '纪要.txt', bytes: encode('original') }])
    await openLocal('纪要.txt')
    const box = await screen.findByRole('textbox', { name: '纪要.txt' })
    fireEvent.change(box, { target: { value: 'first save' } })
    fireEvent.keyDown(box, { key: 's', metaKey: true })
    await waitFor(() => expect(new globalThis.TextDecoder().decode(store.get(pathFor('纪要.txt')))).toBe('first save'))
    fireEvent.change(box, { target: { value: 'second save' } })
    fireEvent.keyDown(box, { key: 's', metaKey: true })
    await waitFor(() => expect(new globalThis.TextDecoder().decode(store.get(pathFor('纪要.txt')))).toBe('second save'))
  })

  it('retires an unstarted queued save when its editor closes', async () => {
    const store = renderWorkspace([{ name: '纪要.txt', bytes: encode('original') }])
    const user = await openLocal('纪要.txt')
    const box = await screen.findByRole('textbox', { name: '纪要.txt' })
    let release: () => void = () => undefined
    office.holdWrite = new Promise<void>((resolve) => {
      release = resolve
    })
    fireEvent.change(box, { target: { value: 'admitted save' } })
    fireEvent.keyDown(box, { key: 's', metaKey: true })
    await waitFor(() => expect(office.writes).toBe(1))
    fireEvent.change(box, { target: { value: 'queued save' } })
    fireEvent.keyDown(box, { key: 's', metaKey: true })
    await user.click(screen.getByRole('button', { name: '返回' }))
    await act(async () => {
      release()
      await new Promise((resolve) => globalThis.setTimeout(resolve, 0))
    })
    expect(new globalThis.TextDecoder().decode(store.get(pathFor('纪要.txt')))).toBe('admitted save')
    expect(screen.queryByRole('textbox', { name: '纪要.txt' })).toBeNull()
  })

  it('edits text bytes and shows them on the next open', async () => {
    const store = renderWorkspace([
      { name: '纪要.txt', bytes: encode('你好 fixture') },
      { name: '附录.txt', bytes: encode('乙') }
    ])
    const user = await openLocal('纪要.txt')

    const box = await screen.findByRole('textbox', { name: '纪要.txt' })
    expect(box).toHaveValue('你好 fixture')
    fireEvent.change(box, { target: { value: '你好 saved' } })
    expect(box).toHaveValue('你好 saved')
    fireEvent.keyDown(box, { key: 's', metaKey: true })
    await waitFor(() => expect(new TextDecoder().decode(store.get(pathFor('纪要.txt')))).toBe('你好 saved'))
    expect(screen.getByText(/最近修改 \d{2}:\d{2}/u)).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '返回' }))
    expect(
      within(screen.getByRole('navigation', { name: '当前位置' })).getByRole('button', { name: FOLDER })
    ).toBeInTheDocument()
    const search = screen.getByPlaceholderText('文件、文件夹…')
    await user.type(search, '附录')
    expect(await screen.findByText('附录.txt')).toBeInTheDocument()
    expect(screen.queryByText('纪要.txt')).not.toBeInTheDocument()
    await user.clear(search)
    await user.click(screen.getByRole('button', { name: '纪要.txt' }))
    expect(await screen.findByRole('textbox', { name: '纪要.txt' })).toHaveValue('你好 saved')
  })

  it('opens media in the editor and sends unknown files to the system editor', async () => {
    const user = userEvent.setup()
    renderWorkspace([
      { name: '封面.png', bytes: Uint8Array.from([137, 80, 78, 71, 1, 2, 3]) },
      { name: '语音.mp3', bytes: Uint8Array.from([73, 68, 51, 4]) },
      { name: '剪辑.mp4', bytes: Uint8Array.from([0, 0, 0, 24, 102, 116, 121, 112]) },
      { name: '说明.acl', bytes: encode('workspace "local"') },
      { name: '图纸.dwg', bytes: Uint8Array.from([1]) },
      { name: '缺失.txt', missing: true }
    ])
    await user.click(await screen.findByRole('button', { name: FOLDER }))
    await user.click(await screen.findByRole('button', { name: '图纸.dwg' }))
    await waitFor(() => {
      const opened = vi.mocked(fetch).mock.calls.flatMap(([url, init]) => {
        if (!String(url).includes('/api/workspace-file/open')) return []
        return [String((init as RequestInit | undefined)?.body ?? '')]
      })
      expect(opened.some((body) => body.includes(pathFor('图纸.dwg')))).toBe(true)
    })
    expect(screen.queryByText('暂不支持预览')).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '封面.png' }))
    const preview = await screen.findByTestId('image-preview-dialog')
    expect(preview).toHaveClass('z-[1100]')
    expect(preview).not.toHaveClass('z-[80]')
    expect(document.querySelector('[data-slot="dialog-overlay"]')).toHaveClass('z-[1100]')
    const large = preview.querySelector('img')
    expect(large).toHaveAttribute('alt', '封面.png')
    expect(large?.getAttribute('src')).toMatch(/^blob:/)
    const thumbnail = document.querySelector('[data-workspace-session-file="封面.png"] img')
    expect(thumbnail).toHaveAttribute('src', large?.getAttribute('src'))
    await user.click(within(preview).getByRole('button', { name: '关闭预览' }))
    expect(screen.queryByTestId('image-preview-dialog')).not.toBeInTheDocument()
    expect(document.querySelector('[data-workspace-session-file="封面.png"] img')).not.toBeNull()
    expect(screen.queryByRole('button', { name: '用默认应用打开' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '返回' })).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '语音.mp3' }))
    expect(await screen.findByRole('button', { name: '用默认应用打开' })).toBeInTheDocument()
    await waitFor(() => expect(document.querySelector('audio')).not.toBeNull())
    await user.click(screen.getByRole('button', { name: '返回' }))

    await user.click(screen.getByRole('button', { name: '剪辑.mp4' }))
    await waitFor(() => expect(document.querySelector('video')).not.toBeNull())
    await user.click(screen.getByRole('button', { name: '返回' }))

    await user.click(screen.getByRole('button', { name: '说明.acl' }))
    expect(await screen.findByRole('textbox', { name: '说明.acl' })).toHaveValue('workspace "local"')
    await user.click(screen.getByRole('button', { name: '返回' }))

    await user.click(screen.getByRole('button', { name: '缺失.txt' }))
    expect(await screen.findByText('文件不可用')).toBeInTheDocument()
    expect(screen.queryByPlaceholderText('在这里开始编写')).not.toBeInTheDocument()
  })

  it('gives office editors the file bytes and reads a saved office file back', async () => {
    const files = [
      ['说明.docx', 'DocumentEditor', [1, 2, 3, 4]],
      ['记录.md', 'MarkdownEditor', [5, 6]],
      ['表.csv', 'SpreadsheetEditor', [7]],
      ['幻灯.pptx', 'PresentationEditor', [8, 8]]
    ] as const
    const store = renderWorkspace([
      ...files.map(([name, , bytes]) => ({ name, bytes: Uint8Array.from(bytes) })),
      { name: '手册.pdf', bytes: Uint8Array.from([9, 0]) }
    ])
    const user = await openLocal('说明.docx')

    for (const [name, editor, bytes] of files) {
      if (name !== '说明.docx') {
        await user.click(screen.getByRole('button', { name: '返回' }))
        await user.click(screen.getByRole('button', { name }))
      }
      await waitFor(() => {
        const mount = latestMount(editor)
        expect(mount?.editor).toBe(editor)
        if (mount?.editor === 'PdfViewer') return
        expect(mount?.content.sourceBytes).toEqual([...bytes])
      })
      expect(screen.queryByPlaceholderText('在这里开始编写')).not.toBeInTheDocument()
    }

    await user.click(screen.getByRole('button', { name: '改内容' }))
    const slideEditor = document.querySelector('[data-editor="PresentationEditor"]')
    fireEvent.keyDown(window, { key: 's', metaKey: true })
    await waitFor(() => expect(Array.from(store.get(pathFor('幻灯.pptx')) ?? [])).toEqual(EDITED_OFFICE_BYTES))
    expect(office.exported.at(-1)).toEqual(EDITED_OFFICE_BYTES)
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 40))
    })
    expect(document.querySelector('[data-editor="PresentationEditor"]')).toBe(slideEditor)
    expect(screen.queryByText('正在加载预览...')).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '返回' }))
    await user.click(screen.getByRole('button', { name: '手册.pdf' }))
    const pdf = await waitFor(() => {
      const mount = latestMount('PdfViewer')
      if (mount?.editor !== 'PdfViewer') throw new Error('pdf')
      return mount
    })
    expect(Array.from(await blobBytes(await pdf.loadSource()))).toEqual([9, 0])
    expect(screen.queryByPlaceholderText('在这里开始编写')).not.toBeInTheDocument()
    const pdfEditor = document.querySelector('[data-editor="pdf"]')
    fireEvent.keyDown(window, { key: 's', metaKey: true })
    await waitFor(() => expect(Array.from(store.get(pathFor('手册.pdf')) ?? [])).toEqual(EDITED_PDF_BYTES))
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 40))
    })
    expect(document.querySelector('[data-editor="pdf"]')).toBe(pdfEditor)
    const savedPdf = latestMount('PdfViewer')
    if (savedPdf?.editor !== 'PdfViewer') throw new Error('pdf')
    expect(savedPdf.sourceKey).toBe(pdf.sourceKey)
    expect(savedPdf.loadSource).toBe(pdf.loadSource)
    expect(Array.from(await blobBytes(await pdf.loadSource()))).toEqual(EDITED_PDF_BYTES)

    await user.click(screen.getByRole('button', { name: '返回' }))
    await user.click(screen.getByRole('button', { name: '幻灯.pptx' }))
    await waitFor(() => {
      const mount = latestMount('PresentationEditor')
      if (mount?.editor !== 'PresentationEditor') throw new Error('presentation')
      expect(mount.content.sourceBytes).toEqual(EDITED_OFFICE_BYTES)
    })

    await user.click(screen.getByRole('button', { name: '返回' }))
    await user.click(screen.getByRole('button', { name: '手册.pdf' }))
    await waitFor(async () => {
      const mount = latestMount('PdfViewer')
      if (mount?.editor !== 'PdfViewer') throw new Error('pdf')
      expect(Array.from(await blobBytes(await mount.loadSource()))).toEqual(EDITED_PDF_BYTES)
    })
  })

  it('opens html as source and svg as an image, without a preview switch', async () => {
    const user = userEvent.setup()
    renderWorkspace([
      { name: '页面.html', bytes: encode('<p>hello</p>') },
      { name: '标记.svg', bytes: encode('<svg xmlns="http://www.w3.org/2000/svg"></svg>') }
    ])
    await user.click(await screen.findByRole('button', { name: FOLDER }))
    await user.click(await screen.findByRole('button', { name: '页面.html' }))
    expect(await screen.findByRole('textbox', { name: '页面.html' })).toHaveValue('<p>hello</p>')
    expect(document.querySelector('[data-testid="segmented-control"]')).toBeNull()

    await user.click(screen.getByRole('button', { name: '返回' }))
    await user.click(screen.getByRole('button', { name: '标记.svg' }))
    expect(await screen.findByRole('img', { name: '标记.svg' })).toBeInTheDocument()
    expect(document.querySelector('[data-testid="segmented-control"]')).toBeNull()
  })

  it('edits markdown source, keeps csv in the sheet, and opens a broken markdown file as text', async () => {
    const store = renderWorkspace([
      { name: '记录.md', bytes: encode('# 标题\n') },
      { name: '表.csv', bytes: encode('name,n\n甲,1\n') },
      { name: '损坏.md', bytes: encode('# 打不开\n') }
    ])
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: FOLDER }))
    await user.click(await screen.findByRole('button', { name: '记录.md' }))
    await waitFor(() => expect(latestMount('MarkdownEditor')?.editor).toBe('MarkdownEditor'))
    expect(latestMount('MarkdownEditor')?.presentation).toBe('note')
    await user.click(screen.getByRole('button', { name: '源码' }))
    const markdown = await screen.findByRole('textbox', { name: '记录.md' })
    expect(markdown).toHaveValue('# 标题\n')
    fireEvent.change(markdown, { target: { value: '# 已改\n' } })
    fireEvent.keyDown(markdown, { key: 's', metaKey: true })
    await waitFor(() => expect(new TextDecoder().decode(store.get(pathFor('记录.md')))).toBe('# 已改\n'))

    await user.click(screen.getByRole('button', { name: '返回' }))
    await user.click(screen.getByRole('button', { name: '表.csv' }))
    await waitFor(() => expect(latestMount('SpreadsheetEditor')?.editor).toBe('SpreadsheetEditor'))
    expect(document.querySelector('[data-testid="segmented-control"]')).toBeNull()

    await user.click(screen.getByRole('button', { name: '返回' }))
    await user.click(screen.getByRole('button', { name: '损坏.md' }))
    expect(await screen.findByRole('textbox', { name: '损坏.md' })).toHaveValue('# 打不开\n')
    expect(screen.queryByText('文件不可用')).not.toBeInTheDocument()
  })

  it('formats json and plain text, and keeps the draft when saving fails', async () => {
    const store = renderWorkspace([
      { name: '配置.json', bytes: encode('{"a":1}') },
      { name: '坏.json', bytes: encode('{') },
      { name: '纪要.txt', bytes: encode('甲  \n\n\n\n乙') }
    ])
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: FOLDER }))
    await user.click(await screen.findByRole('button', { name: '配置.json' }))
    const json = await screen.findByRole('textbox', { name: '配置.json' })
    expect(screen.queryByRole('button', { name: '格式化' })).not.toBeInTheDocument()
    fireEvent.contextMenu(json)
    const formatJson = await screen.findByRole('button', { name: /格式化/ })
    expect(formatJson.querySelector('svg')).toBeTruthy()
    fireEvent.click(formatJson)
    expect(json).toHaveValue('{\n  "a": 1\n}\n')
    fireEvent.keyDown(json, { key: 's', metaKey: true })
    await waitFor(() => expect(new TextDecoder().decode(store.get(pathFor('配置.json')))).toBe('{\n  "a": 1\n}\n'))
    expect(screen.getByRole('textbox', { name: '配置.json' })).toBe(json)
    expect(screen.getByText(/最近修改 \d{2}:\d{2}/u)).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '返回' }))
    await user.click(screen.getByRole('button', { name: '坏.json' }))
    const broken = await screen.findByRole('textbox', { name: '坏.json' })
    fireEvent.contextMenu(broken)
    fireEvent.click(await screen.findByRole('button', { name: /格式化/ }))
    expect(broken).toHaveValue('{')
    expect(await screen.findByRole('status')).toHaveTextContent('格式化失败，请检查文件格式')

    await user.click(screen.getByRole('button', { name: '返回' }))
    await user.click(screen.getByRole('button', { name: '纪要.txt' }))
    const notes = await screen.findByRole('textbox', { name: '纪要.txt' })
    fireEvent.contextMenu(notes)
    fireEvent.click(await screen.findByRole('button', { name: /格式化/ }))
    expect(notes).toHaveValue('甲\n\n\n乙\n')
    office.failSave = true
    fireEvent.keyDown(notes, { key: 's', metaKey: true })
    expect(await screen.findByText('无法保存此文件。自动保存已暂停，请重试或放弃修改。')).toBeInTheDocument()
    expect(new TextDecoder().decode(store.get(pathFor('纪要.txt')))).toBe('甲  \n\n\n\n乙')
  })

  it('does not write a text file again when the draft already matches disk', async () => {
    renderWorkspace([{ name: '纪要.txt', bytes: encode('你好') }])
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: FOLDER }))
    await user.click(await screen.findByRole('button', { name: '纪要.txt' }))
    const field = await screen.findByRole('textbox', { name: '纪要.txt' })
    fireEvent.keyDown(field, { key: 's', metaKey: true })
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 20))
    })
    expect(office.writes).toBe(0)

    fireEvent.change(field, { target: { value: '你好。' } })
    fireEvent.keyDown(field, { key: 's', metaKey: true })
    await waitFor(() => expect(office.writes).toBe(1))
  })

  it('keeps the later office edit when an earlier save is still writing', async () => {
    let releaseWrite: () => void = () => undefined
    office.holdWrite = new Promise((resolve) => {
      releaseWrite = resolve
    })
    const store = renderWorkspace([{ name: '幻灯.pptx', bytes: Uint8Array.from([8, 8]) }])
    const user = await openLocal('幻灯.pptx')
    await user.click(await screen.findByRole('button', { name: '改内容' }))
    fireEvent.keyDown(window, { key: 's', metaKey: true })
    await waitFor(() => expect(office.writes).toBe(1))
    const slide = document.querySelector('[data-editor="PresentationEditor"]')
    await user.click(screen.getByRole('button', { name: '再改' }))
    fireEvent.keyDown(window, { key: 's', metaKey: true })
    releaseWrite()
    await waitFor(() => expect(Array.from(store.get(pathFor('幻灯.pptx')) ?? [])).toEqual([4, 3, 2, 9]))
    expect(office.writes).toBe(2)
    expect(document.querySelector('[data-editor="PresentationEditor"]')).toBe(slide)
    expect(screen.queryByText('正在加载预览...')).not.toBeInTheDocument()
  })

  it('returns markdown from source to the visual editor', async () => {
    renderWorkspace([{ name: '记录.md', bytes: encode('# 标题\n') }])
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: FOLDER }))
    await user.click(await screen.findByRole('button', { name: '记录.md' }))
    await waitFor(() => expect(latestMount('MarkdownEditor')?.presentation).toBe('note'))
    await user.click(screen.getByRole('button', { name: '源码' }))
    expect(await screen.findByRole('textbox', { name: '记录.md' })).toHaveValue('# 标题\n')
    await user.click(screen.getByRole('button', { name: '预览' }))
    await waitFor(() => expect(document.querySelector('[data-editor="MarkdownEditor"]')).toBeTruthy())
    expect(screen.queryByRole('textbox', { name: '记录.md' })).not.toBeInTheDocument()
    const host = document.querySelector('[data-workspace-preview]')
    expect(host?.className).toContain('flex')
    expect(host?.className).toContain('h-full')
    expect(host?.className).toContain('flex-col')
    expect(host?.contains(document.querySelector('[data-editor="MarkdownEditor"]'))).toBe(true)
  })

  it('keeps a visual markdown draft in source mode', async () => {
    renderWorkspace([{ name: '记录.md', bytes: encode('# 标题\n') }])
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: FOLDER }))
    await user.click(await screen.findByRole('button', { name: '记录.md' }))
    await user.click(await screen.findByRole('button', { name: '写入草稿' }))
    await user.click(screen.getByRole('button', { name: '源码' }))
    expect(await screen.findByRole('textbox', { name: '记录.md' })).toHaveValue('# 草稿\n')
  })

  it('opens a large markdown file as source text', async () => {
    const body = 'a'.repeat(VISUAL_TEXT_UNITS + 1)
    renderWorkspace([{ name: '大.md', bytes: encode(body) }])
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: FOLDER }))
    await user.click(await screen.findByRole('button', { name: '大.md' }))
    const field = await screen.findByRole('textbox', { name: '大.md' })
    expect((field as HTMLTextAreaElement).value).toHaveLength(body.length)
    expect(document.querySelector('[data-editor="MarkdownEditor"]')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '预览' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '格式化' })).not.toBeInTheDocument()
    fireEvent.contextMenu(field)
    expect(await screen.findByRole('button', { name: /复制/ })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /格式化/ })).not.toBeInTheDocument()
  })

  it('cuts, copies, pastes, and selects all from the editor menu', async () => {
    renderWorkspace([{ name: '纪要.txt', bytes: encode('你好') }])
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: FOLDER }))
    await user.click(await screen.findByRole('button', { name: '纪要.txt' }))
    const field = (await screen.findByRole('textbox', { name: '纪要.txt' })) as HTMLTextAreaElement
    const clip = { text: '' }
    const previous = Object.getOwnPropertyDescriptor(navigator, 'clipboard')
    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: {
        writeText: async (text: string) => {
          clip.text = text
        },
        readText: async () => clip.text
      }
    })
    const choose = async (name: RegExp) => {
      fireEvent.contextMenu(field)
      fireEvent.click(await screen.findByRole('button', { name }))
      await waitFor(() => expect(screen.queryByRole('menu', { name: '编辑器' })).not.toBeInTheDocument())
    }
    try {
      await choose(/全选/)
      expect(field.selectionStart).toBe(0)
      expect(field.selectionEnd).toBe(2)

      await choose(/复制/)
      expect(clip.text).toBe('你好')

      field.setSelectionRange(0, 1)
      await choose(/剪切/)
      expect(field).toHaveValue('好')
      expect(clip.text).toBe('你')

      field.setSelectionRange(1, 1)
      await choose(/粘贴/)
      await waitFor(() => expect(field).toHaveValue('好你'))

      fireEvent.keyDown(field, { key: 'f', shiftKey: true, altKey: true })
      expect(field).toHaveValue('好你\n')
    } finally {
      if (previous) Object.defineProperty(navigator, 'clipboard', previous)
      else Reflect.deleteProperty(navigator, 'clipboard')
    }
  })

  it('does not read a file past the text budget into the editor', async () => {
    renderWorkspace([{ name: '过大.txt', bytes: encode('x') }])
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: FOLDER }))
    await user.click(await screen.findByRole('button', { name: '过大.txt' }))
    expect(await screen.findByRole('heading', { name: '文件过大' })).toBeInTheDocument()
    expect(screen.getByText('这个文件有 20MB。应用内编辑器不打开它。')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '用默认应用打开' })).toBeInTheDocument()
    expect(screen.queryByRole('textbox', { name: '过大.txt' })).not.toBeInTheDocument()
  })

  it('opens a 32.1MB document in the editor and still refuses one past the raw limit', async () => {
    renderWorkspace([
      { name: '揭榜.docx', bytes: Uint8Array.from([1]) },
      { name: '超限.docx', bytes: Uint8Array.from([1]) }
    ])
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: FOLDER }))
    await user.click(await screen.findByRole('button', { name: '揭榜.docx' }))
    expect(await screen.findByRole('button', { name: '改内容' })).toBeInTheDocument()
    expect(document.querySelector('[data-editor="DocumentEditor"]')).not.toBeNull()
    expect(screen.queryByRole('heading', { name: '文件过大' })).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '返回' }))
    await user.click(await screen.findByRole('button', { name: '超限.docx' }))
    expect(await screen.findByRole('heading', { name: '文件过大' })).toBeInTheDocument()
    expect(screen.getByText('这个文件有 64MB。应用内编辑器不打开它。')).toBeInTheDocument()
    expect(document.querySelector('[data-editor="DocumentEditor"]')).toBeNull()
  })
})
