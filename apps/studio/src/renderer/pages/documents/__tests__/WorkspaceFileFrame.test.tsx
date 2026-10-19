import { createHash } from 'node:crypto'

import { workspaceDocumentsFixture } from '@test-mocks/renderer/workspaceDocuments'
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { DocumentOpenView } from '../DocumentOpenView'
import { TEXT_READ_BYTES } from '../fileOpenBudget'
import { localDocumentFromFile } from '../useLocalDocuments'

const EDITED_OFFICE_BYTES = [4, 3, 2, 1]
const LATER_OFFICE_BYTES = [4, 3, 2, 9]

vi.mock('@a3s-lab/office/core', async () => {
  const { previewKind: kindForName } = await import('../documentPreview')

  async function readFileBytes(file: Blob): Promise<Uint8Array> {
    try {
      if (typeof file.arrayBuffer === 'function') return new Uint8Array(await file.arrayBuffer())
    } catch {
      // The workspace file fixture hands the editor a Blob without arrayBuffer.
    }
    return await new Promise((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => resolve(new Uint8Array(reader.result as ArrayBuffer))
      reader.onerror = () => reject(reader.error ?? new Error('read'))
      reader.readAsArrayBuffer(file)
    })
  }

  return {
    importOfficeFile: async (file: File) => {
      const kind = kindForName(file.name)
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
    createArtifactBlob: async (artifact: { content: { sourceBytes: number[] } }) => {
      const data = Uint8Array.from(artifact.content.sourceBytes)
      return { arrayBuffer: async () => data.slice().buffer }
    }
  }
})

vi.mock('@a3s-lab/office/react', async () => {
  const { createElement, useEffect } = await import('react')
  function recordEditor(editor: string) {
    return function Editor(props: {
      content: { type: string; sourceBytes: number[] }
      onChange?: (content: { type: string; sourceBytes?: number[]; markdown?: string }) => void
    }) {
      return createElement(
        'div',
        { 'data-editor': editor },
        props.content.type === 'markdown'
          ? createElement(
              'button',
              { type: 'button', onClick: () => props.onChange?.({ type: 'markdown', markdown: '# 草稿\n' }) },
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
            onClick: () => props.onChange?.({ ...props.content, sourceBytes: LATER_OFFICE_BYTES })
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
    PdfViewer: (props: { onSave?: (pdf: Blob) => Promise<boolean> }) => {
      useEffect(() => {
        const onKey = (event: KeyboardEvent) => {
          if (!(event.metaKey || event.ctrlKey) || event.altKey || event.shiftKey) return
          if (event.key.toLowerCase() !== 's') return
          event.preventDefault()
          const data = Uint8Array.from(EDITED_OFFICE_BYTES)
          void props.onSave?.(new Blob([data], { type: 'application/pdf' }))
        }
        window.addEventListener('keydown', onKey)
        return () => window.removeEventListener('keydown', onKey)
      }, [props.onSave])
      return createElement('div', { 'data-editor': 'pdf' })
    }
  }
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

function expectSharedChrome(container: HTMLElement) {
  const toolbar = container.querySelector('[data-workspace-toolbar]')
  expect(toolbar).toBeTruthy()
  expect(toolbar?.className).toContain('h-9')
  expect(toolbar?.className).toContain('border-[#ececec]')
  expect(toolbar?.className).toContain('bg-white')
  expect(toolbar?.parentElement?.className).toContain('bg-white')
  expect(toolbar?.parentElement?.className).toContain('h-full')
  expect(container.querySelector('select, datalist')).toBeNull()
}

function mountWorkspace(
  name: string,
  bytes: Uint8Array | null,
  options?: {
    readOnly?: boolean
    onOpenOutside?: () => void
    mode?: 'missing' | 'too-large' | 'hold'
    holdPut?: boolean
  }
) {
  const path = `/tmp/session/${name}`
  const store = new Map<string, Uint8Array>()
  if (bytes) store.set(path, bytes)
  let puts = 0
  let putsStarted = 0
  let release = () => undefined
  let releasePut = () => undefined
  const putGate = options?.holdPut
    ? new Promise<void>((resolve) => {
        releasePut = resolve
      })
    : null
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    configurable: true,
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
  })
  const revision = () =>
    createHash('sha256')
      .update(store.get(path) ?? new Uint8Array())
      .digest('hex')
  const headers = { get: (header: string) => (header.toLowerCase() === 'etag' ? `"${revision()}"` : null) }
  const diskFetch = async (url: string, init?: RequestInit) => {
    const text = String(url)
    if (!text.includes('/api/workspace-file/snapshot') && !text.includes('/api/workspace-file/raw')) {
      return { ok: false, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
    }
    const method = (init?.method ?? 'GET').toUpperCase()
    if (options?.mode === 'hold' && method === 'GET') {
      await new Promise<void>((resolve) => {
        release = resolve
      })
    }
    if (options?.mode === 'missing' && method === 'GET') {
      return { ok: false, status: 404, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
    }
    if (options?.mode === 'too-large' && method === 'GET') {
      return {
        ok: true,
        status: 200,
        headers: {
          get: (header: string) => (header.toLowerCase() === 'content-length' ? String(TEXT_READ_BYTES + 8) : null)
        },
        body: { cancel: async () => undefined },
        json: async () => ({}),
        arrayBuffer: async () => new ArrayBuffer(0)
      }
    }
    if (method === 'PUT') {
      putsStarted += 1
      if (putGate) await putGate
      puts += 1
      if (new globalThis.Headers(init?.headers).get('if-match') !== `"${revision()}"`) return { ok: false, status: 409 }
      store.set(new URL(text).searchParams.get('path') ?? path, await requestBytes(init?.body))
      return { ok: true, headers, json: async () => ({ ok: true }), arrayBuffer: async () => new ArrayBuffer(0) }
    }
    const stored = store.get(new URL(text).searchParams.get('path') ?? path)
    if (!stored) return { ok: false, status: 404, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
    const copy = Uint8Array.from(stored)
    return { ok: true, status: 200, headers, json: async () => ({}), arrayBuffer: async () => copy.buffer }
  }
  const documents = workspaceDocumentsFixture(diskFetch)
  vi.stubGlobal(
    'fetch',
    vi.fn((url: string, init?: globalThis.RequestInit) =>
      String(url).includes('/api/workspace-documents') ? documents(init) : diskFetch(url, init)
    )
  )
  const view = render(
    <DocumentOpenView
      document={localDocumentFromFile({
        name,
        path,
        sizeBytes: bytes?.byteLength ?? 0,
        modifiedMs: 1,
        folder: ''
      })}
      workspaceRoot="/tmp/session"
      cloudReady={false}
      frame="workspace"
      readOnly={options?.readOnly}
      onOpenOutside={options?.onOpenOutside}
    />
  )
  return {
    ...view,
    store,
    path,
    puts: () => puts,
    putsStarted: () => putsStarted,
    release: () => release(),
    releasePut: () => releasePut()
  }
}

describe('workspace file frame', () => {
  afterEach(() => {
    cleanup()
    vi.unstubAllGlobals()
    delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
  })

  it('edits plain text in the tab and saves with the shortcut', async () => {
    const store = new Map<string, Uint8Array>()
    const textPath = '/tmp/session/notes.txt'
    store.set(textPath, encode('hello'))
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
    })
    const revision = () =>
      createHash('sha256')
        .update(store.get(textPath) ?? new Uint8Array())
        .digest('hex')
    const headers = { get: (name: string) => (name.toLowerCase() === 'etag' ? `"${revision()}"` : null) }
    const diskFetch = async (url: string, init?: RequestInit) => {
      const text = String(url)
      if (!text.includes('/api/workspace-file/snapshot')) {
        return { ok: false, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
      }
      const path = new URL(text).searchParams.get('path') ?? ''
      if ((init?.method ?? 'GET').toUpperCase() === 'PUT') {
        if (new globalThis.Headers(init?.headers).get('if-match') !== `"${revision()}"`)
          return { ok: false, status: 409 }
        store.set(path, await requestBytes(init?.body))
        return { ok: true, headers, json: async () => ({ ok: true }), arrayBuffer: async () => new ArrayBuffer(0) }
      }
      const bytes = store.get(path)
      if (!bytes) return { ok: false, status: 404, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
      const copy = Uint8Array.from(bytes)
      return { ok: true, status: 200, headers, json: async () => ({}), arrayBuffer: async () => copy.buffer }
    }
    const documents = workspaceDocumentsFixture(diskFetch)
    vi.stubGlobal(
      'fetch',
      vi.fn((url: string, init?: globalThis.RequestInit) =>
        String(url).includes('/api/workspace-documents') ? documents(init) : diskFetch(url, init)
      )
    )
    render(
      <DocumentOpenView
        document={localDocumentFromFile({
          name: 'notes.txt',
          path: textPath,
          sizeBytes: 5,
          modifiedMs: 1,
          folder: ''
        })}
        workspaceRoot="/tmp/session"
        cloudReady={false}
        frame="workspace"
      />
    )
    const box = await screen.findByRole('textbox', { name: 'notes.txt' })
    expect(document.querySelector('[data-workspace-toolbar]')).toBeNull()
    expect(document.querySelector('select, datalist')).toBeNull()
    expect(screen.queryByRole('button', { name: '保存' })).not.toBeInTheDocument()
    fireEvent.change(box, { target: { value: 'hello saved' } })
    expect(screen.getByText('未保存')).toBeInTheDocument()
    const dirtyToolbar = document.querySelector('[data-workspace-toolbar]')
    expect(dirtyToolbar?.className).toContain('h-9')
    expect(dirtyToolbar?.className).toContain('border-[#ececec]')
    expect(dirtyToolbar?.className).toContain('bg-white')
    expect(dirtyToolbar?.parentElement?.className).toContain('bg-white')
    expect(dirtyToolbar?.querySelector('[data-testid="segmented-control"]')).toBeNull()
    fireEvent.keyDown(box, { key: 's', metaKey: true })
    await waitFor(() => expect(new TextDecoder().decode(store.get(textPath))).toBe('hello saved'))
    expect(screen.queryByText('未保存')).not.toBeInTheDocument()
    expect(document.querySelector('[data-workspace-toolbar]')).toContainElement(
      screen.getByText(/最近修改 \d{2}:\d{2}/u)
    )
  })

  it('refuses a stale text write and does not replace the newer bytes', async () => {
    const { store, path } = mountWorkspace('notes.txt', encode('hello'))
    const box = await screen.findByRole('textbox', { name: 'notes.txt' })
    store.set(path, encode('newer'))
    fireEvent.change(box, { target: { value: 'hello saved' } })
    fireEvent.keyDown(box, { key: 's', metaKey: true })
    await waitFor(() => expect(screen.getByText(/无法保存此文件/u)).toBeInTheDocument())
    expect(new TextDecoder().decode(store.get(path))).toBe('newer')
  })

  it('does not write a read-only text file', async () => {
    const { store, path, puts } = mountWorkspace('notes.txt', encode('hello'), { readOnly: true })
    const box = await screen.findByRole('textbox', { name: 'notes.txt' })
    fireEvent.change(box, { target: { value: 'changed' } })
    fireEvent.keyDown(box, { key: 's', metaKey: true })
    expect(box).toHaveValue('hello')
    expect(screen.queryByText('未保存')).not.toBeInTheDocument()
    expect(puts()).toBe(0)
    expect(new TextDecoder().decode(store.get(path))).toBe('hello')
  })

  it('keeps the markdown source switch in the shared toolbar', async () => {
    const text = '# readme\n'
    const { container } = mountWorkspace('readme.md', encode(text))
    await screen.findByRole('button', { name: '写入草稿' })
    expectSharedChrome(container)
    const toolbar = container.querySelector('[data-workspace-toolbar]')
    const mode = toolbar?.querySelector('[data-testid="segmented-control"]')
    expect(mode).toBeTruthy()
    expect(mode?.getAttribute('data-value')).toBe('preview')
    fireEvent.click(screen.getByRole('button', { name: '源码' }))
    expect(await screen.findByRole('textbox', { name: 'readme.md' })).toHaveValue(text)
    expect(toolbar?.contains(screen.getByRole('button', { name: '预览' }))).toBe(true)
    fireEvent.click(screen.getByRole('button', { name: '预览' }))
    fireEvent.click(await screen.findByRole('button', { name: '写入草稿' }))
    expect(screen.getByText('未保存')).toBeInTheDocument()
  })

  it.each([
    ['brief.docx', 'DocumentEditor'],
    ['sheet.xlsx', 'SpreadsheetEditor'],
    ['deck.pptx', 'PresentationEditor'],
    ['brief.pdf', 'pdf']
  ] as const)('opens %s in the white workspace pane', async (name, editor) => {
    const { container } = mountWorkspace(name, Uint8Array.from([1, 2, 3, 4]))
    const surface = await waitFor(() => {
      const node = container.querySelector(`[data-editor="${editor}"]`)
      expect(node).toBeTruthy()
      return node
    })
    expect(container.querySelector('[data-workspace-toolbar]')).toBeNull()
    const pane = surface?.parentElement?.parentElement?.parentElement
    expect(pane?.className).toContain('bg-white')
    expect(pane?.className).toContain('h-full')
    expect(container.querySelector('[data-testid="segmented-control"]')).toBeNull()
    expect(container.querySelector('select, datalist')).toBeNull()
  })

  it('shows a word document as unsaved and writes it with the shortcut', async () => {
    const { container, store, path } = mountWorkspace('brief.docx', Uint8Array.from([1, 2, 3, 4]))
    await screen.findByRole('button', { name: '改内容' })
    expect(container.querySelector('[data-workspace-toolbar]')).toBeNull()
    fireEvent.click(screen.getByRole('button', { name: '改内容' }))
    expect(screen.getByText('未保存')).toBeInTheDocument()
    expectSharedChrome(container)
    fireEvent.keyDown(window, { key: 's', metaKey: true })
    await waitFor(() => expect(Array.from(store.get(path) ?? [])).toEqual(EDITED_OFFICE_BYTES))
    expect(screen.queryByText('未保存')).not.toBeInTheDocument()
    expect(screen.getByText(/最近修改 \d{2}:\d{2}/u)).toBeInTheDocument()
    expectSharedChrome(container)
  })

  it('keeps a word document dirty when it changes while the save is writing', async () => {
    const view = mountWorkspace('brief.docx', Uint8Array.from([1, 2, 3, 4]), { holdPut: true })
    try {
      await screen.findByRole('button', { name: '改内容' })
      fireEvent.click(screen.getByRole('button', { name: '改内容' }))
      expect(screen.getByText('未保存')).toBeInTheDocument()
      fireEvent.keyDown(window, { key: 's', metaKey: true })
      await waitFor(() => expect(view.putsStarted()).toBe(1))
      fireEvent.click(screen.getByRole('button', { name: '再改' }))
      view.releasePut()
      await waitFor(() => expect(Array.from(view.store.get(view.path) ?? [])).toEqual(EDITED_OFFICE_BYTES))
      expect(screen.getByText('未保存')).toBeInTheDocument()
    } finally {
      view.releasePut()
    }
  })

  it('does not write a read-only word document', async () => {
    const { puts } = mountWorkspace('brief.docx', Uint8Array.from([1, 2, 3, 4]), { readOnly: true })
    await screen.findByRole('button', { name: '改内容' })
    fireEvent.click(screen.getByRole('button', { name: '改内容' }))
    fireEvent.keyDown(window, { key: 's', metaKey: true })
    await waitFor(() => expect(screen.queryByText('未保存')).not.toBeInTheDocument())
    expect(puts()).toBe(0)
  })

  it.each([
    ['photo.png', 'img'],
    ['clip.mp3', 'audio'],
    ['clip.mp4', 'video']
  ] as const)('opens %s on the white media pane', async (name, tag) => {
    const open = vi.fn()
    const { container } = mountWorkspace(name, Uint8Array.from([1, 2, 3, 4]), { onOpenOutside: open })
    await waitFor(() => expect(container.querySelector(tag)).toBeTruthy())
    expectSharedChrome(container)
    expect(container.innerHTML).not.toContain('bg-[#f7f7f7]')
    const action = screen.getByRole('button', { name: '用默认应用打开' })
    expect(action.className).toContain('h-9')
    expect(action.className).toContain('bg-[#f5f5f5]')
    fireEvent.click(action)
    expect(open).toHaveBeenCalledOnce()
  })

  it('keeps loading, too-large, unavailable, and unsupported in one pane notice', async () => {
    const loading = mountWorkspace('notes.txt', encode('hello'), { mode: 'hold' })
    const loadingStatus = await screen.findByRole('status')
    expect(loadingStatus).toHaveAttribute('data-workspace-notice')
    expect(loadingStatus.className).toContain('bg-white')
    expect(loadingStatus.className).not.toContain('mt-8')
    expect(screen.getByRole('heading', { name: '正在加载预览...' })).toBeInTheDocument()
    expect(screen.getByText('文件在此窗格中打开。')).toBeInTheDocument()
    loading.unmount()
    loading.release()

    const open = vi.fn()
    const cases = [
      ['huge.txt', 'too-large', '文件过大', '应用内编辑器不打开它。'],
      ['gone.txt', 'missing', '文件不可用', '文件可能已被移动、删除或无法访问。'],
      ['archive.zip', 'ready', '暂不支持预览', '此文件类型暂不支持在应用内预览，可以用默认应用打开。']
    ] as const
    for (const [name, mode, title, description] of cases) {
      const view = mountWorkspace(name, Uint8Array.from([1]), {
        mode: mode === 'ready' ? undefined : mode,
        onOpenOutside: open
      })
      const status = await screen.findByRole('status')
      expect(status).toHaveAttribute('data-workspace-notice')
      expect(status.className).toContain('bg-white')
      expect(status.className).not.toContain('mt-8')
      expect(screen.getByRole('heading', { name: title })).toBeInTheDocument()
      expect(status.querySelector('p')?.className).toContain('text-[#8a8a8a]')
      expect(status.querySelector('p')).toHaveTextContent(description)
      const action = screen.getByRole('button', { name: '用默认应用打开' })
      expect(action.className).toContain('h-9')
      expect(action.className).toContain('bg-[#f5f5f5]')
      expect(status.querySelector('select, datalist')).toBeNull()
      fireEvent.click(action)
      view.unmount()
    }
    expect(open).toHaveBeenCalledTimes(3)
  })
})
