import { MockUsePreferenceUtils } from '@test-mocks/renderer/usePreference'
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { writeCenterId } from '@renderer/data/memberOrgs'
import * as workbenchComposerBridge from '@renderer/pages/agents/workbenchComposerBridge'

import { cloudWorkspaceRoot, resetCloudWorkspaceForTests } from '../cloudWorkspace'
import { DocumentPipelineStatus } from '../DocumentPipelineStatus'
import DocumentsPage from '../DocumentsPage'

vi.mock('@renderer/pages/documents/blankOfficeFile', async () => {
  const actual = await vi.importActual<typeof import('../blankOfficeFile')>('../blankOfficeFile')
  return {
    ...actual,
    blankOfficeFileBytes: vi.fn(async () => new Uint8Array([80, 75, 3, 4]))
  }
})

vi.mock('@renderer/pages/agents/WorkbenchFileEditor', () => ({
  WorkbenchFileEditor: () => null
}))

beforeEach(() => {
  Element.prototype.scrollTo = () => undefined
  MockUsePreferenceUtils.resetMocks()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

afterEach(() => {
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
  delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  MockUsePreferenceUtils.resetMocks()
})

function treeNav() {
  return screen.getByRole('navigation', { name: '会话文件夹' })
}

describe('DocumentsPage sidebar', () => {
  it('uses the workspace file tree and omits the cloud and local switch', async () => {
    render(<DocumentsPage />)

    const nav = treeNav()
    expect(nav).toHaveStyle({ width: '260px' })
    expect(nav).toHaveClass('border-r', 'border-[#efefef]')
    const search = within(nav).getByRole('searchbox', { name: '搜索文件' })
    expect(search).toHaveAttribute('placeholder', '文件、文件夹…')
    expect(search.closest('label')).toHaveClass('h-[30px]', 'bg-[#f5f5f5]')
    expect(within(nav).getByRole('button', { name: '刷新' })).toBeInTheDocument()
    expect(within(nav).queryByRole('button', { name: '最近' })).not.toBeInTheDocument()
    expect(within(nav).getByRole('button', { name: '我的文档' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '后退' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '前进' })).toBeDisabled()
    expect(screen.getByRole('button', { name: '上一级' })).toBeDisabled()
    expect(
      within(screen.getByRole('navigation', { name: '当前位置' })).getByRole('button', { name: '我的文档' })
    ).toHaveAttribute('aria-current', 'page')
    expect(document.querySelector('[data-ui="documents.origin-tabs"]')).not.toBeInTheDocument()
    expect(screen.queryByRole('radiogroup', { name: '文档位置' })).not.toBeInTheDocument()
    expect(screen.queryByRole('radio', { name: '云端' })).not.toBeInTheDocument()
    expect(screen.queryByRole('radio', { name: '本地' })).not.toBeInTheDocument()
    expect(within(nav).queryByRole('button', { name: '收藏' })).not.toBeInTheDocument()
    expect(within(nav).queryByRole('button', { name: '云盘' })).not.toBeInTheDocument()
    expect(within(nav).queryByRole('button', { name: '回收站' })).not.toBeInTheDocument()
    expect(within(nav).queryByRole('button', { name: '存储空间' })).not.toBeInTheDocument()
    expect(screen.getByRole('heading', { name: '文档' })).toHaveClass('text-[24px]', 'font-bold')
    expect(screen.getByText('管理并编辑本地 OKF 源文件')).toBeInTheDocument()
  })

  it('lists the default local workspace when no custom workspace is stored', async () => {
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
    })
    const roots: string[] = []
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => {
        const text = String(url)
        if (!text.includes('/api/local-documents/entries')) return { ok: false, json: async () => ({}) }
        roots.push(new URL(text).searchParams.get('root') ?? '')
        return { ok: true, json: async () => ({ entries: [], capped: false }) }
      })
    )
    render(<DocumentsPage />)

    await waitFor(() => expect(roots).toContain('/Users/roylin/.a3s'))
    expect(screen.queryByRole('radio', { name: '云端' })).not.toBeInTheDocument()
  })

  it('lists workspace folders in the tree and their files in the list', async () => {
    const user = userEvent.setup()
    MockUsePreferenceUtils.setPreferenceValue('app.workspace.path', '/tmp/a3s-workspace')
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
    })
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => {
        const text = String(url)
        if (!text.includes('/api/local-documents/entries')) return { ok: false, json: async () => ({}) }
        const path = new URL(text).searchParams.get('path') ?? ''
        expect(new URL(text).searchParams.get('root')).toBe('/tmp/a3s-workspace')
        if (path.endsWith('/项目资料')) {
          return {
            ok: true,
            json: async () => ({
              entries: [
                {
                  name: '纪要.txt',
                  path: '/tmp/a3s-workspace/项目资料/纪要.txt',
                  kind: 'file',
                  sizeBytes: 4,
                  modifiedMs: 1
                }
              ],
              capped: false
            })
          }
        }
        return {
          ok: true,
          json: async () => ({
            entries: [
              {
                name: '项目资料',
                path: '/tmp/a3s-workspace/项目资料',
                kind: 'directory',
                sizeBytes: 0,
                modifiedMs: 2
              }
            ],
            capped: false
          })
        }
      })
    )
    render(<DocumentsPage />)

    const folder = await screen.findByRole('button', { name: '项目资料' })
    const treeScroll = document.querySelector('[data-workspace-session-tree-scroll]')
    expect(treeScroll).not.toBeNull()
    expect(treeScroll).toContainElement(folder)
    expect(treeScroll).not.toContainElement(within(treeNav()).getByRole('button', { name: '我的文档' }))
    expect(treeScroll).not.toContainElement(document.querySelector('[data-workspace-session-search]'))
    expect(folder).toHaveAttribute('aria-expanded', 'true')
    await user.click(folder)
    expect(folder).toHaveAttribute('aria-expanded', 'false')
    await user.click(folder)
    expect(folder).toHaveAttribute('aria-expanded', 'true')
    expect(await screen.findByRole('button', { name: '纪要.txt' })).toBeInTheDocument()
    expect(screen.getByRole('columnheader', { name: '文件名' })).toBeInTheDocument()
    expect(screen.queryByRole('columnheader', { name: '解析' })).not.toBeInTheDocument()
    expect(screen.getByRole('columnheader', { name: '更新时间' })).toBeInTheDocument()
    expect(screen.getByRole('columnheader', { name: '大小' })).toHaveClass('text-right')
    expect(screen.queryByRole('columnheader', { name: '所有者' })).not.toBeInTheDocument()
    const up = screen.getByRole('button', { name: '上一级' })
    expect(up).toBeEnabled()
    expect(screen.getByRole('button', { name: '后退' })).toBeEnabled()
    const location = screen.getByRole('navigation', { name: '当前位置' })
    expect(within(location).getByRole('button', { name: '项目资料' })).toHaveAttribute('aria-current', 'page')
    await user.click(up)
    expect(screen.queryByRole('button', { name: '纪要.txt' })).not.toBeInTheDocument()
    expect(screen.getByText('这个文件夹还没有文件')).toBeInTheDocument()
    expect(document.querySelector('[data-workspace-session-empty]')).not.toBeNull()
    expect(within(location).getByRole('button', { name: '我的文档' })).toHaveAttribute('aria-current', 'page')
    expect(screen.getByRole('button', { name: '新建文件' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '新建文件夹' })).toBeInTheDocument()
    expect(within(treeNav()).getByRole('button', { name: '刷新' })).toBeInTheDocument()
  })

  it('opens an editable file in an editor window and sends the rest to the system editor', async () => {
    const user = userEvent.setup()
    const invoke = vi.fn(async () => undefined)
    Object.defineProperty(globalThis, '__TAURI_INTERNALS__', { configurable: true, value: { invoke } })
    MockUsePreferenceUtils.setPreferenceValue('app.workspace.path', '/tmp/a3s-workspace')
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
    })
    const opened: string[] = []
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init?: RequestInit) => {
        const text = String(url)
        if (text.includes('/api/workspace-file/open') && init?.method === 'POST') {
          opened.push(String(init.body))
          return { ok: true, json: async () => ({}) }
        }
        if (text.includes('/api/workspace-file/raw') || text.includes('/api/workspace-file/snapshot')) {
          const bytes = Uint8Array.from([137, 80, 78, 71])
          return {
            ok: true,
            headers: {
              get: (name: string) => (name.toLowerCase() === 'etag' ? `"${'a'.repeat(64)}"` : String(bytes.byteLength))
            },
            arrayBuffer: async () => bytes.buffer,
            json: async () => ({})
          }
        }
        if (!text.includes('/api/local-documents/entries')) return { ok: false, json: async () => ({}) }
        return {
          ok: true,
          json: async () => ({
            entries: [
              { name: '纪要.txt', path: '/tmp/a3s-workspace/纪要.txt', kind: 'file', sizeBytes: 4, modifiedMs: 1 },
              { name: '封面.png', path: '/tmp/a3s-workspace/封面.png', kind: 'file', sizeBytes: 8, modifiedMs: 2 }
            ],
            capped: false
          })
        }
      })
    )
    render(<DocumentsPage />)

    await user.click(await screen.findByRole('button', { name: '纪要.txt' }))
    expect(invoke).toHaveBeenCalledWith('open_file_editor_window', {
      root: '/tmp/a3s-workspace',
      path: '/tmp/a3s-workspace/纪要.txt',
      title: '纪要.txt'
    })
    expect(screen.queryByRole('textbox', { name: '纪要.txt' })).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '封面.png' }))
    expect(await screen.findByTestId('image-preview-dialog')).toBeInTheDocument()
    expect(invoke).not.toHaveBeenCalledWith(
      'open_file_editor_window',
      expect.objectContaining({ path: '/tmp/a3s-workspace/封面.png' })
    )
    expect(opened).toEqual([])
  })

  it('uploads a file dropped onto the document list', async () => {
    MockUsePreferenceUtils.setPreferenceValue('app.workspace.path', '/tmp/a3s-workspace')
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
    })
    const bodies: string[] = []
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init?: RequestInit) => {
        const text = String(url)
        if (text.includes('/api/workspace-file/import') && init?.method === 'POST') {
          bodies.push(text)
          return { ok: true, json: async () => ({ ok: true, path: '/tmp/a3s-workspace/外来.txt' }) }
        }
        if (!text.includes('/api/local-documents/entries')) return { ok: false, json: async () => ({}) }
        const path = new URL(text).searchParams.get('path') ?? ''
        if (path.endsWith('/box')) return { ok: true, json: async () => ({ entries: [], capped: false }) }
        return {
          ok: true,
          json: async () => ({
            entries: [{ name: 'box', path: '/tmp/a3s-workspace/box', kind: 'directory', sizeBytes: 0, modifiedMs: 1 }],
            capped: false
          })
        }
      })
    )
    render(<DocumentsPage />)
    const list = await screen.findByRole('region', { name: '文件列表' })
    const file = new File(['hello'], '外来.txt', { type: 'text/plain' })
    const dataTransfer = { types: ['Files'], files: [file], items: [], dropEffect: 'none' }
    fireEvent.dragEnter(list, { dataTransfer })
    expect(screen.getByText('松开即可上传到这个文件夹')).toBeInTheDocument()
    fireEvent.drop(document.querySelector('[data-workspace-session-layout="split"]') as HTMLElement, { dataTransfer })
    await waitFor(() => {
      expect(
        bodies.some(
          (url) => url.includes('name=%E5%A4%96%E6%9D%A5.txt') && url.includes('directory=%2Ftmp%2Fa3s-workspace')
        )
      ).toBe(true)
    })
    expect(screen.getByText('已上传 1 个文件')).toBeInTheDocument()
  })

  it('uploads a file dropped onto the team drive', async () => {
    resetCloudWorkspaceForTests()
    writeCenterId('org-1')
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
    })
    const posts: string[] = []
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init?: RequestInit) => {
        const text = String(url)
        if (init?.method === 'POST' && text.endsWith('/api/drive/org-1/files')) {
          posts.push(text)
          return { ok: true, json: async () => ({ success: true, data: { id: 'file-1' } }) }
        }
        if (text.includes('/api/drive/org-1/nodes') || text.includes('/api/drive/org-1/files')) {
          return { ok: true, json: async () => ({ success: true, data: [] }) }
        }
        return { ok: true, json: async () => ({ ok: true }) }
      })
    )
    render(<DocumentsPage root={cloudWorkspaceRoot('org-1')} />)
    const list = await screen.findByRole('region', { name: '文件列表' })
    const file = new File(['hello'], '外来.txt', { type: 'text/plain' })
    const dataTransfer = { types: ['Files'], files: [file], items: [], dropEffect: 'none' }
    fireEvent.dragEnter(list, { dataTransfer })
    expect(screen.getByText('松开即可上传到这个文件夹')).toBeInTheDocument()
    fireEvent.drop(document.querySelector('[data-workspace-session-layout="split"]') as HTMLElement, { dataTransfer })
    await waitFor(() => expect(posts).toEqual(['http://127.0.0.1:9/api/drive/org-1/files']))
    expect(screen.getByText('已上传 1 个文件')).toBeInTheDocument()
    resetCloudWorkspaceForTests()
  })

  it('uploads a typed file and hands checked files to a task without a drop', async () => {
    resetCloudWorkspaceForTests()
    writeCenterId('org-1')
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
    })
    const posted: string[] = []
    const handoff = vi.spyOn(workbenchComposerBridge, 'queueWorkbenchComposer').mockImplementation(() => {})
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init?: RequestInit) => {
        const text = String(url)
        if (init?.method === 'POST' && text.endsWith('/api/drive/org-1/files')) {
          const form = init.body as FormData
          const file = form.get('file')
          posted.push(file instanceof File ? file.name : '')
          return { ok: true, json: async () => ({ success: true, data: { id: 'file-new' } }) }
        }
        if (text.includes('/api/drive/org-1/files')) {
          return {
            ok: true,
            json: async () => ({
              success: true,
              data: [
                {
                  id: 'file-a',
                  name: '甲.txt',
                  content_type: 'text/plain',
                  size_bytes: 1,
                  node_id: null,
                  dept_id: null,
                  created_at: '2026-10-09T01:00:00Z'
                },
                {
                  id: 'file-b',
                  name: '乙.txt',
                  content_type: 'text/plain',
                  size_bytes: 1,
                  node_id: null,
                  dept_id: null,
                  created_at: '2026-10-09T01:00:00Z'
                }
              ]
            })
          }
        }
        if (text.includes('/api/drive/org-1/nodes')) {
          return { ok: true, json: async () => ({ success: true, data: [] }) }
        }
        return { ok: true, json: async () => ({ ok: true }) }
      })
    )
    render(<DocumentsPage root={cloudWorkspaceRoot('org-1')} />)
    const user = userEvent.setup()
    expect(await screen.findByRole('checkbox', { name: '选择 甲.txt' })).toBeInTheDocument()
    expect(screen.getByRole('checkbox', { name: '选择 乙.txt' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '交给任务' })).toBeDisabled()

    await user.click(screen.getByRole('button', { name: '上传' }))
    fireEvent.change(screen.getByRole('textbox', { name: '上传文件名' }), { target: { value: '外来.txt' } })
    fireEvent.change(screen.getByRole('textbox', { name: '上传内容' }), { target: { value: 'hello' } })
    await user.click(screen.getByRole('button', { name: '开始上传' }))
    await waitFor(() => expect(posted).toEqual(['外来.txt']))
    expect(screen.getByText('已上传 1 个文件')).toBeInTheDocument()

    await user.click(screen.getByRole('checkbox', { name: '选择 甲.txt' }))
    await user.click(screen.getByRole('checkbox', { name: '选择 乙.txt' }))
    expect(screen.getByText('2 个已选')).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '交给任务' }))
    expect(handoff).toHaveBeenCalledWith('已交给任务：甲.txt、乙.txt')
    expect(screen.getByRole('status')).toHaveTextContent('已交给任务：甲.txt、乙.txt')
    handoff.mockRestore()
    resetCloudWorkspaceForTests()
  })

  it('omits new file from the team drive tree menu', async () => {
    resetCloudWorkspaceForTests()
    writeCenterId('org-1')
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
    })
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => {
        const text = String(url)
        if (text.includes('/api/drive/org-1/nodes') && !text.includes('parent_id=')) {
          return {
            ok: true,
            json: async () => ({
              success: true,
              data: [{ id: 'node-1', parent_id: null, dept_id: null, name: '测试', created_at: '2026-10-09T01:00:00Z' }]
            })
          }
        }
        if (text.includes('/api/drive/org-1/nodes') || text.includes('/api/drive/org-1/files')) {
          return { ok: true, json: async () => ({ success: true, data: [] }) }
        }
        return { ok: true, json: async () => ({ ok: true }) }
      })
    )
    render(<DocumentsPage root={cloudWorkspaceRoot('org-1')} />)
    const folder = await within(treeNav()).findByRole('button', { name: '测试' })
    fireEvent.contextMenu(folder)
    const menu = await screen.findByRole('menu', { name: '文件' })
    expect(within(menu).queryByRole('menuitem', { name: '新建文件' })).not.toBeInTheDocument()
    expect(within(menu).getByRole('menuitem', { name: '新建文件夹' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '新建文件' })).toBeInTheDocument()
    resetCloudWorkspaceForTests()
  })

  it('shows upload, parse, and compile progress for each team file', async () => {
    resetCloudWorkspaceForTests()
    writeCenterId('org-1')
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
    })
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string) => {
        const text = String(url)
        if (text.includes('/pipeline')) {
          return {
            ok: true,
            json: async () => ({
              success: true,
              data: {
                parse: [
                  { file_id: 'scan-1', file_name: '满江红.pdf', status: 'stored' },
                  { file_id: 'scan-2', file_name: '扫描.png', status: 'unparsed' }
                ],
                compile: [
                  { id: 'job-1', file_id: 'file-1', file_name: '指南.md', status: 'done' },
                  { id: 'job-2', file_id: 'file-9', file_name: '草稿.md', status: 'running' }
                ]
              }
            })
          }
        }
        if (text.includes('/encyclopedia')) {
          return {
            ok: true,
            json: async () => ({
              success: true,
              data: {
                file_id: 'file-1',
                file_name: '指南.md',
                article: {
                  id: 'article-1',
                  title: '满江红',
                  body: '怒发冲冠',
                  status: 'published',
                  sources: [
                    {
                      file_id: 'file-1',
                      file_name: '满江红.pdf',
                      passage_id: 'passage-1',
                      heading: '满江红',
                      excerpt: '怒发冲冠'
                    }
                  ]
                }
              }
            })
          }
        }
        if (text.includes('/parse')) {
          return { ok: false, status: 500, json: async () => ({ success: false, message: 'parse failed' }) }
        }
        if (text.includes('/api/drive/org-1/files')) {
          return {
            ok: true,
            json: async () => ({
              success: true,
              data: [
                {
                  id: 'file-1',
                  name: '指南.md',
                  content_type: 'text/markdown',
                  size_bytes: 12,
                  node_id: null,
                  dept_id: null,
                  created_at: '2026-10-09T01:00:00Z',
                  agent_retrievals: 0,
                  parse_status: 'parsed',
                  compile_status: 'done'
                },
                {
                  id: 'file-2',
                  name: '扫描件.pdf',
                  content_type: 'application/pdf',
                  size_bytes: 12,
                  node_id: null,
                  dept_id: null,
                  created_at: '2026-10-09T01:01:00Z',
                  agent_retrievals: 0,
                  parse_status: 'stored',
                  compile_status: 'skipped'
                }
              ]
            })
          }
        }
        if (text.includes('/api/drive/org-1/nodes')) {
          return { ok: true, json: async () => ({ success: true, data: [] }) }
        }
        return { ok: true, json: async () => ({ ok: true }) }
      })
    )
    render(<DocumentsPage root={cloudWorkspaceRoot('org-1')} />)
    expect(await screen.findByRole('columnheader', { name: '处理' })).toBeInTheDocument()
    expect(screen.queryByRole('columnheader', { name: '状态' })).not.toBeInTheDocument()
    expect(screen.queryByRole('columnheader', { name: '解析' })).not.toBeInTheDocument()
    expect(await screen.findByRole('button', { name: '指南.md' })).toBeInTheDocument()
    const pipeline = document.querySelector('[data-compile-status="done"]')
    expect(pipeline).toHaveAttribute('data-upload-status', 'uploaded')
    expect(pipeline).toHaveAttribute('data-parse-status', 'parsed')
    expect(pipeline).toHaveAccessibleName('已上传 · 已解析 · 已编译')
    expect(pipeline).toHaveTextContent('已就绪')
    expect(screen.getAllByRole('button', { name: '解析' })).toHaveLength(1)
    expect(screen.getAllByRole('button', { name: '百科' })).toHaveLength(1)
    expect(screen.getByText('待解析')).toBeInTheDocument()
    const queueButton = screen.getByRole('button', { name: '任务队列' })
    expect(queueButton).toHaveTextContent('3')
    await userEvent.setup().click(queueButton)
    const queue = await screen.findByRole('dialog', { name: '任务队列' })
    expect(queue).toHaveTextContent('满江红.pdf')
    expect(queue).toHaveTextContent('待解析')
    expect(queue).toHaveTextContent('扫描.png')
    expect(queue).toHaveTextContent('未能解析')
    expect(queue).toHaveTextContent('指南.md')
    expect(queue).toHaveTextContent('已编译')
    await userEvent.setup().click(within(queue).getByRole('button', { name: '百科' }))
    expect(await screen.findByRole('dialog', { name: '知识编译' })).toHaveTextContent('知识编译')
    expect(screen.getAllByText('怒发冲冠').length).toBeGreaterThan(0)
    expect(screen.getByText('溯源')).toBeInTheDocument()
    expect(screen.getByText('满江红.pdf · 满江红')).toBeInTheDocument()
    await userEvent.setup().click(screen.getAllByRole('button', { name: '解析' })[0])
    expect(await screen.findByText('解析没有完成。')).toBeInTheDocument()
    expect(screen.queryByText('documents.pipeline.parse_failed')).not.toBeInTheDocument()
    resetCloudWorkspaceForTests()
  })

  it('shows the current pipeline stage for a file that is not parsed yet', () => {
    const view = render(<DocumentPipelineStatus parseStatus="stored" compileStatus="skipped" />)
    expect(screen.getByRole('group')).toHaveTextContent('待解析')
    expect(screen.getByRole('group')).toHaveAccessibleName('已上传 · 未解析 · 未编译')
    view.rerender(<DocumentPipelineStatus parseStatus="stored" compileStatus="skipped" parsing />)
    expect(screen.getByRole('group')).toHaveTextContent('正在解析')
    expect(screen.getByRole('group')).toHaveAttribute('data-parse-status', 'parsing')
    view.rerender(<DocumentPipelineStatus parseStatus="parsed" compileStatus="failed" />)
    expect(screen.getByRole('group')).toHaveTextContent('编译失败')
    view.rerender(<DocumentPipelineStatus parseStatus="parsed" compileStatus="done" />)
    expect(screen.getByRole('group')).toHaveTextContent('已就绪')
    expect(screen.getByRole('group')).toHaveAccessibleName('已上传 · 已解析 · 已编译')
    view.rerender(<DocumentPipelineStatus parseStatus="unparsed" compileStatus="skipped" />)
    expect(screen.getByRole('group')).toHaveTextContent('未能解析')
    expect(screen.getByRole('group')).toHaveAttribute('data-parse-status', 'unparsed')
  })

  it('opens an office create menu and writes a blank document', async () => {
    const user = userEvent.setup()
    MockUsePreferenceUtils.setPreferenceValue('app.workspace.path', '/tmp/a3s-workspace')
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
    })
    const posts: { url: string; body: string }[] = []
    const puts: { url: string; size: number }[] = []
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init?: RequestInit) => {
        const text = String(url)
        if (init?.method === 'POST' && text.includes('/api/local-documents/entries')) {
          posts.push({ url: text, body: String(init.body) })
          const name = (JSON.parse(String(init.body)) as { name: string }).name
          return {
            ok: true,
            json: async () => ({
              name,
              path: `/tmp/a3s-workspace/${name}`,
              kind: 'file',
              sizeBytes: 4,
              modifiedMs: 1
            })
          }
        }
        if (init?.method === 'PUT' && text.includes('/api/workspace-file/raw')) {
          const body = init.body as ArrayBuffer | Uint8Array
          puts.push({ url: text, size: body instanceof Uint8Array ? body.byteLength : 0 })
          return { ok: true, json: async () => ({}) }
        }
        if (!text.includes('/api/local-documents/entries')) return { ok: false, json: async () => ({}) }
        return { ok: true, json: async () => ({ entries: [], capped: false }) }
      })
    )
    render(<DocumentsPage />)

    await user.click(await screen.findByRole('button', { name: '新建文件' }))
    const menu = await screen.findByRole('dialog', { name: '新建文档' })
    for (const name of ['文档', '表格', '幻灯片', 'PDF', 'Markdown']) {
      expect(within(menu).getByRole('button', { name })).toBeInTheDocument()
    }
    expect(within(menu).queryByRole('button', { name: '收集表' })).not.toBeInTheDocument()
    expect(within(menu).queryByRole('button', { name: '思维导图' })).not.toBeInTheDocument()

    await user.click(within(menu).getByRole('button', { name: '文档' }))
    expect(posts).toHaveLength(0)
    const name = await within(menu).findByRole('textbox', { name: '文件名' })
    expect(name).toHaveValue('未命名文档')
    expect(within(menu).getByText('.docx')).toBeInTheDocument()

    await user.keyboard('{Escape}')
    expect(posts).toHaveLength(0)
    expect(within(menu).getByRole('button', { name: '表格' })).toBeInTheDocument()

    await user.click(within(menu).getByRole('button', { name: '文档' }))
    const named = await within(menu).findByRole('textbox', { name: '文件名' })
    await user.clear(named)
    await user.type(named, '季度/计划')
    await user.click(within(menu).getByRole('button', { name: '创建' }))
    expect(posts).toHaveLength(0)
    expect(within(menu).getByRole('alert')).toHaveTextContent('名称不能包含')

    await user.clear(named)
    await user.type(named, '季度计划.docx')
    await user.click(within(menu).getByRole('button', { name: '创建' }))
    await waitFor(() => expect(posts).toHaveLength(1))
    expect(JSON.parse(posts[0].body)).toMatchObject({
      root: '/tmp/a3s-workspace',
      parent: '/tmp/a3s-workspace',
      name: '季度计划.docx',
      kind: 'file'
    })
    expect(puts).toEqual([
      expect.objectContaining({
        size: 4,
        url: expect.stringContaining(encodeURIComponent('/tmp/a3s-workspace/季度计划.docx'))
      })
    ])
    expect(screen.queryByRole('dialog', { name: '新建文档' })).not.toBeInTheDocument()
  })

  it('creates a document from the file name currently shown when the field did not emit a change', async () => {
    const user = userEvent.setup()
    MockUsePreferenceUtils.setPreferenceValue('app.workspace.path', '/tmp/a3s-workspace')
    Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
      configurable: true,
      value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
    })
    const posts: { url: string; body: string }[] = []
    vi.stubGlobal(
      'fetch',
      vi.fn(async (url: string, init?: RequestInit) => {
        const text = String(url)
        if (init?.method === 'POST' && text.includes('/api/local-documents/entries')) {
          posts.push({ url: text, body: String(init.body) })
          const name = (JSON.parse(String(init.body)) as { name: string }).name
          return {
            ok: true,
            json: async () => ({
              name,
              path: `/tmp/a3s-workspace/${name}`,
              kind: 'file',
              sizeBytes: 4,
              modifiedMs: 1
            })
          }
        }
        if (init?.method === 'PUT' && text.includes('/api/workspace-file/raw')) {
          return { ok: true, json: async () => ({}) }
        }
        if (!text.includes('/api/local-documents/entries')) return { ok: false, json: async () => ({}) }
        return { ok: true, json: async () => ({ entries: [], capped: false }) }
      })
    )
    render(<DocumentsPage />)

    await user.click(await screen.findByRole('button', { name: '新建文件' }))
    const menu = await screen.findByRole('dialog', { name: '新建文档' })
    await user.click(within(menu).getByRole('button', { name: '文档' }))
    const shown = await within(menu).findByRole('textbox', { name: '文件名' })
    expect(shown).toHaveValue('未命名文档')
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set
    setter?.call(shown, '字段所示')
    fireEvent.submit(shown.closest('form')!)

    await waitFor(() => expect(posts).toHaveLength(1))
    expect(JSON.parse(posts[0].body).name).toBe('字段所示.docx')
  })
})
