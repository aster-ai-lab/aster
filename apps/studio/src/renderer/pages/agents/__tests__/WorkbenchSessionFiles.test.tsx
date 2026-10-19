import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { WorkbenchSessionFiles } from '../WorkbenchSessionFiles'
import { useWorkbenchWorkspaceTabs, WorkbenchWorkspaceTabsProvider } from '../workbenchWorkspaceTabs'

const pane = vi.hoisted(() => ({ maximized: false }))

vi.mock('@renderer/components/chat/panes/Shell', () => ({
  useRightPanelPresentationMaximized: () => pane.maximized,
  useRightPanelState: () => ({
    presentationOpen: false,
    presentationMaximized: pane.maximized,
    activePanelId: undefined
  })
}))

function EditorProbe() {
  const api = useWorkbenchWorkspaceTabs()
  const editors = api.tabs.filter((tab) => tab.kind === 'editor')
  return <div data-editor-path={editors.map((tab) => tab.path).join('|')} data-editor-count={editors.length} />
}

const runtime = {
  apiBaseUrl: 'http://127.0.0.1:4318',
  token: 'ab'.repeat(32)
}

const rootEntries = {
  entries: [
    { name: 'Notes', path: '/tmp/session/Notes', kind: 'directory', sizeBytes: 0, modifiedMs: 3 },
    {
      name: 'root.txt',
      path: '/tmp/session/root.txt',
      kind: 'file',
      sizeBytes: 2,
      modifiedMs: 2,
      agentRetrievals: 4
    }
  ],
  capped: false
}

const noteEntries = {
  entries: [{ name: 'note.txt', path: '/tmp/session/Notes/note.txt', kind: 'file', sizeBytes: 1, modifiedMs: 1 }],
  capped: false
}

function installRuntime() {
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    configurable: true,
    value: runtime
  })
}

function eventStream(frame: string, signal?: AbortSignal) {
  const payload = new TextEncoder().encode(frame)
  let sent = false
  return {
    getReader() {
      return {
        async read(): Promise<{ done: boolean; value?: Uint8Array }> {
          if (signal?.aborted) throw new DOMException('aborted', 'AbortError')
          if (!sent) {
            sent = true
            return { done: false, value: payload }
          }
          await new Promise<void>((_resolve, reject) => {
            const abort = () => reject(new DOMException('aborted', 'AbortError'))
            if (signal?.aborted) abort()
            else signal?.addEventListener('abort', abort, { once: true })
          })
          return { done: true }
        },
        cancel() {},
        releaseLock() {}
      }
    }
  }
}

function listing(url: string) {
  const path = new URL(url, 'http://127.0.0.1').searchParams.get('path') ?? ''
  if (path.endsWith('/Notes')) return noteEntries
  return rootEntries
}

describe('WorkbenchSessionFiles', () => {
  afterEach(() => {
    pane.maximized = false
    vi.unstubAllGlobals()
    vi.stubGlobal(
      'ResizeObserver',
      class ResizeObserver {
        observe() {}
        unobserve() {}
        disconnect() {}
      }
    )
    Reflect.deleteProperty(globalThis, '__A3S_STUDIO_RUNTIME__')
  })

  it('lists the session folder and its files in the documents-style tree', async () => {
    installRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo) => ({
        ok: true,
        json: async () => listing(String(input))
      }))
    )

    render(<WorkbenchSessionFiles root="/tmp/session" />)

    expect(await screen.findByRole('button', { name: 'Notes' })).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('button', { name: '会话工作区' })).toBeInTheDocument()
    await waitFor(() => {
      expect(document.querySelector('[data-workspace-session-file="note.txt"]')).toHaveAttribute(
        'data-workspace-session-file-folder',
        'Notes'
      )
    })
    expect(document.querySelector('[data-workspace-session-file="root.txt"]')).toHaveAttribute(
      'data-workspace-session-file-folder',
      ''
    )
    expect(screen.getByRole('navigation', { name: '会话文件夹' })).toHaveAttribute(
      'data-workspace-session-root',
      '/tmp/session'
    )
    expect(screen.getByRole('button', { name: '新建文件' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '新建文件夹' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '最近' })).toBeInTheDocument()

    const notes = screen.getByRole('button', { name: 'Notes' })
    const disclosure = notes.parentElement?.querySelector('[data-workspace-session-disclosure]')
    expect(disclosure).toBe(notes.previousElementSibling)
    fireEvent.click(disclosure as HTMLButtonElement)
    expect(notes).toHaveAttribute('aria-expanded', 'false')
    expect(document.querySelector('[data-workspace-session-file="note.txt"]')).toBeNull()
    fireEvent.click(disclosure as HTMLButtonElement)
    await waitFor(() => {
      expect(notes).toHaveAttribute('aria-expanded', 'true')
    })
    expect(document.querySelector('[data-workspace-session-file="note.txt"]')).not.toBeNull()
  })

  it('renames from the file menu and creates a file inline', async () => {
    installRuntime()
    const fetchMock = vi.fn(async (input: RequestInfo, init?: RequestInit) => {
      if (init?.method === 'POST') {
        return {
          ok: true,
          status: 200,
          json: async () => ({
            name: 'fresh.txt',
            path: '/tmp/session/fresh.txt',
            kind: 'file',
            sizeBytes: 0,
            modifiedMs: 1
          })
        }
      }
      return { ok: true, status: 200, json: async () => listing(String(input)) }
    })
    vi.stubGlobal('fetch', fetchMock)
    const user = userEvent.setup()

    render(<WorkbenchSessionFiles root="/tmp/session" />)

    await waitFor(() => {
      expect(document.querySelector('[data-workspace-session-file="root.txt"]')).not.toBeNull()
    })
    fireEvent.contextMenu(document.querySelector('[data-workspace-session-file="root.txt"]') as HTMLButtonElement)
    await user.click(await screen.findByRole('menuitem', { name: '重命名' }))
    const rename = await screen.findByRole('textbox', { name: '重命名' })
    await user.clear(rename)
    await user.type(rename, 'renamed.txt{Enter}')
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        'http://127.0.0.1:4318/api/local-documents/rename',
        expect.objectContaining({ method: 'POST' })
      )
    })

    await user.click(screen.getByRole('button', { name: '新建文件' }))
    const name = await screen.findByRole('textbox', { name: '名称' })
    await user.clear(name)
    await user.type(name, 'fresh.txt{Enter}')
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        'http://127.0.0.1:4318/api/local-documents/entries',
        expect.objectContaining({ method: 'POST' })
      )
    })

    await user.click(screen.getByRole('button', { name: '新建文件' }))
    const shown = await screen.findByRole('textbox', { name: '名称' })
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set
    setter?.call(shown, 'shown.txt')
    fireEvent.submit(shown.closest('form') as HTMLFormElement)
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        'http://127.0.0.1:4318/api/local-documents/entries',
        expect.objectContaining({
          method: 'POST',
          body: expect.stringContaining('shown.txt')
        })
      )
    })
  })

  it('shows how many times an agent retrieved each file', async () => {
    installRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo) => ({
        ok: true,
        json: async () => listing(String(input))
      }))
    )

    render(<WorkbenchSessionFiles layout="split" root="/tmp/session" />)

    expect(await screen.findByRole('columnheader', { name: '检索' })).toBeInTheDocument()
    expect(document.querySelector('[data-workspace-session-file="root.txt"]')).not.toBeNull()
    expect(document.querySelector('[data-agent-retrievals="4"]')).not.toBeNull()
  })

  it('shows a folder tree and a file list when the workspace is maximized', async () => {
    pane.maximized = true
    installRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo) => ({
        ok: true,
        json: async () => listing(String(input))
      }))
    )
    const user = userEvent.setup()

    render(<WorkbenchSessionFiles root="/tmp/session" />)

    expect(document.querySelector('[data-workspace-session-layout="split"]')).not.toBeNull()
    expect(await screen.findByRole('button', { name: 'Notes' })).toHaveAttribute('aria-expanded', 'true')
    expect(
      document.querySelector('[data-workspace-session-list] [data-workspace-session-file="root.txt"]')
    ).not.toBeNull()
    expect(document.querySelector('[data-workspace-session-file="note.txt"]')).toBeNull()
    expect(screen.getByRole('columnheader', { name: '文件名' })).toBeInTheDocument()

    const notes = screen.getByRole('button', { name: 'Notes' })
    const disclosure = notes.parentElement?.querySelector('[data-workspace-session-disclosure]')
    expect(disclosure).toBe(notes.previousElementSibling)
    await user.click(disclosure as HTMLButtonElement)
    expect(notes).toHaveAttribute('aria-expanded', 'false')

    await user.click(notes)

    await waitFor(() => {
      expect(
        document.querySelector('[data-workspace-session-list] [data-workspace-session-file="note.txt"]')
      ).not.toBeNull()
    })
    expect(notes).toHaveAttribute('aria-expanded', 'true')
    expect(document.querySelector('[data-workspace-session-list] [data-workspace-session-file="root.txt"]')).toBeNull()
    await user.click(notes)
    expect(notes).toHaveAttribute('aria-expanded', 'false')
    expect(
      document.querySelector('[data-workspace-session-list] [data-workspace-session-file="note.txt"]')
    ).not.toBeNull()
  })

  it('opens a supported file in an editor tab and uses the system only for unsupported files', async () => {
    installRuntime()
    const fetchMock = vi.fn(async (input: RequestInfo, init?: RequestInit) => {
      if (init?.method === 'POST') return { ok: true, status: 200, json: async () => ({ ok: true }) }
      const path = new URL(String(input), 'http://127.0.0.1').searchParams.get('path') ?? ''
      if (path.endsWith('/Notes')) return { ok: true, status: 200, json: async () => noteEntries }
      return {
        ok: true,
        status: 200,
        json: async () => ({
          entries: [
            ...rootEntries.entries,
            { name: 'blob.bin', path: '/tmp/session/blob.bin', kind: 'file', sizeBytes: 4, modifiedMs: 1 },
            { name: '001_init.sql', path: '/tmp/session/001_init.sql', kind: 'file', sizeBytes: 8, modifiedMs: 4 }
          ],
          capped: false
        })
      }
    })
    vi.stubGlobal('fetch', fetchMock)

    render(
      <WorkbenchWorkspaceTabsProvider>
        <EditorProbe />
        <WorkbenchSessionFiles root="/tmp/session" />
      </WorkbenchWorkspaceTabsProvider>
    )
    await waitFor(() => {
      expect(document.querySelector('[data-workspace-session-file="root.txt"]')).not.toBeNull()
    })

    const file = () => document.querySelector('[data-workspace-session-file="root.txt"]') as HTMLButtonElement
    const opened = () => document.querySelector('[data-editor-path]') as HTMLElement
    fireEvent.click(file())
    await new Promise((resolve) => setTimeout(resolve, 500))
    expect(fetchMock.mock.calls.some((call) => String(call[0]).includes('/api/workspace-file/open'))).toBe(false)
    expect(opened()).toHaveAttribute('data-editor-count', '0')

    fireEvent.doubleClick(file())
    await waitFor(() => expect(opened()).toHaveAttribute('data-editor-path', '/tmp/session/root.txt'))
    expect(opened()).toHaveAttribute('data-editor-count', '1')
    expect(screen.queryByRole('textbox', { name: '重命名' })).not.toBeInTheDocument()
    expect(fetchMock.mock.calls.some((call) => String(call[0]).includes('/api/workspace-file/open'))).toBe(false)

    fireEvent.doubleClick(file())
    expect(opened()).toHaveAttribute('data-editor-count', '1')

    fireEvent.doubleClick(document.querySelector('[data-workspace-session-file="001_init.sql"]') as HTMLButtonElement)
    await waitFor(() => expect(opened()).toHaveAttribute('data-editor-count', '2'))
    expect(opened().getAttribute('data-editor-path')).toContain('/tmp/session/001_init.sql')

    fireEvent.doubleClick(document.querySelector('[data-workspace-session-file="blob.bin"]') as HTMLButtonElement)
    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        'http://127.0.0.1:4318/api/workspace-file/open',
        expect.objectContaining({ method: 'POST' })
      )
    })
    expect(opened()).toHaveAttribute('data-editor-count', '2')

    fireEvent.doubleClick(screen.getByRole('button', { name: 'Notes' }))
    expect(screen.queryByRole('textbox', { name: '重命名' })).not.toBeInTheDocument()
    fireEvent.doubleClick(screen.getByRole('button', { name: '会话工作区' }))
    expect(screen.queryByRole('textbox', { name: '重命名' })).not.toBeInTheDocument()
  })

  it('drags files and folders into another folder', async () => {
    installRuntime()
    const directory = (name: string, path: string) => ({
      name,
      path,
      kind: 'directory' as const,
      sizeBytes: 0,
      modifiedMs: 1
    })
    const file = (name: string, path: string) => ({ name, path, kind: 'file' as const, sizeBytes: 2, modifiedMs: 1 })
    let root = [
      directory('Notes', '/tmp/session/Notes'),
      directory('Draft', '/tmp/session/Draft'),
      file('solo.txt', '/tmp/session/solo.txt'),
      file('root.txt', '/tmp/session/root.txt')
    ]
    let notes = [file('root.txt', '/tmp/session/Notes/root.txt')]
    const fetchMock = vi.fn(async (input: RequestInfo, init?: RequestInit) => {
      const url = String(input)
      if (init?.method === 'POST' && url.endsWith('/api/local-documents/move')) {
        const body = JSON.parse(String(init.body)) as { path: string; destination: string }
        if (body.path.endsWith('/root.txt') && body.destination.endsWith('/Notes')) {
          return { ok: false, status: 409, json: async () => ({}) }
        }
        const name = body.path.split('/').pop() ?? ''
        const fromRoot = root.findIndex((entry) => entry.path === body.path)
        const moved = fromRoot >= 0 ? root.splice(fromRoot, 1)[0] : null
        if (moved && body.destination.endsWith('/Notes')) {
          notes = [...notes, { ...moved, path: `${body.destination}/${name}` }]
        }
        return {
          ok: true,
          status: 200,
          json: async () => ({ ...moved, name, path: `${body.destination}/${name}` })
        }
      }
      const path = new URL(url, 'http://127.0.0.1').searchParams.get('path') ?? ''
      const entries = path.endsWith('/Notes') ? notes : !path || path === '/tmp/session' ? root : []
      return { ok: true, status: 200, json: async () => ({ entries, capped: false }) }
    })
    vi.stubGlobal('fetch', fetchMock)
    render(<WorkbenchSessionFiles root="/tmp/session" />)
    const notesButton = await screen.findByRole('button', { name: 'Notes' })
    const previousPoint = document.elementFromPoint
    const pointAt = (target: HTMLElement) => {
      document.elementFromPoint = () => target
    }
    const pointer = (type: 'pointerdown' | 'pointermove' | 'pointerup', source: HTMLElement, x: number) => {
      const event = new MouseEvent(type, { bubbles: true, cancelable: true, button: 0, clientX: x, clientY: 12 })
      Object.defineProperty(event, 'pointerId', { value: 1 })
      act(() => {
        source.dispatchEvent(event)
      })
    }
    const dragOnto = (source: HTMLElement, target: HTMLElement) => {
      pointAt(target)
      pointer('pointerdown', source, 8)
      pointer('pointermove', source, 40)
      pointer('pointerup', source, 40)
      document.elementFromPoint = previousPoint
    }

    const rootFile = () =>
      [...document.querySelectorAll('[data-workspace-session-file="root.txt"]')].find(
        (node) => node.getAttribute('data-workspace-session-file-folder') === ''
      ) as HTMLButtonElement
    dragOnto(rootFile(), notesButton)
    expect(await screen.findByRole('alert')).toHaveTextContent('当前位置已存在同名文件或文件夹。')
    expect(rootFile()).toBeInTheDocument()
    expect(fetchMock.mock.calls.some((call) => String(call[0]).endsWith('/api/local-documents/move'))).toBe(true)

    const solo = () => document.querySelector('[data-workspace-session-file="solo.txt"]') as HTMLButtonElement
    dragOnto(solo(), notesButton)
    await waitFor(() => {
      expect(document.querySelector('[data-workspace-session-file="solo.txt"]')).toHaveAttribute(
        'data-workspace-session-file-folder',
        'Notes'
      )
    })

    dragOnto(screen.getByRole('button', { name: 'Draft' }), notesButton)
    await waitFor(() => {
      expect(notesButton.parentElement?.parentElement).toContainElement(screen.getByRole('button', { name: 'Draft' }))
    })

    dragOnto(notesButton, notesButton)
    expect(await screen.findByRole('alert')).toHaveTextContent('不能把文件夹移动到自身或其子文件夹。')
  })

  it('uses the documents page file icons in the tree', async () => {
    installRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo) => {
        const path = new URL(String(input), 'http://127.0.0.1').searchParams.get('path') ?? ''
        if (path.endsWith('/Notes')) return { ok: true, json: async () => noteEntries }
        return {
          ok: true,
          json: async () => ({
            entries: [
              ...rootEntries.entries,
              { name: '001_init.sql', path: '/tmp/session/001_init.sql', kind: 'file', sizeBytes: 3, modifiedMs: 4 },
              { name: 'Cargo.toml', path: '/tmp/session/Cargo.toml', kind: 'file', sizeBytes: 3, modifiedMs: 5 }
            ],
            capped: false
          })
        }
      })
    )
    render(<WorkbenchSessionFiles root="/tmp/session" />)
    const sql = await waitFor(() => {
      const node = document.querySelector('[data-workspace-session-file="001_init.sql"] text')
      expect(node).not.toBeNull()
      return node as SVGTextElement
    })
    const config = document.querySelector('[data-workspace-session-file="Cargo.toml"] text')
    const notes = screen.getByRole('button', { name: 'Notes' })
    expect(sql).toHaveTextContent('SQL')
    expect(config).toHaveTextContent('CFG')
    expect(notes.querySelector('svg')).not.toBeNull()
    expect(notes.querySelector('.lucide-folder')).toBeNull()
  })

  it('uses the message image preview for an image file icon', async () => {
    installRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo) => {
        const url = String(input)
        if (url.includes('/api/workspace-file/raw')) {
          const bytes = Uint8Array.from([137, 80, 78, 71])
          return {
            ok: true,
            headers: { get: () => String(bytes.byteLength) },
            arrayBuffer: async () => bytes.buffer,
            json: async () => ({})
          }
        }
        return {
          ok: true,
          json: async () => ({
            entries: [{ name: 'cover.png', path: '/tmp/session/cover.png', kind: 'file', sizeBytes: 4, modifiedMs: 2 }],
            capped: false
          })
        }
      })
    )
    render(<WorkbenchSessionFiles root="/tmp/session" />)
    const thumbnail = await waitFor(() => {
      const image = document.querySelector('[data-workspace-session-file="cover.png"] img')
      expect(image).not.toBeNull()
      return image as HTMLImageElement
    })
    expect(thumbnail.getAttribute('src')).toMatch(/^blob:/)
    expect(thumbnail).toHaveAttribute('alt', '')
    fireEvent.click(thumbnail)
    expect(await screen.findByTestId('image-preview-dialog')).toBeInTheDocument()
    expect(document.querySelector('[data-workspace-file-editor]')).toBeNull()
    const row = thumbnail.closest('button')
    expect(row).not.toBeNull()
    fireEvent.doubleClick(row as HTMLElement)
    expect(document.querySelector('[data-workspace-file-editor]')).toBeNull()
    expect(screen.getAllByTestId('image-preview-dialog')).toHaveLength(1)
  })

  it('renames the listed file from the keyboard', async () => {
    installRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo) => ({
        ok: true,
        json: async () => listing(String(input))
      }))
    )

    render(<WorkbenchSessionFiles layout="split" root="/tmp/session" rootLabel="团队文档" />)
    const file = await waitFor(() => {
      const row = document.querySelector('[data-workspace-session-file="root.txt"]')
      expect(row).not.toBeNull()
      return row as HTMLButtonElement
    })

    fireEvent.keyDown(file, { key: 'F2' })
    const rename = await screen.findByRole('textbox', { name: '重命名' })
    expect(rename).toHaveValue('root.txt')
    fireEvent.keyDown(rename, { key: 'Escape' })
    await waitFor(() => {
      expect(screen.queryByRole('textbox', { name: '重命名' })).not.toBeInTheDocument()
    })
  })

  it('asks before deleting the listed file from the keyboard', async () => {
    installRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo) => ({
        ok: true,
        json: async () => listing(String(input))
      }))
    )

    render(<WorkbenchSessionFiles layout="split" root="/tmp/session" rootLabel="团队文档" />)
    const file = await waitFor(() => {
      const row = document.querySelector('table [data-workspace-session-file="root.txt"]')
      expect(row).not.toBeNull()
      return row as HTMLButtonElement
    })
    fireEvent.keyDown(file, { key: 'Delete' })
    const dialog = await screen.findByRole('dialog', { name: '删除这个文件或文件夹？' })
    expect(dialog).toHaveTextContent('root.txt 会移到废纸篓。')
  })

  it('puts an icon on every file-menu action', async () => {
    installRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo) => ({
        ok: true,
        json: async () => listing(String(input))
      }))
    )

    render(<WorkbenchSessionFiles root="/tmp/session" />)
    await waitFor(() => {
      expect(document.querySelector('[data-workspace-session-file="root.txt"]')).not.toBeNull()
    })
    fireEvent.contextMenu(document.querySelector('[data-workspace-session-file="root.txt"]') as HTMLButtonElement)

    for (const name of ['打开', '新建文件', '新建文件夹', '重命名', '删除']) {
      const item = await screen.findByRole('menuitem', { name })
      expect(item.querySelector('svg')).not.toBeNull()
    }
  })

  it('reloads a directory when the disk watcher reports a change', async () => {
    installRuntime()
    let notes = noteEntries.entries
    let releaseChange: () => void = () => {}
    const changeReady = new Promise<void>((resolve) => {
      releaseChange = resolve
    })
    let announced = false
    vi.stubGlobal(
      'fetch',
      vi.fn(async (input: RequestInfo, init?: RequestInit) => {
        const url = String(input)
        if (url.includes('/api/local-documents/changes')) {
          if (!announced) {
            await changeReady
            announced = true
            notes = [
              ...noteEntries.entries,
              { name: 'fresh.txt', path: '/tmp/session/Notes/fresh.txt', kind: 'file', sizeBytes: 4, modifiedMs: 9 }
            ]
            const frame =
              'event: workspace.files.changed\nid: 1\ndata: {"revision":1,"paths":["Notes"],"fullScan":false}\n\n'
            return {
              ok: true,
              headers: { get: (name: string) => (name.toLowerCase() === 'content-type' ? 'text/event-stream' : null) },
              body: eventStream(frame, init?.signal)
            }
          }
          return new Promise((_resolve, reject) => {
            init?.signal?.addEventListener('abort', () => reject(new DOMException('aborted', 'AbortError')))
          })
        }
        const path = new URL(url, 'http://127.0.0.1').searchParams.get('path') ?? ''
        if (path.endsWith('/Notes')) return { ok: true, json: async () => ({ entries: notes, capped: false }) }
        return { ok: true, json: async () => rootEntries }
      })
    )

    render(<WorkbenchSessionFiles root="/tmp/session" />)
    await waitFor(() => {
      expect(document.querySelector('[data-workspace-session-file="note.txt"]')).not.toBeNull()
    })
    releaseChange()
    await waitFor(() => {
      expect(document.querySelector('[data-workspace-session-file="fresh.txt"]')).not.toBeNull()
    })
    await waitFor(() => {
      expect(document.querySelector('[data-workspace-session-recent="fresh.txt"]')).not.toBeNull()
    })
  })

  it('shows at most eight recent files, newest first', async () => {
    installRuntime()
    const entries = Array.from({ length: 10 }, (_, index) => ({
      name: `file-${index}.txt`,
      path: `/tmp/session/file-${index}.txt`,
      kind: 'file' as const,
      sizeBytes: 1,
      modifiedMs: index + 1
    }))
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({ ok: true, json: async () => ({ entries, capped: false }) }))
    )

    render(<WorkbenchSessionFiles root="/tmp/session" />)

    await waitFor(() => {
      expect(document.querySelectorAll('[data-workspace-session-recent]')).toHaveLength(8)
    })
    const names = [...document.querySelectorAll('[data-workspace-session-recent]')].map((node) =>
      node.getAttribute('data-workspace-session-recent')
    )
    expect(names).toEqual([
      'file-9.txt',
      'file-8.txt',
      'file-7.txt',
      'file-6.txt',
      'file-5.txt',
      'file-4.txt',
      'file-3.txt',
      'file-2.txt'
    ])
  })

  it('says the session folder is empty after a successful load', async () => {
    installRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => ({ ok: true, json: async () => ({ entries: [], capped: false }) }))
    )

    render(<WorkbenchSessionFiles root="/tmp/session" />)

    expect(await screen.findByText('这个文件夹还没有文件')).toBeInTheDocument()
  })

  it('says the session folder did not load when the listing fails', async () => {
    installRuntime()
    vi.stubGlobal(
      'fetch',
      vi.fn(async () => {
        throw new Error('offline')
      })
    )

    render(<WorkbenchSessionFiles root="/tmp/session" />)

    await waitFor(() => {
      expect(screen.getByText('会话文件夹没有加载出来')).toBeInTheDocument()
    })
  })

  it('says there is no session folder when the conversation has no directory', () => {
    render(<WorkbenchSessionFiles root="" />)

    expect(screen.getByText('没有会话文件夹')).toBeInTheDocument()
  })
})
