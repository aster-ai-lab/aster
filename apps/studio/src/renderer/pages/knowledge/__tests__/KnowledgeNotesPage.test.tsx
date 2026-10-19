import { MockUsePreferenceUtils } from '@test-mocks/renderer/usePreference'
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { cacheService } from '@data/CacheService'
import { PERSONAL_CENTER, writeCenterId } from '@renderer/data/memberOrgs'
import { TEXT_READ_BYTES, VISUAL_TEXT_UNITS } from '@renderer/pages/documents/fileOpenBudget'
import { buildAgentSessionTopicId } from '@renderer/utils/agentSession'

import { writeHeldNoteDraft } from '../heldNoteDraft'
import { resetKnowledgeCompileStore } from '../knowledgeCompileRun'
import { KnowledgeNotesPage } from '../KnowledgeNotesPage'
import KnowledgePage from '../KnowledgePage'
import { NOTE_INDEX_LIMIT } from '../noteReading'
import type { KnowledgeSection } from '../routeSearch'

const ROOT = '/workspace/a3s-notes'
const JIA = '见 [[乙]] 和 [乙文](乙.md)\n[[团队纪要]]'
const YI = '乙的正文'

const compileAgent = vi.hoisted(() => ({
  start: vi.fn(async () => {
    throw new Error('no agent')
  }),
  settled: null as null | (() => void),
  dispatch: vi.fn(async () => true)
}))

vi.mock('../knowledgeCompileAgent', () => ({
  startKnowledgeCompileSession: (root: string) => compileAgent.start(root)
}))

vi.mock('../KnowledgeCompileTurn', () => ({
  KnowledgeCompileTurn: () => null,
  dispatchKnowledgeCompileTurn: (sessionId: string, text: string) => compileAgent.dispatch(sessionId, text)
}))

vi.mock('../KnowledgeCompileAgentRun', async () => {
  const { createElement } = await import('react')
  return {
    KnowledgeCompileAgentRun: (props: { sessionId: string; onSettled: () => void }) => {
      compileAgent.settled = props.onSettled
      return createElement('div', {
        role: 'log',
        'aria-label': '编译记录',
        'data-ui': 'knowledge.compile.agent',
        'data-session': props.sessionId
      })
    }
  }
})

const office = vi.hoisted(() => ({
  markdowns: [] as string[],
  edited: '甲未保存的修改，仍见 [[乙]]',
  saved: '甲已保存，仍见 [[乙]]',
  extensions: [] as ReadonlyArray<{
    name?: string
    options?: {
      text?: string
      onOpen?: (path: string) => void
      notes?: ReadonlyArray<{ name: string; path: string }>
    }
  }>
}))

vi.mock('@a3s-lab/office/react', async () => {
  const { createElement, useEffect } = await import('react')

  return {
    MarkdownEditor: (props: {
      content: { markdown: string }
      preview?: boolean
      extensions?: ReadonlyArray<{
        name?: string
        options?: {
          text?: string
          onOpen?: (path: string) => void
          notes?: ReadonlyArray<{ name: string; path: string }>
        }
      }>
      onChange?: (next: { type: 'markdown'; markdown: string }) => void
      onReady?: () => void
      onSave?: (markdown: string) => void | Promise<void>
      fileActions?: ReadonlyArray<{ id: string; label: string; onSelect: () => void | Promise<void> }>
    }) => {
      useEffect(() => {
        props.onReady?.()
      }, [props.onReady])
      office.markdowns.push(props.content.markdown)
      office.extensions = props.extensions ?? []
      const reveal = props.extensions?.find((item) => item.name === 'knowledgeReveal')?.options?.text ?? ''
      return createElement(
        'div',
        {
          'data-editor': 'markdown',
          'data-markdown': props.content.markdown,
          'data-extensions': (props.extensions ?? []).map((item) => item.name).join(' '),
          'data-reveal': reveal,
          'data-preview': props.preview ? 'true' : 'false'
        },
        createElement(
          'button',
          {
            type: 'button',
            onClick: () => props.onChange?.({ type: 'markdown', markdown: office.edited })
          },
          '写入编辑'
        ),
        props.onSave
          ? createElement(
              'button',
              {
                type: 'button',
                onClick: () => {
                  props.onChange?.({ type: 'markdown', markdown: office.saved })
                  return props.onSave?.(office.saved)
                }
              },
              '保存'
            )
          : null,
        props.fileActions?.length ? createElement('button', { type: 'button' }, '文件') : null
      )
    }
  }
})

type FixtureFile = {
  name: string
  bytes?: Uint8Array
  missing?: boolean
  kind?: 'file' | 'directory'
  modifiedMs?: number
}
type VNode = { kind: 'file' | 'directory'; bytes?: Uint8Array; modifiedMs?: number }
type WriteRecord = { path: string; body: string }

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

function Page() {
  const [route, setRoute] = useState<{ baseId?: string; section?: KnowledgeSection }>({
    baseId: 'base-1',
    section: 'wiki'
  })
  return <KnowledgePage baseId={route.baseId} section={route.section} onChange={setRoute} />
}

function renderNotes(
  files: FixtureFile[],
  options?: {
    pauseFirstCloudPut?: Promise<void>
    holdLocalVaultList?: Promise<void>
    notesPage?: boolean
    vaultDirectory?: string
  }
) {
  const nodes = new Map<string, VNode>()
  const writes: WriteRecord[] = []
  const ensureDir = (path: string) => {
    if (!nodes.has(path)) nodes.set(path, { kind: 'directory' })
  }
  ensureDir(ROOT)
  ensureDir(`${ROOT}/wiki`)
  ensureDir(`${ROOT}/wiki/local`)
  nodes.set(`${ROOT}/根.md`, { kind: 'file', bytes: encode('# 根') })
  for (const file of files) {
    const path = `${ROOT}/wiki/local/${file.name}`
    const segments = file.name.split('/').filter(Boolean)
    let cursor = `${ROOT}/wiki/local`
    for (const segment of segments.slice(0, -1)) {
      cursor += `/${segment}`
      ensureDir(cursor)
    }
    if (file.kind === 'directory') {
      ensureDir(path)
      continue
    }
    nodes.set(
      path,
      file.missing
        ? { kind: 'file', modifiedMs: file.modifiedMs ?? 1 }
        : {
            kind: 'file',
            bytes: file.bytes ? Uint8Array.from(file.bytes) : new Uint8Array(),
            modifiedMs: file.modifiedMs ?? 1
          }
    )
  }

  let localVaultLists = 0
  const childrenOf = (dir: string) => {
    const prefix = `${dir}/`
    const entries = []
    for (const [path, node] of nodes) {
      if (!path.startsWith(prefix)) continue
      const name = path.slice(prefix.length)
      if (!name || name.includes('/')) continue
      entries.push({
        name,
        path,
        kind: node.kind,
        sizeBytes: node.bytes?.byteLength ?? 0,
        modifiedMs: node.modifiedMs ?? 1
      })
    }
    return entries
  }

  MockUsePreferenceUtils.setPreferenceValue('app.workspace.path', ROOT)
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    configurable: true,
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32), homePath: '/Users/roylin' }
  })
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init?: RequestInit) => {
      const text = String(url)
      const method = (init?.method ?? 'GET').toUpperCase()
      if (text.includes('/encyclopedia')) {
        return {
          ok: true,
          status: 200,
          json: async () => ({ success: true, data: [] }),
          arrayBuffer: async () => new ArrayBuffer(0)
        }
      }
      if (text.includes('/api/local-documents/move') && method === 'POST') {
        const body = JSON.parse(String(init?.body)) as { path: string; destination: string }
        const node = nodes.get(body.path)
        if (!node)
          return { ok: false, status: 404, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
        const name = body.path.slice(body.path.lastIndexOf('/') + 1)
        const dest = `${body.destination}/${name}`
        if (dest !== body.path && nodes.has(dest)) {
          return { ok: false, status: 409, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
        }
        const prefix = `${body.path}/`
        const moved: Array<[string, VNode]> = []
        for (const [path, item] of nodes) {
          if (path === body.path || path.startsWith(prefix)) moved.push([path, item])
        }
        for (const [path] of moved) nodes.delete(path)
        for (const [path, item] of moved) nodes.set(`${dest}${path.slice(body.path.length)}`, item)
        return {
          ok: true,
          status: 200,
          json: async () => ({
            name,
            path: dest,
            kind: node.kind,
            sizeBytes: node.bytes?.byteLength ?? 0,
            modifiedMs: 1
          }),
          arrayBuffer: async () => new ArrayBuffer(0)
        }
      }
      if (text.includes('/api/local-documents/rename') && method === 'POST') {
        const body = JSON.parse(String(init?.body)) as { path: string; name: string }
        const node = nodes.get(body.path)
        if (!node)
          return { ok: false, status: 404, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
        const parent = body.path.slice(0, body.path.lastIndexOf('/'))
        const dest = `${parent}/${body.name}`
        if (dest !== body.path && nodes.has(dest)) {
          return { ok: false, status: 409, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
        }
        const prefix = `${body.path}/`
        const moved: Array<[string, VNode]> = []
        for (const [path, item] of nodes) {
          if (path === body.path || path.startsWith(prefix)) moved.push([path, item])
        }
        for (const [path] of moved) nodes.delete(path)
        for (const [path, item] of moved) nodes.set(`${dest}${path.slice(body.path.length)}`, item)
        return {
          ok: true,
          status: 200,
          json: async () => ({
            name: body.name,
            path: dest,
            kind: node.kind,
            sizeBytes: node.bytes?.byteLength ?? 0,
            modifiedMs: 1
          }),
          arrayBuffer: async () => new ArrayBuffer(0)
        }
      }
      if (text.includes('/api/local-documents/remove') && method === 'POST') {
        const body = JSON.parse(String(init?.body)) as { path: string }
        if (!nodes.has(body.path)) {
          return { ok: false, status: 404, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
        }
        const prefix = `${body.path}/`
        for (const path of [...nodes.keys()]) {
          if (path === body.path || path.startsWith(prefix)) nodes.delete(path)
        }
        return { ok: true, status: 200, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
      }
      if (text.includes('/api/local-documents/entries') && method === 'POST') {
        const body = JSON.parse(String(init?.body)) as { parent: string; name: string; kind: string }
        const dest = `${body.parent}/${body.name}`
        if (nodes.has(dest))
          return { ok: false, status: 409, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
        const directory = body.kind === 'folder' || body.kind === 'directory'
        nodes.set(dest, directory ? { kind: 'directory' } : { kind: 'file', bytes: new Uint8Array() })
        return {
          ok: true,
          status: 200,
          json: async () => ({
            name: body.name,
            path: dest,
            kind: directory ? 'directory' : 'file',
            sizeBytes: 0,
            modifiedMs: 1
          }),
          arrayBuffer: async () => new ArrayBuffer(0)
        }
      }
      if (text.includes('/api/local-documents/entries')) {
        const dir = new URL(text).searchParams.get('path') ?? ''
        if (dir === `${ROOT}/wiki/local`) {
          localVaultLists += 1
          if (options?.holdLocalVaultList && localVaultLists > 1) await options.holdLocalVaultList
        }
        if (!nodes.has(dir) || nodes.get(dir)?.kind !== 'directory') {
          return { ok: false, status: 404, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
        }
        return {
          ok: true,
          status: 200,
          json: async () => ({ entries: childrenOf(dir), capped: false }),
          arrayBuffer: async () => new ArrayBuffer(0)
        }
      }
      if (text.includes('/api/workspace-file/raw')) {
        const path = new URL(text).searchParams.get('path') ?? ''
        if (method === 'GET' && path.endsWith('/过大.md')) {
          return {
            ok: true,
            status: 200,
            headers: {
              get: (name: string) => (name.toLowerCase() === 'content-length' ? String(TEXT_READ_BYTES + 8) : null)
            },
            body: { cancel: async () => undefined },
            json: async () => ({}),
            arrayBuffer: async () => {
              throw new Error('oversized body was read')
            }
          }
        }
        const node = nodes.get(path)
        if (method === 'PUT') {
          if (!node || node.kind !== 'file') {
            return { ok: false, status: 404, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
          }
          if (options?.pauseFirstCloudPut && path.endsWith('/wiki/cloud/product-handbook/index.md')) {
            await options.pauseFirstCloudPut
          }
          const bytes = Uint8Array.from(await requestBytes(init?.body))
          node.bytes = bytes
          writes.push({ path, body: new TextDecoder().decode(bytes) })
          return {
            ok: true,
            status: 200,
            json: async () => ({ ok: true }),
            arrayBuffer: async () => new ArrayBuffer(0)
          }
        }
        if (!node || node.kind !== 'file' || !node.bytes) {
          return { ok: false, status: 404, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
        }
        const copy = Uint8Array.from(node.bytes)
        return { ok: true, status: 200, json: async () => ({}), arrayBuffer: async () => copy.buffer }
      }
      return { ok: false, status: 404, json: async () => ({}), arrayBuffer: async () => new ArrayBuffer(0) }
    })
  )
  if (options?.notesPage || options?.vaultDirectory) {
    render(<KnowledgeNotesPage vaultDirectory={options.vaultDirectory} />)
    return { writes, nodes }
  }
  render(<Page />)
  const wiki = screen.queryByRole('radio', { name: '百科' })
  if (wiki) fireEvent.click(wiki)
  return { writes, nodes }
}

function notes(): FixtureFile[] {
  return [
    { name: '甲.md', bytes: encode(JIA) },
    { name: '乙.md', bytes: encode(YI) },
    { name: '丙.md', missing: true },
    { name: '乱码.md', bytes: Uint8Array.from([0xff, 0xfe]) },
    { name: 'readme.txt', bytes: encode('not a note') }
  ]
}

function listNav() {
  return screen.getByRole('navigation', { name: '笔记' })
}

function openRailGroup(name: string) {
  const rail = screen.getByRole('complementary', { name: '这篇笔记' })
  const header = within(rail).getByRole('button', { name })
  if (header.getAttribute('aria-expanded') !== 'true') fireEvent.click(header)
}

function compileCount(name: string) {
  return (_content: string, element: Element | null) => element?.getAttribute('aria-label') === name
}

function compileStatusKey(sessionId = 'compile-session') {
  return `topic.stream.statuses.${buildAgentSessionTopicId(sessionId)}` as 'topic.stream.statuses.${topicId}'
}

function compileShell(pending: string[], accept: () => unknown) {
  return (command: string) => {
    if (command === 'plan_personal_okf') return Promise.resolve({ pending })
    if (command === 'pin_personal_okf') return Promise.resolve(null)
    if (command === 'release_personal_okf') return Promise.resolve(null)
    if (command === 'remember_personal_okf') return Promise.resolve(null)
    if (command === 'accept_personal_okf') return Promise.resolve(accept())
    return Promise.reject(new Error(command))
  }
}

beforeEach(() => {
  resetKnowledgeCompileStore()
  cacheService.deleteShared(compileStatusKey())
  compileAgent.start.mockReset()
  compileAgent.start.mockRejectedValue(new Error('no agent'))
  compileAgent.settled = null
  compileAgent.dispatch.mockReset()
  compileAgent.dispatch.mockResolvedValue(true)
  writeHeldNoteDraft(null)
  office.markdowns.length = 0
  office.extensions = []
  office.edited = '甲未保存的修改，仍见 [[乙]]'
  localStorage.clear()
  document.cookie = 'a3s.org.center=; Path=/; Max-Age=0'
  MockUsePreferenceUtils.resetMocks()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

afterEach(() => {
  cleanup()
  resetKnowledgeCompileStore()
  cacheService.deleteShared(compileStatusKey())
  vi.unstubAllGlobals()
  localStorage.clear()
  document.cookie = 'a3s.org.center=; Path=/; Max-Age=0'
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
  delete (globalThis as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__
  MockUsePreferenceUtils.resetMocks()
})

describe('KnowledgePage notes', () => {
  it('reads personal notes from wiki/local and opens the cloud catalog without downloading', async () => {
    const user = userEvent.setup()
    renderNotes(notes())

    expect(document.querySelector('[data-ui="knowledge.view"]')).toHaveClass('bg-white')
    expect(screen.getByRole('heading', { name: '知识' })).toHaveClass('text-[24px]', 'font-bold')
    const search = screen.getByPlaceholderText('搜索笔记')
    const searchBox = search.closest('label')?.parentElement
    expect(searchBox).toHaveClass('h-[30px]', 'bg-[#f5f5f5]')
    expect(within(searchBox as HTMLElement).queryByRole('button', { name: '重新读取' })).not.toBeInTheDocument()
    const refresh = screen.getByRole('button', { name: '重新读取' })
    expect(searchBox).not.toContainElement(refresh)
    expect(search.compareDocumentPosition(refresh) & Node.DOCUMENT_POSITION_FOLLOWING).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    )
    expect(refresh.querySelector('svg')).not.toHaveClass('animate-spin')
    expect(screen.getByRole('heading', { name: '知识' }).parentElement).not.toContainElement(search)
    expect(screen.queryByRole('radiogroup', { name: '笔记位置' })).not.toBeInTheDocument()
    expect(screen.queryByRole('radio', { name: '本地' })).not.toBeInTheDocument()
    expect(screen.queryByRole('radio', { name: '云端' })).not.toBeInTheDocument()
    expect(screen.queryByRole('radio', { name: '文件名' })).not.toBeInTheDocument()
    expect(screen.queryByRole('radio', { name: '正文' })).not.toBeInTheDocument()
    const toolbar = screen.getByRole('group', { name: '知识操作' })
    expect(toolbar).toHaveClass('h-[30px]', 'rounded-[8px]', 'bg-[#f5f5f5]')
    const graph = within(toolbar).getByRole('button', { name: '图谱' })
    expect(graph.querySelector('svg')).toBeInTheDocument()
    expect(within(toolbar).queryByRole('button', { name: '知识编译' })).not.toBeInTheDocument()
    const compile = screen.getByRole('button', { name: '知识编译' })
    expect(compile).toHaveAttribute('data-variant', 'emphasis')
    expect(compile).toHaveClass('h-[30px]', 'rounded-[8px]')
    expect(compile).not.toHaveAttribute('aria-pressed')
    expect(screen.queryByRole('button', { name: '1 篇跳过' })).not.toBeInTheDocument()

    const nav = listNav()
    expect(nav.parentElement).toContainElement(search)
    expect(search.compareDocumentPosition(nav) & Node.DOCUMENT_POSITION_FOLLOWING).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    )
    const root = within(nav).getByRole('button', { name: '笔记' })
    expect(root).not.toHaveAttribute('aria-expanded')
    expect(root.querySelectorAll('svg')).toHaveLength(1)
    const rootRow = root.parentElement
    expect(rootRow).toHaveClass('group')
    const createFile = within(rootRow as HTMLElement).getByRole('button', { name: '新建文件' })
    const createFolder = within(rootRow as HTMLElement).getByRole('button', { name: '新建文件夹' })
    expect(createFile.parentElement?.className ?? '').not.toContain('opacity-0')
    expect(createFile.parentElement?.className ?? '').not.toContain('pointer-events-none')
    expect(createFolder.parentElement).toBe(createFile.parentElement)
    const reload = within(rootRow as HTMLElement).getByRole('button', { name: '重新读取' })
    expect(reload.compareDocumentPosition(createFile) & Node.DOCUMENT_POSITION_FOLLOWING).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    )
    expect(screen.getByText('正在读取笔记')).toBeInTheDocument()
    expect(screen.queryByText('还没有打开笔记')).not.toBeInTheDocument()
    expect(within(nav).queryByRole('radio', { name: '本地' })).not.toBeInTheDocument()
    expect(within(nav).queryByRole('radio', { name: '云端' })).not.toBeInTheDocument()
    expect(await within(nav).findByRole('button', { name: '甲' })).toBeInTheDocument()
    expect(screen.queryByText('正在读取笔记')).not.toBeInTheDocument()
    fireEvent.click(root)
    expect(within(nav).getByRole('button', { name: '甲' })).toBeInTheDocument()
    expect(within(nav).getByRole('button', { name: '乙' })).toBeInTheDocument()
    expect(within(nav).getByRole('button', { name: '丙' })).toBeInTheDocument()
    expect(within(nav).queryByRole('button', { name: '根' })).not.toBeInTheDocument()
    expect(within(nav).queryByRole('button', { name: 'readme.txt' })).not.toBeInTheDocument()
    expect(within(nav).queryByRole('button', { name: 'readme' })).not.toBeInTheDocument()
    expect(screen.queryByText('产品手册')).not.toBeInTheDocument()

    fireEvent.change(search, { target: { value: '乙' } })
    expect(within(nav).queryByRole('button', { name: '甲' })).not.toBeInTheDocument()
    expect(within(nav).getByRole('button', { name: '乙' })).toBeInTheDocument()
    fireEvent.change(search, { target: { value: '' } })

    await user.click(within(nav).getByRole('button', { name: '甲' }))
    const selected = within(nav).getByRole('button', { name: '甲' })
    expect(selected).toHaveClass('h-9', 'text-[13px]', 'bg-[#f2f3f5]', 'text-[#171717]')
    expect(selected).toHaveAttribute('aria-current', 'page')
    expect(selected.className).not.toContain('a3s-tenant-brand')
    expect(screen.getByRole('heading', { level: 2, name: '甲' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '返回' })).not.toBeInTheDocument()
    expect(screen.queryByRole('navigation', { name: '链接' })).not.toBeInTheDocument()
    expect(screen.queryByRole('navigation', { name: '反向链接' })).not.toBeInTheDocument()

    act(() => writeCenterId('org-1'))
    expect(screen.getByText('编写本地工作区里的笔记')).toBeInTheDocument()
    expect(screen.queryByText('这些条目来自团队云盘里的文本文档。')).not.toBeInTheDocument()
    expect(screen.queryByRole('region', { name: '百科' })).not.toBeInTheDocument()
    expect(within(nav).getByRole('button', { name: '甲' })).toBeInTheDocument()
    expect(screen.queryByText('产品手册')).not.toBeInTheDocument()
  })

  it('keeps personal notes on wiki/local when a team center is selected', async () => {
    const user = userEvent.setup()
    writeCenterId('org-1')
    renderNotes(notes(), { notesPage: true })

    expect(await within(listNav()).findByRole('button', { name: '甲' })).toBeInTheDocument()
    expect(screen.queryByText('当前没有团队会话，这里不会列出本地笔记。')).not.toBeInTheDocument()
    expect(screen.queryByText('团队笔记暂时无法读取。')).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '云端' }))
    expect(screen.getByRole('heading', { name: '产品手册' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '甲' })).not.toBeInTheDocument()
    expect(screen.queryByText('团队笔记暂时无法读取。')).not.toBeInTheDocument()

    act(() => writeCenterId(PERSONAL_CENTER))
    expect(screen.queryByRole('radiogroup', { name: '笔记位置' })).not.toBeInTheDocument()
    expect(await within(listNav()).findByRole('button', { name: '甲' })).toBeInTheDocument()
    expect(screen.queryByRole('heading', { name: '产品手册' })).not.toBeInTheDocument()
  })

  it('downloads an OKF bundle into wiki/cloud only after install', async () => {
    const user = userEvent.setup()
    let releaseDownload: () => void = () => {}
    const pauseDownload = new Promise<void>((resolve) => {
      releaseDownload = resolve
    })
    const { writes } = renderNotes(notes(), { pauseFirstCloudPut: pauseDownload, notesPage: true })
    act(() => writeCenterId('org-1'))

    await user.click(screen.getByRole('button', { name: '云端' }))
    expect(screen.getByRole('heading', { name: '产品手册' })).toBeInTheDocument()
    expect(writes).toHaveLength(0)
    expect(screen.queryByRole('radio', { name: '市场' })).not.toBeInTheDocument()
    expect(screen.queryByRole('radio', { name: '已安装' })).not.toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: '下载 产品手册' }))
    const progress = await screen.findByRole('progressbar', { name: '正在下载 产品手册' })
    expect(progress).toHaveAttribute('aria-valuemin', '0')
    expect(progress).toHaveAttribute('aria-valuemax', '100')
    expect(Number(progress.getAttribute('aria-valuenow'))).toBeLessThan(100)
    expect(progress).toHaveTextContent('正在准备')
    releaseDownload()
    await waitFor(() => {
      expect(
        writes.some(
          (write) =>
            write.path === `${ROOT}/wiki/cloud/product-handbook/index.md` && write.body.includes('okf_version: "0.2"')
        )
      ).toBe(true)
    })
    expect(
      writes.some(
        (write) => write.path === `${ROOT}/wiki/cloud/product-handbook/概述.md` && write.body.includes('type: Guide')
      )
    ).toBe(true)
    expect(writes.every((write) => !write.path.includes('/wiki/local'))).toBe(true)
    expect(screen.queryByRole('button', { name: '下载 产品手册' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '预览 产品手册' }).parentElement).toHaveClass(
      'opacity-0',
      'group-hover:opacity-100'
    )

    fireEvent.click(screen.getByRole('button', { name: '预览 产品手册' }))
    await waitFor(() => {
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute(
        'data-markdown',
        expect.stringContaining('okf_version: "0.2"')
      )
    })
    expect(screen.queryByRole('button', { name: '甲' })).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '本地' }))
    await user.click(screen.getByRole('button', { name: '云端' }))
    const written = writes.length
    fireEvent.click(screen.getByRole('button', { name: '删除 产品手册' }))
    expect(await screen.findByRole('button', { name: '下载 产品手册' })).toBeInTheDocument()
    expect(writes).toHaveLength(written)

    await user.click(screen.getByRole('button', { name: '本地' }))
    expect(await within(listNav()).findByRole('button', { name: '甲' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '概述' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '根' })).not.toBeInTheDocument()
  })

  it('mounts the note text, saves it, and refuses an unreadable note', async () => {
    const invoke = vi.fn(async () => 'compiled=1 unchanged=0 skipped=0 failed=0')
    Object.defineProperty(globalThis, '__TAURI_INTERNALS__', {
      configurable: true,
      value: { invoke }
    })
    const user = userEvent.setup()
    const { writes } = renderNotes(notes())

    await user.click(await within(listNav()).findByRole('button', { name: '甲' }))
    await waitFor(() => expect(office.markdowns.at(-1)).toBe(JIA))
    expect(document.querySelector('[data-editor="markdown"]')).toBeInTheDocument()
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute(
      'data-extensions',
      expect.stringContaining('knowledgeWikiLink')
    )
    expect(screen.queryByRole('button', { name: '文件' })).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '保存' }))
    await waitFor(() => expect(writes.at(-1)?.body).toBe(office.saved))
    expect(invoke.mock.calls.map((call) => call[0])).not.toContain('compile_personal_okf')
    expect(writes.at(-1)?.path).toBe(`${ROOT}/wiki/local/甲.md`)
    await user.click(within(listNav()).getByRole('button', { name: '乙' }))
    await user.click(within(listNav()).getByRole('button', { name: '甲' }))
    await waitFor(() => expect(office.markdowns.at(-1)).toBe(office.saved))

    await user.click(within(listNav()).getByRole('button', { name: '丙' }))
    expect(await screen.findByRole('heading', { name: '笔记不可用' })).toBeInTheDocument()
    expect(screen.getByText('这篇笔记可能已被移动、删除或无法读取。')).toBeInTheDocument()
    expect(document.querySelector('[data-editor="markdown"]')).not.toBeInTheDocument()

    await user.click(within(listNav()).getByRole('button', { name: '乱码' }))
    expect(await screen.findByRole('heading', { name: '笔记不可用' })).toBeInTheDocument()
    expect(document.querySelector('[data-editor="markdown"]')).not.toBeInTheDocument()
  })

  it('keeps an unsaved edit when the search field changes', async () => {
    const user = userEvent.setup()
    renderNotes(notes())

    await user.click(await within(listNav()).findByRole('button', { name: '甲' }))
    await waitFor(() => expect(office.markdowns.at(-1)).toBe(JIA))
    await user.click(screen.getByRole('button', { name: '写入编辑' }))
    await waitFor(() => expect(office.markdowns.at(-1)).toBe(office.edited))

    const beforeSearch = office.markdowns.length
    fireEvent.change(screen.getByPlaceholderText('搜索笔记'), { target: { value: '乙' } })
    await waitFor(() => expect(office.markdowns.length).toBeGreaterThan(beforeSearch))
    expect(office.markdowns.slice(beforeSearch).every((item) => item === office.edited)).toBe(true)
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', office.edited)
    expect(within(listNav()).queryByRole('button', { name: '甲' })).not.toBeInTheDocument()
  })

  it('opens a note without a back button or a link footer', async () => {
    const user = userEvent.setup()
    renderNotes(notes())

    await user.click(await within(listNav()).findByRole('button', { name: '甲' }))
    await waitFor(() => expect(office.markdowns.at(-1)).toBe(JIA))
    expect(screen.queryByRole('button', { name: '返回' })).not.toBeInTheDocument()
    expect(screen.queryByRole('navigation', { name: '链接' })).not.toBeInTheDocument()
    expect(screen.queryByRole('navigation', { name: '反向链接' })).not.toBeInTheDocument()
    const heading = screen.getByRole('heading', { level: 2, name: '甲' })
    const column = heading.parentElement?.parentElement
    expect(heading.parentElement).toHaveClass('mb-2', 'h-[30px]')
    expect(column).toHaveClass('flex-1', 'w-full', 'pt-1')
    expect(column?.className ?? '').not.toMatch(/max-w-|pt-5/)
  })

  it('opens a file on click and does not rename it on double click', async () => {
    renderNotes([{ name: '甲.md', bytes: encode(JIA) }])
    const nav = listNav()
    const file = await within(nav).findByRole('button', { name: '甲' })
    fireEvent.click(file)
    expect(file).toHaveAttribute('aria-current', 'page')
    expect(await screen.findByRole('heading', { level: 2, name: '甲' })).toBeInTheDocument()
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', JIA)
    )
    expect(screen.queryByRole('textbox', { name: '名称' })).not.toBeInTheDocument()

    fireEvent.click(file)
    expect(screen.queryByRole('textbox', { name: '名称' })).not.toBeInTheDocument()
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', JIA)

    fireEvent.contextMenu(file)
    const fileMenu = screen.getByRole('menu', { name: '笔记' })
    for (const name of ['打开', '新建文件', '新建文件夹', '删除']) {
      expect(within(fileMenu).getByRole('menuitem', { name }).querySelector('svg')).toBeTruthy()
    }
    expect(within(fileMenu).getByRole('menuitem', { name: '重命名' })).toBeInTheDocument()
    fireEvent.pointerDown(document.body)

    fireEvent.doubleClick(file)
    expect(screen.queryByRole('textbox', { name: '名称' })).not.toBeInTheDocument()
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', JIA)

    fireEvent.click(within(nav).getByRole('button', { name: '甲' }), { detail: 2 })
    expect(screen.queryByRole('textbox', { name: '名称' })).not.toBeInTheDocument()
    expect(within(nav).getByRole('button', { name: '甲' })).toHaveAttribute('aria-current', 'page')

    fireEvent.contextMenu(within(nav).getByRole('button', { name: '笔记' }))
    fireEvent.click(within(screen.getByRole('menu', { name: '笔记' })).getByRole('menuitem', { name: '新建文件夹' }))
    fireEvent.submit(screen.getByRole('textbox', { name: '名称' }).closest('form') as HTMLFormElement)
    const folder = await within(nav).findByRole('button', { name: '未命名文件夹' })
    expect(folder).toHaveAttribute('aria-expanded', 'true')
    fireEvent.contextMenu(folder)
    expect(
      within(screen.getByRole('menu', { name: '笔记' })).getByRole('menuitem', { name: '重命名' })
    ).toBeInTheDocument()
    fireEvent.pointerDown(document.body)
    fireEvent.click(folder, { detail: 2 })
    fireEvent.doubleClick(folder)
    expect(screen.queryByRole('textbox', { name: '名称' })).not.toBeInTheDocument()
    expect(folder).toHaveAttribute('aria-expanded', 'true')
  })

  it('drags files and folders into another folder', async () => {
    renderNotes([
      { name: '甲.md', bytes: encode(JIA) },
      { name: '副本.md', bytes: encode('副本') },
      { name: '归档', kind: 'directory' },
      { name: '归档/副本.md', bytes: encode('已有') },
      { name: '草稿', kind: 'directory' },
      { name: '草稿/乙.md', bytes: encode(YI) }
    ])
    const nav = listNav()
    const rowOf = (name: string) => within(nav).getByRole('button', { name }).parentElement as HTMLElement
    const rootCopy = () =>
      within(nav)
        .getAllByRole('button', { name: '副本' })
        .find((button) => !rowOf('归档').contains(button)) as HTMLElement
    await within(nav).findByRole('button', { name: '甲' })
    const archive = within(nav).getByRole('button', { name: '归档' })
    expect(within(rowOf('归档')).queryByRole('button', { name: '甲' })).not.toBeInTheDocument()

    const previousPoint = document.elementFromPoint
    const pointAt = (target: HTMLElement) => {
      document.elementFromPoint = () => target
    }
    const pointer = (type: 'pointerdown' | 'pointermove' | 'pointerup', source: HTMLElement, x: number) => {
      const event = new MouseEvent(type, { bubbles: true, cancelable: true, button: 0, clientX: x, clientY: 10 })
      Object.defineProperty(event, 'pointerId', { value: 1 })
      act(() => {
        source.dispatchEvent(event)
      })
    }
    const dragOnto = (source: HTMLElement, target: HTMLElement) => {
      pointAt(target)
      pointer('pointerdown', source, 10)
      pointer('pointermove', source, 40)
      pointer('pointerup', source, 40)
      document.elementFromPoint = previousPoint
    }

    dragOnto(within(nav).getByRole('button', { name: '甲' }), within(nav).getByRole('button', { name: '笔记' }))
    expect(screen.queryByRole('alert')).not.toBeInTheDocument()
    expect(within(rowOf('归档')).queryByRole('button', { name: '甲' })).not.toBeInTheDocument()

    const file = within(nav).getByRole('button', { name: '甲' })
    pointAt(archive)
    pointer('pointerdown', file, 10)
    pointer('pointermove', file, 40)
    expect(archive).toHaveClass('ring-2')
    pointer('pointerup', file, 40)
    document.elementFromPoint = previousPoint
    await waitFor(() => {
      expect(within(rowOf('归档')).getByRole('button', { name: '甲' })).toBeInTheDocument()
    })

    fireEvent.click(within(rowOf('归档')).getByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', JIA)
    )

    dragOnto(within(nav).getByRole('button', { name: '草稿' }), within(nav).getByRole('button', { name: '归档' }))
    await waitFor(() => {
      expect(within(rowOf('归档')).getByRole('button', { name: '草稿' })).toBeInTheDocument()
    })
    expect(within(rowOf('草稿')).getByRole('button', { name: '乙' })).toBeInTheDocument()
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', JIA)

    dragOnto(
      within(nav).getByRole('button', { name: '归档' }),
      within(rowOf('归档')).getByRole('button', { name: '草稿' })
    )
    expect(await screen.findByRole('alert')).toHaveTextContent('不能把文件夹移动到自身或其子文件夹。')
    expect(within(rowOf('归档')).getByRole('button', { name: '草稿' })).toBeInTheDocument()

    dragOnto(rootCopy(), within(nav).getByRole('button', { name: '归档' }))
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('当前位置已有同名项。'))
    expect(rootCopy()).toBeInTheDocument()

    dragOnto(
      within(rowOf('归档')).getByRole('button', { name: '甲' }),
      within(nav).getByRole('button', { name: '笔记' })
    )
    await waitFor(() => {
      expect(within(rowOf('归档')).queryByRole('button', { name: '甲' })).not.toBeInTheDocument()
    })
    expect(within(nav).getByRole('button', { name: '甲' })).toHaveAttribute('aria-current', 'page')
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', JIA)
    )
  })

  it('creates and deletes markdown notes from the local vault tree', async () => {
    const user = userEvent.setup()
    const { writes } = renderNotes([])
    const nav = listNav()
    expect(await within(nav).findByText('还没有笔记')).toBeInTheDocument()
    expect(screen.getByText('还没有打开笔记')).toBeInTheDocument()
    expect(screen.queryByRole('navigation', { name: '继续写' })).not.toBeInTheDocument()
    const blank = screen.getByText('还没有打开笔记').parentElement as HTMLElement
    expect(within(blank).getByRole('button', { name: '新建文件' })).toBeInTheDocument()
    expect(within(blank).getByRole('button', { name: '新建文件夹' })).toBeInTheDocument()

    const root = within(nav).getByRole('button', { name: '笔记' })
    expect(root.parentElement?.parentElement).toHaveClass('sticky', 'top-0')
    expect(nav.contains(screen.getByRole('searchbox', { name: '搜索笔记' }))).toBe(false)
    fireEvent.click(within(root.parentElement as HTMLElement).getByRole('button', { name: '新建文件' }))
    const fileName = screen.getByRole('textbox', { name: '名称' })
    expect(fileName).toHaveValue('未命名.md')
    fireEvent.change(fileName, { target: { value: '新笔记.md' } })
    fireEvent.submit(fileName.closest('form') as HTMLFormElement)
    expect(await within(nav).findByRole('button', { name: '新笔记' })).toHaveAttribute('aria-current', 'page')
    await waitFor(() => {
      expect(writes.some((write) => write.path === `${ROOT}/wiki/local/新笔记.md` && write.body === '\n')).toBe(true)
    })
    await waitFor(() => {
      expect(document.querySelector('[data-editor="markdown"]')).toBeInTheDocument()
    })
    expect(screen.queryByText('还没有打开笔记')).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '返回' })).not.toBeInTheDocument()

    fireEvent.contextMenu(within(nav).getByRole('button', { name: '笔记' }))
    const rootMenu = screen.getByRole('menu', { name: '笔记' })
    expect(within(rootMenu).getByRole('menuitem', { name: '新建文件' })).toBeInTheDocument()
    expect(within(rootMenu).getByRole('menuitem', { name: '新建文件夹' })).toBeInTheDocument()
    expect(within(rootMenu).queryByRole('menuitem', { name: '重命名' })).not.toBeInTheDocument()
    expect(within(rootMenu).queryByRole('menuitem', { name: '删除' })).not.toBeInTheDocument()
    await user.click(within(rootMenu).getByRole('menuitem', { name: '新建文件夹' }))
    const folderName = screen.getByRole('textbox', { name: '名称' })
    expect(folderName).toHaveValue('未命名文件夹')
    fireEvent.submit(folderName.closest('form') as HTMLFormElement)
    expect(await within(nav).findByRole('button', { name: '未命名文件夹' })).toHaveAttribute('aria-expanded', 'true')

    fireEvent.contextMenu(within(nav).getByRole('button', { name: '新笔记' }))
    const fileMenu = screen.getByRole('menu', { name: '笔记' })
    expect(within(fileMenu).getByRole('menuitem', { name: '打开' })).toBeInTheDocument()
    expect(within(fileMenu).getByRole('menuitem', { name: '重命名' })).toBeInTheDocument()
    expect(within(fileMenu).getByRole('menuitem', { name: '删除' })).toBeInTheDocument()
    await user.click(within(fileMenu).getByRole('menuitem', { name: '删除' }))
    const dialog = screen.getByRole('dialog', { name: '删除这个文件或文件夹？' })
    expect(dialog).toHaveClass('ui-popconfirm')
    expect(within(dialog).getByText('新笔记.md 会移到废纸篓。')).toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: '删除' }))
    await waitFor(() => {
      expect(within(nav).queryByRole('button', { name: '新笔记' })).not.toBeInTheDocument()
    })
    expect(within(nav).getByRole('button', { name: '未命名文件夹' })).toBeInTheDocument()

    fireEvent.click(
      within(within(nav).getByRole('button', { name: '笔记' }).parentElement as HTMLElement).getByRole('button', {
        name: '新建文件'
      })
    )
    const rejected = screen.getByRole('textbox', { name: '名称' })
    expect(rejected).toHaveValue('未命名.md')
    fireEvent.change(rejected, { target: { value: '说明.txt' } })
    fireEvent.submit(rejected.closest('form') as HTMLFormElement)
    expect(await screen.findByRole('alert')).toHaveTextContent('只能创建 Markdown 文件。')
    expect(writes.some((write) => write.path.endsWith('说明.txt') || write.path.endsWith('说明.txt.md'))).toBe(false)
    expect(within(nav).queryByRole('button', { name: '说明.txt' })).not.toBeInTheDocument()

    fireEvent.change(screen.getByRole('textbox', { name: '名称' }), { target: { value: '会议纪要' } })
    fireEvent.submit(screen.getByRole('textbox', { name: '名称' }).closest('form') as HTMLFormElement)
    expect(await within(nav).findByRole('button', { name: '会议纪要' })).toBeInTheDocument()
    await waitFor(() => {
      expect(writes.some((write) => write.path === `${ROOT}/wiki/local/会议纪要.md` && write.body === '\n')).toBe(true)
    })
  })

  it('submits the note name currently shown when the field did not emit a change', async () => {
    const { writes } = renderNotes([])
    const nav = listNav()
    const root = await within(nav).findByRole('button', { name: '笔记' })
    fireEvent.click(within(root.parentElement as HTMLElement).getByRole('button', { name: '新建文件' }))
    const fileName = screen.getByRole('textbox', { name: '名称' })
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')?.set
    setter?.call(fileName, 'e2e-shown.md')
    fireEvent.submit(fileName.closest('form') as HTMLFormElement)
    expect(await within(nav).findByRole('button', { name: 'e2e-shown' })).toBeInTheDocument()
    await waitFor(() => {
      expect(writes.some((write) => write.path === `${ROOT}/wiki/local/e2e-shown.md`)).toBe(true)
    })

    fireEvent.keyDown(within(nav).getByRole('button', { name: 'e2e-shown' }), { key: 'ContextMenu' })
    expect(await screen.findByRole('menu', { name: '笔记' })).toBeInTheDocument()
    expect(screen.getByRole('menuitem', { name: '删除' })).toBeInTheDocument()
  })

  it('edits the open note title without renaming the file', async () => {
    const { writes } = renderNotes(notes())
    const user = userEvent.setup()
    await user.click(await within(listNav()).findByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', JIA)
    )
    await user.click(screen.getByRole('button', { name: '写入编辑' }))
    const heading = screen.getByRole('heading', { level: 2, name: '甲' })
    expect(heading.parentElement).toHaveClass('mb-2', 'h-[30px]')

    await user.click(heading)
    const title = screen.getByRole('textbox', { name: '笔记标题' })
    expect(title).toHaveValue('甲')
    expect(screen.getByText('1/80')).toBeInTheDocument()
    fireEvent.change(title, { target: { value: '   ' } })
    fireEvent.submit(title.closest('form') as HTMLFormElement)
    expect(await screen.findByRole('alert')).toHaveTextContent('标题不能为空。')
    expect(within(listNav()).getByRole('button', { name: '甲' })).toBeInTheDocument()

    const longTitle = '菜'.repeat(90)
    fireEvent.change(screen.getByRole('textbox', { name: '笔记标题' }), { target: { value: longTitle } })
    expect(screen.getByRole('textbox', { name: '笔记标题' })).toHaveValue('菜'.repeat(80))
    expect(screen.getByText('80/80')).toBeInTheDocument()
    fireEvent.change(screen.getByRole('textbox', { name: '笔记标题' }), { target: { value: '家常食谱' } })
    fireEvent.submit(screen.getByRole('textbox', { name: '笔记标题' }).closest('form') as HTMLFormElement)

    expect(await screen.findByRole('heading', { level: 2, name: '家常食谱' })).toBeInTheDocument()
    expect(within(listNav()).getByRole('button', { name: '家常食谱' })).toBeInTheDocument()
    expect(within(listNav()).queryByRole('button', { name: '甲' })).not.toBeInTheDocument()
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', office.edited)
    await waitFor(() => {
      expect(
        writes.some(
          (write) =>
            write.path === `${ROOT}/wiki/local/甲.md` &&
            write.body.includes('title: "家常食谱"') &&
            write.body.includes(office.edited)
        )
      ).toBe(true)
    })
    expect(writes.some((write) => write.path.endsWith('/家常食谱.md'))).toBe(false)

    fireEvent.contextMenu(within(listNav()).getByRole('button', { name: '家常食谱' }))
    expect(
      within(screen.getByRole('menu', { name: '笔记' })).getByRole('menuitem', { name: '重命名' })
    ).toBeInTheDocument()
  })

  it('renames an open markdown file and keeps the unsaved draft', async () => {
    renderNotes([{ name: '甲.md', bytes: encode(JIA) }])
    const user = userEvent.setup()
    await user.click(await within(listNav()).findByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', JIA)
    )
    await user.click(screen.getByRole('button', { name: '写入编辑' }))
    fireEvent.contextMenu(within(listNav()).getByRole('button', { name: '甲' }))
    await user.click(screen.getByRole('menuitem', { name: '重命名' }))
    const field = screen.getByRole('textbox', { name: '名称' })
    fireEvent.change(field, { target: { value: '乙.md' } })
    fireEvent.submit(field.closest('form') as HTMLFormElement)
    expect(await within(listNav()).findByRole('button', { name: '乙' })).toHaveAttribute('aria-current', 'page')
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', office.edited)
    expect(screen.getByRole('heading', { level: 2, name: '乙' })).toBeInTheDocument()
    expect(within(listNav()).queryByRole('button', { name: '甲' })).not.toBeInTheDocument()
  })

  it('shows the last save time at the top right of the editor after saving', async () => {
    renderNotes(notes())
    const user = userEvent.setup()
    await user.click(await within(listNav()).findByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', JIA)
    )
    expect(screen.queryByText(/最近修改/)).not.toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '保存' }))
    const saved = await screen.findByText(/^最近修改 \d{2}:\d{2}$/)
    const heading = screen.getByRole('heading', { level: 2, name: '甲' })
    expect(heading.parentElement).toContainElement(saved)
    expect(heading.parentElement).toHaveClass('mb-2', 'h-[30px]')

    await user.click(within(listNav()).getByRole('button', { name: '乙' }))
    await waitFor(() => expect(screen.queryByText(/最近修改/)).not.toBeInTheDocument())
    await user.click(within(listNav()).getByRole('button', { name: '甲' }))
    expect(await screen.findByText(/^最近修改 \d{2}:\d{2}$/)).toBeInTheDocument()
  })

  it('spins the sidebar refresh and reloads the latest wiki/local tree without replacing the open draft', async () => {
    let releaseList: () => void = () => {}
    const holdLocalVaultList = new Promise<void>((resolve) => {
      releaseList = resolve
    })
    const { nodes } = renderNotes(notes(), { holdLocalVaultList })
    const user = userEvent.setup()
    const nav = listNav()
    await user.click(await within(nav).findByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', JIA)
    )
    await user.click(screen.getByRole('button', { name: '写入编辑' }))
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', office.edited)

    nodes.set(`${ROOT}/wiki/local/丁.md`, { kind: 'file', bytes: encode('丁的正文') })
    const refresh = screen.getByRole('button', { name: '重新读取' })
    await user.click(refresh)
    await waitFor(() => expect(refresh).toHaveAttribute('aria-busy', 'true'))
    expect(refresh.querySelector('svg')).toHaveClass('animate-spin')
    expect(within(nav).queryByRole('button', { name: '丁' })).not.toBeInTheDocument()

    releaseList()
    expect(await within(nav).findByRole('button', { name: '丁' })).toBeInTheDocument()
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', office.edited)
    expect(refresh).not.toHaveAttribute('aria-busy')
    expect(refresh.querySelector('svg')).not.toHaveClass('animate-spin')
  })

  it('keeps the wiki extension while a dirty draft is open and the index reloads', async () => {
    const { nodes } = renderNotes([
      { name: '甲.md', bytes: encode('见 [[乙]]') },
      { name: '乙.md', bytes: encode('乙的正文') }
    ])
    const user = userEvent.setup()
    await user.click(await within(listNav()).findByRole('button', { name: '甲' }))
    const rail = await screen.findByRole('complementary', { name: '这篇笔记' })
    await waitFor(() => expect(within(rail).getByRole('button', { name: '链接' })).toBeInTheDocument())
    expect(within(rail).queryByText(/已载入/)).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '写入编辑' }))
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', office.edited)
    const extensions = office.extensions
    const wiki = extensions.find((item) => item.name === 'knowledgeWikiLink')
    expect(wiki).toBeDefined()

    nodes.set(`${ROOT}/wiki/local/乙.md`, { kind: 'file', bytes: encode('新的来处 [[甲]]') })
    nodes.set(`${ROOT}/wiki/local/丁.md`, { kind: 'file', bytes: encode('丁的正文') })
    await user.click(screen.getByRole('button', { name: '重新读取' }))
    await waitFor(() => {
      openRailGroup('反向链接')
      expect(within(rail).getByText('新的来处 [[甲]]')).toBeInTheDocument()
    })
    expect(await within(listNav()).findByRole('button', { name: '丁' })).toBeInTheDocument()
    expect(office.extensions).toBe(extensions)
    expect(office.extensions.find((item) => item.name === 'knowledgeWikiLink')).toBe(wiki)
    expect(wiki?.options?.notes?.map((note) => note.name)).toContain('丁.md')
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', office.edited)
  })

  it('opens a long note as source text and refuses a file past the read budget', async () => {
    const body = `[[目标]]${'a'.repeat(VISUAL_TEXT_UNITS)}`
    renderNotes([
      { name: '长.md', bytes: encode(body) },
      { name: '过大.md', bytes: encode('x') }
    ])
    const user = userEvent.setup()
    await user.click(await within(listNav()).findByRole('button', { name: '长' }))
    const field = await screen.findByRole('textbox', { name: '长.md' })
    expect((field as HTMLTextAreaElement).value).toHaveLength(body.length)
    expect((field as HTMLTextAreaElement).value).toContain('[[目标]]')
    expect(document.querySelector('[data-editor="markdown"]')).not.toBeInTheDocument()

    await user.click(within(listNav()).getByRole('button', { name: '过大' }))
    expect(await screen.findByRole('heading', { name: '文件过大' })).toBeInTheDocument()
    expect(screen.getByText('这个文件有 20MB。应用内编辑器不打开它。')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '用默认应用打开' })).toBeInTheDocument()
    expect(screen.queryByRole('textbox', { name: '过大.md' })).not.toBeInTheDocument()
    expect(screen.queryByRole('textbox', { name: '长.md' })).not.toBeInTheDocument()
    expect(document.querySelector('[data-editor="markdown"]')).not.toBeInTheDocument()
  })

  it('opens a backlink and filters the tree by file name', async () => {
    const user = userEvent.setup()
    renderNotes([
      { name: '甲.md', bytes: encode('见 [[乙]]') },
      { name: '乙.md', bytes: encode('乙的正文') },
      { name: '船票.md', bytes: encode('无关') },
      { name: '日程.md', bytes: encode('下午买船票') }
    ])

    await user.click(await within(listNav()).findByRole('button', { name: '乙' }))
    const rail = await screen.findByRole('complementary', { name: '这篇笔记' })
    await waitFor(() => expect(within(rail).getByRole('button', { name: '反向链接' })).toBeInTheDocument())
    expect(within(rail).queryByText(/已载入/)).not.toBeInTheDocument()
    openRailGroup('反向链接')
    const backlink = await within(rail).findByRole('button', { name: '甲' })
    expect(backlink).toHaveTextContent('见 [[乙]]')
    await user.click(backlink)
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', '见 [[乙]]')
    )
    openRailGroup('链接')
    expect(
      within(screen.getByRole('complementary', { name: '这篇笔记' })).getByRole('button', { name: '乙' })
    ).toBeInTheDocument()

    fireEvent.change(screen.getByPlaceholderText('搜索笔记'), { target: { value: '船票' } })
    expect(screen.queryByRole('navigation', { name: '正文' })).not.toBeInTheDocument()
    expect(screen.queryByRole('radio', { name: '文件名' })).not.toBeInTheDocument()
    expect(screen.queryByRole('radio', { name: '正文' })).not.toBeInTheDocument()
    expect(within(listNav()).getByRole('button', { name: '船票' })).toBeInTheDocument()
    expect(within(listNav()).queryByRole('button', { name: '日程' })).not.toBeInTheDocument()
  })

  it('lists notes only inside the requested vault directory', async () => {
    renderNotes(
      [
        { name: '甲.md', bytes: encode('工作区里的船票') },
        { name: 'nested/日程.md', bytes: encode('目录里的船票') }
      ],
      { vaultDirectory: `${ROOT}/wiki/local/nested` }
    )

    expect(await within(listNav()).findByRole('button', { name: '日程' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '甲' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '知识编译' })).not.toBeInTheDocument()
    expect(screen.queryByRole('navigation', { name: '概念' })).not.toBeInTheDocument()
    expect(screen.queryByRole('radio', { name: '正文' })).not.toBeInTheDocument()
  })

  it('refuses a new note named index.md or log.md', async () => {
    renderNotes([])
    const nav = listNav()
    const root = await within(nav).findByRole('button', { name: '笔记' })
    fireEvent.click(within(root.parentElement as HTMLElement).getByRole('button', { name: '新建文件' }))
    const name = screen.getByRole('textbox', { name: '名称' })
    fireEvent.change(name, { target: { value: 'index.md' } })
    fireEvent.submit(name.closest('form') as HTMLFormElement)
    expect(await screen.findByRole('alert')).toHaveTextContent('不能使用这个保留名称。')
    fireEvent.change(screen.getByRole('textbox', { name: '名称' }), { target: { value: 'log.md' } })
    fireEvent.submit(screen.getByRole('textbox', { name: '名称' }).closest('form') as HTMLFormElement)
    expect(await screen.findByRole('alert')).toHaveTextContent('不能使用这个保留名称。')
    expect(within(nav).queryByRole('button', { name: 'index' })).not.toBeInTheDocument()
    expect(within(nav).queryByRole('button', { name: 'log' })).not.toBeInTheDocument()
  })

  it('refuses a new note with an empty name', async () => {
    const { writes } = renderNotes([])
    const nav = listNav()
    const root = await within(nav).findByRole('button', { name: '笔记' })
    fireEvent.click(within(root.parentElement as HTMLElement).getByRole('button', { name: '新建文件' }))
    const name = screen.getByRole('textbox', { name: '名称' })
    fireEvent.change(name, { target: { value: '' } })
    fireEvent.submit(name.closest('form') as HTMLFormElement)
    expect(await screen.findByRole('alert')).toHaveTextContent('名称不能为空。')
    expect(writes).toHaveLength(0)
    expect(within(nav).queryByRole('button', { name: '未命名' })).not.toBeInTheDocument()
  })

  it('keeps an existing type line on save and hides it from the editor', async () => {
    const user = userEvent.setup()
    const stamped = `---\ntype: Note\ntitle: "甲"\n---\n${JIA}`
    const { writes } = renderNotes([{ name: '甲.md', bytes: encode(stamped) }])
    await user.click(await within(listNav()).findByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', JIA)
    )
    await user.click(screen.getByRole('button', { name: '保存' }))
    await waitFor(() => {
      const body = writes.at(-1)?.body ?? ''
      expect(body.startsWith('---\ntype: Note\n---\n')).toBe(true)
      expect(body.endsWith(office.saved)).toBe(true)
    })
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', office.saved)
  })

  it('opens a read-only concept and filters skipped notes from the saved index', async () => {
    const user = userEvent.setup()
    const prose = '见轮渡\n\n[乙](../乙.md)\n[缺失](缺失.md)\n'
    const concept = `---\ntype: Document\ntitle: "甲"\nresource: "../甲.md"\nsources:\n  - id: source\n    resource: "../甲.md"\n    title: "甲"\n---\n\n${prose}\n## Sources\n\n- [甲](../甲.md)\n`
    const { nodes } = renderNotes([
      { name: '甲.md', bytes: encode(`---\ntype: Note\ntitle: "甲"\n---\n${JIA}`) },
      { name: '乙.md', bytes: encode(YI) },
      { name: '空.md', bytes: encode('\n') },
      {
        name: 'index.md',
        bytes: encode('---\nokf_version: "0.2"\n---\n\n- [甲](/concepts/甲.md)\n\n## Skipped\n\n- `空.md` — empty\n')
      },
      { name: 'concepts/甲.md', bytes: encode(concept) }
    ])
    const compile = screen.getByRole('button', { name: '知识编译' })
    expect(compile).toHaveAttribute('data-variant', 'emphasis')
    expect(screen.queryByText(/篇未变/)).not.toBeInTheDocument()
    await user.click(compile)
    const dialog = await screen.findByRole('dialog', { name: '知识编译' })
    expect(within(dialog).getByText(/在这里阅读这个库并编写概念页/)).toBeInTheDocument()
    expect(within(dialog).queryByText(/a3s/i)).not.toBeInTheDocument()
    expect(within(dialog).queryByRole('button', { name: '关闭' })).not.toBeInTheDocument()
    expect(within(dialog).queryByRole('region', { name: '编译进度' })).not.toBeInTheDocument()
    expect(within(dialog).queryByRole('status', { name: '编译完成' })).not.toBeInTheDocument()
    expect(within(dialog).getByText(compileCount('1 个概念'))).toBeInTheDocument()
    expect(screen.queryByText(compileCount('0 篇未变'))).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '0 篇失败' })).not.toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: '取消' }))
    expect(screen.queryByRole('dialog', { name: '知识编译' })).not.toBeInTheDocument()
    const notesNav = listNav()
    expect(within(notesNav).queryByRole('button', { name: 'index' })).not.toBeInTheDocument()
    expect(within(notesNav).queryByRole('button', { name: 'concepts' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('radio', { name: '概念' }))
    const concepts = await screen.findByRole('navigation', { name: '概念' })
    const conceptRow = await within(concepts).findByRole('button', { name: '甲' })
    expect(conceptRow).not.toHaveTextContent('只读')
    expect(within(concepts).getAllByText('只读')).toHaveLength(1)
    expect(conceptRow).toHaveAccessibleName('甲')
    await user.click(conceptRow)
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute(
        'data-markdown',
        expect.stringContaining('见轮渡')
      )
    )
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-preview', 'true')
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-extensions', '')
    expect(screen.getByText('Document')).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '保存' })).not.toBeInTheDocument()
    expect(screen.queryByRole('navigation', { name: '链接' })).not.toBeInTheDocument()
    const rail = screen.getByRole('complementary', { name: '这个概念' })
    expect(within(rail).getByText('缺失')).toBeInTheDocument()
    await user.click(within(rail).getByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', JIA)
    )
    expect(screen.getByRole('button', { name: '保存' })).toBeInTheDocument()
    await user.click(within(screen.getByRole('navigation', { name: '概念' })).getByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute(
        'data-markdown',
        expect.stringContaining('见轮渡')
      )
    )
    expect(screen.queryByRole('button', { name: '保存' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '知识编译' }))
    await user.click(
      within(await screen.findByRole('dialog', { name: '知识编译' })).getByRole('button', { name: '1 篇跳过' })
    )
    expect(screen.queryByRole('dialog', { name: '知识编译' })).not.toBeInTheDocument()
    expect(screen.getByRole('list', { name: '编译结果' })).toHaveTextContent('这篇没有正文')
    expect(within(listNav()).getByRole('button', { name: '空' })).toBeInTheDocument()
    expect(within(listNav()).queryByRole('button', { name: '甲' })).not.toBeInTheDocument()
    expect(screen.queryByRole('navigation', { name: '概念' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '空，这篇没有正文' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', '\n')
    )
    expect(screen.queryByRole('navigation', { name: '概念' })).not.toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '知识编译' }))
    await user.click(
      within(await screen.findByRole('dialog', { name: '知识编译' })).getByRole('button', { name: '1 篇跳过' })
    )
    expect(await screen.findByRole('navigation', { name: '概念' })).toBeInTheDocument()
    expect(nodes.get(`${ROOT}/wiki/local/concepts/甲.md`)?.kind).toBe('file')
  })

  it('starts an a3s code session inside the dialog', async () => {
    let finishAccept: ((counts: { concepts: number; skipped: number; failed: number }) => void) | null = null
    const invoke = vi.fn(
      compileShell(['丙.md', '乱码.md'], () => {
        return new Promise((resolve) => {
          finishAccept = resolve
        })
      })
    )
    Object.defineProperty(globalThis, '__TAURI_INTERNALS__', {
      configurable: true,
      value: { invoke }
    })
    compileAgent.start.mockResolvedValue({ sessionId: 'compile-session', agentId: 'work-agent' })
    const user = userEvent.setup()
    const { nodes } = renderNotes(notes())
    await user.click(screen.getByRole('radio', { name: '概念' }))
    expect(await screen.findByText('编译后会在这里列出概念')).toBeInTheDocument()
    await user.click(screen.getByRole('radio', { name: '笔记' }))
    await user.click(await within(listNav()).findByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', JIA)
    )
    await user.click(screen.getByRole('button', { name: '写入编辑' }))
    await user.click(screen.getByRole('button', { name: '知识编译' }))
    const dialog = await screen.findByRole('dialog', { name: '知识编译' })
    expect(dialog).toHaveAttribute('data-size', 'lg')
    expect(dialog.querySelector('select, datalist')).toBeNull()
    expect(within(dialog).getByText('在这里阅读这个库并编写概念页。正在编辑的正文会保留。')).toBeInTheDocument()
    expect(within(dialog).queryByText(/a3s/i)).not.toBeInTheDocument()
    nodes.set(`${ROOT}/wiki/local/concepts`, { kind: 'directory' })
    nodes.set(`${ROOT}/wiki/local/index.md`, {
      kind: 'file',
      modifiedMs: Date.now() + 60_000,
      bytes: encode(
        '---\nokf_version: "0.2"\n---\n\n- [甲](/concepts/甲.md)\n- [乙](/concepts/乙.md)\n\n## Skipped\n\n- `丙.md` — empty\n\n## Failed\n\n- `乱码.md` — encoding\n'
      )
    })
    nodes.set(`${ROOT}/wiki/local/concepts/甲.md`, {
      kind: 'file',
      modifiedMs: Date.now() + 60_000,
      bytes: encode('---\ntype: Document\ntitle: "甲"\n---\n\n')
    })
    nodes.set(`${ROOT}/wiki/local/concepts/乙.md`, {
      kind: 'file',
      modifiedMs: 1,
      bytes: encode('---\ntype: Document\ntitle: "乙"\n---\n\n')
    })
    await user.click(within(dialog).getByRole('button', { name: '开始编译' }))
    const stream = await within(dialog).findByRole('log', { name: '编译记录' })
    const progress = await within(dialog).findByRole('progressbar', { name: '编译进度' })
    expect(progress).toHaveAttribute('aria-valuemin', '0')
    expect(progress).toHaveAttribute('aria-valuemax', '2')
    expect(progress).toHaveAttribute('aria-valuenow', '0')
    expect(within(dialog).getByText('0 / 2')).toBeInTheDocument()
    expect(within(dialog).queryByText(compileCount('2 个概念'))).not.toBeInTheDocument()
    expect(stream).toHaveAttribute('data-ui', 'knowledge.compile.agent')
    expect(stream).toHaveAttribute('data-session', 'compile-session')
    expect(compileAgent.start).toHaveBeenCalledWith(`${ROOT}/wiki/local`)
    expect(compileAgent.dispatch).toHaveBeenCalledTimes(1)
    expect(compileAgent.dispatch).toHaveBeenCalledWith(
      'compile-session',
      '只编译下面这些笔记。工作区就是这个库。为每一篇编写 concepts/ 下的概念页，frontmatter 写 type: Document，并链回原笔记。空笔记不要建概念页。不要删除笔记，也不要改写笔记正文。其他概念页不要改。\n丙.md\n乱码.md'
    )
    expect(invoke.mock.calls.map((call) => call[0])).not.toContain('compile_personal_okf')
    expect(within(dialog).queryByRole('button', { name: '开始编译' })).not.toBeInTheDocument()
    expect(within(dialog).queryByRole('button', { name: '取消' })).not.toBeInTheDocument()
    act(() => {
      compileAgent.settled?.()
    })
    expect(within(dialog).queryByRole('button', { name: '关闭' })).not.toBeInTheDocument()
    expect(within(dialog).getByText(/正在核对这个库/)).toBeInTheDocument()
    expect(within(dialog).queryByText(/a3s/i)).not.toBeInTheDocument()
    await waitFor(() => expect(invoke).toHaveBeenCalledWith('accept_personal_okf', { root: `${ROOT}/wiki/local` }))
    expect(invoke.mock.calls.map((call) => call[0])).not.toContain('compile_personal_okf')
    console.log('关闭 absent when the turn settles')
    console.log('running progress is 0 / 2')
    console.log('does not become full merely because index.md is newer')
    console.log('dialog text has no a3s match')
    await act(async () => {
      finishAccept?.({ concepts: 1, skipped: 1, failed: 1 })
    })
    expect(await within(dialog).findByRole('button', { name: '关闭' })).toBeInTheDocument()
    expect(within(dialog).getByText(compileCount('1 个概念'))).toBeInTheDocument()
    expect(within(dialog).getByRole('button', { name: '1 篇跳过' })).toBeInTheDocument()
    expect(within(dialog).getByRole('button', { name: '1 篇失败' })).toBeInTheDocument()
    expect(within(dialog).queryByText(compileCount('2 个概念'))).not.toBeInTheDocument()
    console.log('关闭 present only after accept succeeds')
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', office.edited)
    expect(await screen.findByText('大纲按当前正文。')).toBeInTheDocument()
    expect(screen.getByText('链接、反向链接、标签、待办和属性按已保存内容。')).toBeInTheDocument()
    expect(screen.getByRole('status', { name: '未保存' })).toBeInTheDocument()
    await user.click(within(dialog).getByRole('button', { name: '关闭' }))
    expect(screen.queryByRole('dialog', { name: '知识编译' })).not.toBeInTheDocument()
  })

  it('accepts a vault with nothing new without starting a session', async () => {
    const invoke = vi.fn(compileShell([], () => ({ concepts: 1, skipped: 1, failed: 0 })))
    Object.defineProperty(globalThis, '__TAURI_INTERNALS__', {
      configurable: true,
      value: { invoke }
    })
    const user = userEvent.setup()
    renderNotes([{ name: '甲.md', bytes: encode('甲') }])
    await user.click(await screen.findByRole('button', { name: '知识编译' }))
    const dialog = await screen.findByRole('dialog', { name: '知识编译' })
    await user.click(within(dialog).getByRole('button', { name: '开始编译' }))
    expect(await within(dialog).findByRole('button', { name: '关闭' })).toBeInTheDocument()
    expect(within(dialog).queryByRole('log', { name: '编译记录' })).not.toBeInTheDocument()
    expect(compileAgent.start).not.toHaveBeenCalled()
    expect(compileAgent.dispatch).not.toHaveBeenCalled()
    expect(invoke.mock.calls.map((call) => call[0])).not.toContain('remember_personal_okf')
    expect(invoke.mock.calls.map((call) => call[0])).not.toContain('pin_personal_okf')
    expect(invoke.mock.calls.map((call) => call[0])).not.toContain('release_personal_okf')
    expect(invoke).toHaveBeenCalledWith('accept_personal_okf', { root: `${ROOT}/wiki/local` })
    expect(invoke.mock.calls.map((call) => call[0])).not.toContain('compile_personal_okf')
  })

  it('shows 知识编译没有完成 when accept is rejected and leaves start compile ready', async () => {
    const invoke = vi.fn(compileShell(['甲.md'], () => Promise.reject(new Error('accept_personal_okf'))))
    Object.defineProperty(globalThis, '__TAURI_INTERNALS__', {
      configurable: true,
      value: { invoke }
    })
    compileAgent.start.mockResolvedValue({ sessionId: 'compile-session', agentId: 'work-agent' })
    const user = userEvent.setup()
    renderNotes([{ name: '甲.md', bytes: encode('甲') }])
    await user.click(await screen.findByRole('button', { name: '知识编译' }))
    const dialog = await screen.findByRole('dialog', { name: '知识编译' })
    await user.click(within(dialog).getByRole('button', { name: '开始编译' }))
    expect(await within(dialog).findByRole('log', { name: '编译记录' })).toBeInTheDocument()
    act(() => {
      compileAgent.settled?.()
    })
    expect(await within(dialog).findByRole('alert')).toHaveTextContent('知识编译没有完成。')
    expect(within(dialog).getByRole('log', { name: '编译记录' })).toBeInTheDocument()
    expect(within(dialog).queryByRole('button', { name: '关闭' })).not.toBeInTheDocument()
    expect(within(dialog).getByRole('button', { name: '开始编译' })).toBeEnabled()
    expect(invoke.mock.calls.map((call) => call[0])).toContain('accept_personal_okf')
    expect(invoke.mock.calls.map((call) => call[0])).not.toContain('compile_personal_okf')
    expect(within(dialog).queryByText(/a3s/i)).not.toBeInTheDocument()
    console.log('rejected accept shows 知识编译没有完成')
    console.log('开始编译 enabled')
  })

  it('restores the unsaved draft when the knowledge page mounts again', async () => {
    const first = renderNotes([{ name: '甲.md', bytes: encode(JIA) }])
    const user = userEvent.setup()
    await user.click(await within(listNav()).findByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', JIA)
    )
    await user.click(screen.getByRole('button', { name: '写入编辑' }))
    expect(await screen.findByRole('status', { name: '未保存' })).toBeInTheDocument()
    expect(first.writes).toHaveLength(0)
    cleanup()
    const second = renderNotes([{ name: '甲.md', bytes: encode(JIA) }])
    expect(await screen.findByRole('status', { name: '未保存' })).toBeInTheDocument()
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', office.edited)
    )
    expect(first.writes).toHaveLength(0)
    expect(second.writes).toHaveLength(0)
  })

  it('finds a concept by its saved passage and does not search the workspace encyclopedia', async () => {
    const invoke = vi.fn(async () => {
      throw new Error('workspace search is not used here')
    })
    Object.defineProperty(globalThis, '__TAURI_INTERNALS__', {
      configurable: true,
      value: { invoke }
    })
    const concept = '---\ntype: Document\ntitle: "甲"\nresource: "../甲.md"\n---\n\n下午的轮渡\n'
    renderNotes([
      { name: '甲.md', bytes: encode('笔记里没有那个词') },
      { name: 'index.md', bytes: encode('---\nokf_version: "0.2"\n---\n\n- [甲](/concepts/甲.md)\n') },
      { name: 'concepts/甲.md', bytes: encode(concept) }
    ])
    expect(screen.queryByText(compileCount('1 个概念'))).not.toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: '知识编译' }))
    const dialog = await screen.findByRole('dialog', { name: '知识编译' })
    expect(within(dialog).getByText(compileCount('1 个概念'))).toBeInTheDocument()
    expect(screen.queryByText(/篇未变/)).not.toBeInTheDocument()
    await userEvent.click(within(dialog).getByRole('button', { name: '取消' }))
    await userEvent.click(screen.getByRole('radio', { name: '概念' }))
    fireEvent.change(screen.getByPlaceholderText('搜索笔记'), { target: { value: '轮渡' } })
    expect(screen.queryByRole('navigation', { name: '笔记' })).not.toBeInTheDocument()
    const concepts = await screen.findByRole('navigation', { name: '概念' })
    expect(await within(concepts).findByRole('button', { name: '甲' })).toBeInTheDocument()
    expect(screen.queryByRole('navigation', { name: '正文' })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '根' })).not.toBeInTheDocument()
    expect(invoke).not.toHaveBeenCalled()
    await userEvent.click(within(concepts).getByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute(
        'data-markdown',
        expect.stringContaining('轮渡')
      )
    )
    expect(screen.queryByRole('button', { name: '保存' })).not.toBeInTheDocument()
  })

  it('offers the five newest notes and keeps the center from opening one', async () => {
    const user = userEvent.setup()
    renderNotes([
      { name: '旧.md', bytes: encode('看起来很新'), modifiedMs: 1 },
      { name: '中.md', bytes: encode('中'), modifiedMs: 4 },
      { name: '次.md', bytes: encode('次'), modifiedMs: 5 },
      { name: '再.md', bytes: encode('再'), modifiedMs: 6 },
      { name: '更.md', bytes: encode('更'), modifiedMs: 7 },
      { name: '最新.md', bytes: encode('旧的正文'), modifiedMs: 8 },
      { name: '夹/新.md', bytes: encode('夹里'), modifiedMs: 9 }
    ])
    expect(screen.getByText('正在读取笔记')).toBeInTheDocument()
    const recent = await screen.findByRole('navigation', { name: '继续写' })
    expect(screen.queryByText('正在读取笔记')).not.toBeInTheDocument()
    expect(screen.queryByText('还没有打开笔记')).not.toBeInTheDocument()
    expect(document.querySelector('[data-editor="markdown"]')).not.toBeInTheDocument()
    expect(
      within(recent)
        .getAllByRole('button')
        .map((button) => button.querySelector('span')?.textContent)
    ).toEqual(['新', '最新', '更', '再', '次'])
    expect(within(recent).getAllByRole('button')[0]).toHaveTextContent(/\d{2}:\d{2}/)
    expect(within(recent).queryByRole('button', { name: '旧' })).not.toBeInTheDocument()
    expect(within(recent).queryByRole('button', { name: '中' })).not.toBeInTheDocument()
    const center = recent.parentElement as HTMLElement
    expect(within(center).getByRole('button', { name: '新建文件' })).toBeInTheDocument()
    expect(within(center).getByRole('button', { name: '新建文件夹' })).toBeInTheDocument()
    expect(within(listNav()).getByRole('button', { name: '旧' })).toBeInTheDocument()
    await user.click(within(recent).getByRole('button', { name: '最新' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', '旧的正文')
    )
    expect(screen.getByRole('heading', { level: 2, name: '最新' })).toBeInTheDocument()
  })

  it('follows the draft outline, toggles a body tag, and keeps the saved reading rail', async () => {
    const user = userEvent.setup()
    const saved = '---\nstatus: draft\n---\n# 已保存标题\n\n见 [[乙]]\n[[还没有]]\n#旅行\n- [ ] 买票\n'
    renderNotes([
      { name: '甲.md', bytes: encode(saved) },
      { name: '乙.md', bytes: encode('见 [[甲]] 的来处\n#旅行') },
      { name: '丙.md', bytes: encode('---\ntags: 旅行\n---\n没有井号') }
    ])
    await user.click(await within(listNav()).findByRole('button', { name: '甲' }))
    const rail = await screen.findByRole('complementary', { name: '这篇笔记' })
    await waitFor(() =>
      expect(within(rail).getByRole('button', { name: '大纲' })).toHaveAttribute('aria-expanded', 'true')
    )
    expect(within(rail).queryByText(/已载入/)).not.toBeInTheDocument()
    expect(
      within(rail)
        .getAllByRole('region')
        .map((region) => region.getAttribute('aria-label'))
    ).toEqual(['大纲', '链接', '未在此库中', '反向链接', '标签', '待办', '属性'])
    expect(screen.queryByText('大纲按当前正文。')).not.toBeInTheDocument()
    expect(screen.queryByRole('status', { name: '未保存' })).not.toBeInTheDocument()
    expect(within(rail).getByRole('button', { name: '已保存标题' })).toBeInTheDocument()
    openRailGroup('反向链接')
    expect(
      within(within(rail).getByRole('region', { name: '反向链接' })).getByRole('button', { name: '乙' })
    ).toHaveTextContent('见 [[甲]] 的来处')
    openRailGroup('属性')
    expect(within(rail).getByText('status: draft')).toBeInTheDocument()
    openRailGroup('待办')
    expect(within(rail).getByRole('button', { name: '买票' })).toBeInTheDocument()
    expect(within(rail).queryByRole('button', { name: '档案' })).not.toBeInTheDocument()

    openRailGroup('标签')
    await user.click(within(rail).getByRole('button', { name: '旅行' }))
    expect(within(rail).getByRole('button', { name: '旅行' })).toHaveAttribute('aria-pressed', 'true')
    expect(within(listNav()).getByRole('button', { name: '甲' })).toBeInTheDocument()
    expect(within(listNav()).getByRole('button', { name: '乙' })).toBeInTheDocument()
    expect(within(listNav()).queryByRole('button', { name: '丙' })).not.toBeInTheDocument()
    await user.click(within(rail).getByRole('button', { name: '旅行' }))
    expect(within(listNav()).getByRole('button', { name: '丙' })).toBeInTheDocument()

    office.edited = '# 新标题\n\n见 [[丙]]\n\n```\n# 假标题\n```\n'
    await user.click(screen.getByRole('button', { name: '写入编辑' }))
    await waitFor(() => expect(within(rail).getByRole('button', { name: '新标题' })).toBeInTheDocument())
    expect(within(rail).queryByRole('button', { name: '假标题' })).not.toBeInTheDocument()
    expect(within(rail).queryByRole('button', { name: '已保存标题' })).not.toBeInTheDocument()
    expect(within(rail).getByText('大纲按当前正文。')).toBeInTheDocument()
    expect(within(rail).getByText('链接、反向链接、标签、待办和属性按已保存内容。')).toBeInTheDocument()
    expect(screen.getByRole('status', { name: '未保存' })).toBeInTheDocument()
    openRailGroup('链接')
    const links = within(rail).getByRole('region', { name: '链接' })
    expect(within(links).getByRole('button', { name: '乙' })).toBeInTheDocument()
    expect(within(links).queryByRole('button', { name: '丙' })).not.toBeInTheDocument()
    await user.click(within(rail).getByRole('button', { name: '新标题' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-reveal', '新标题')
    )
  })

  it('confirms before creating an unresolved note and opens a title that already exists', async () => {
    const user = userEvent.setup()
    const source = '[[乙]]\n[[团队纪要]]\n[[..]]\n[[目录/秘密]]\n[[index.md]]\n[[log]]\n[[后来]]'
    const { writes, nodes } = renderNotes([
      { name: '甲.md', bytes: encode(source) },
      { name: '乙.md', bytes: encode('乙的正文') }
    ])
    const rowFor = (label: string) => {
      openRailGroup('未在此库中')
      const region = within(screen.getByRole('complementary', { name: '这篇笔记' })).getByRole('region', {
        name: '未在此库中'
      })
      return within(region).getByText(label).parentElement as HTMLElement
    }
    const openDialog = async (label: string) => {
      await user.click(within(rowFor(label)).getByRole('button', { name: '创建' }))
      return screen.findByRole('dialog', { name: '创建这篇笔记？' })
    }

    await user.click(await within(listNav()).findByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', source)
    )
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute(
      'data-extensions',
      expect.stringContaining('knowledgeWikiLink')
    )
    const wiki = office.extensions.find((item) => item.name === 'knowledgeWikiLink')
    act(() => {
      wiki?.options?.onOpen?.(`${ROOT}/wiki/local/乙.md`)
    })
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', '乙的正文')
    )
    expect(writes).toHaveLength(0)
    await user.click(within(listNav()).getByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', source)
    )
    openRailGroup('链接')
    openRailGroup('未在此库中')
    expect(
      within(
        within(screen.getByRole('complementary', { name: '这篇笔记' })).getByRole('region', { name: '链接' })
      ).getByRole('button', { name: '乙' })
    ).toBeInTheDocument()
    expect(
      within(
        within(screen.getByRole('complementary', { name: '这篇笔记' })).getByRole('region', { name: '未在此库中' })
      ).queryByText('乙')
    ).not.toBeInTheDocument()

    const cancelled = await openDialog('团队纪要')
    await user.click(within(cancelled).getByRole('button', { name: '取消' }))
    await waitFor(() => expect(screen.queryByRole('dialog', { name: '创建这篇笔记？' })).not.toBeInTheDocument())
    expect(writes).toHaveLength(0)
    expect(within(listNav()).queryByRole('button', { name: '团队纪要' })).not.toBeInTheDocument()

    const creating = await openDialog('团队纪要')
    await user.click(within(creating).getByRole('button', { name: '创建这篇笔记' }))
    await waitFor(() => {
      expect(writes.some((write) => write.path === `${ROOT}/wiki/local/团队纪要.md` && write.body === '\n')).toBe(true)
    })
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', '\n')
    )
    expect(screen.getByRole('heading', { level: 2, name: '团队纪要' })).toBeInTheDocument()

    await user.click(within(listNav()).getByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', source)
    )
    const refused = [
      ['..', '名称不能为空。'],
      ['秘密', '名称不能包含路径分隔符。'],
      ['index', '不能使用这个保留名称。'],
      ['log', '不能使用这个保留名称。']
    ] as const
    for (const [label, message] of refused) {
      const dialog = await openDialog(label)
      await user.click(within(dialog).getByRole('button', { name: '创建这篇笔记' }))
      expect(await screen.findByRole('alert')).toHaveTextContent(message)
      await waitFor(() => expect(screen.queryByRole('dialog', { name: '创建这篇笔记？' })).not.toBeInTheDocument())
    }
    expect(writes).toHaveLength(1)
    expect(within(listNav()).queryByRole('button', { name: 'index' })).not.toBeInTheDocument()
    expect(within(listNav()).queryByRole('button', { name: 'log' })).not.toBeInTheDocument()

    nodes.set(`${ROOT}/wiki/local/后来.md`, { kind: 'file', bytes: encode('已有正文'), modifiedMs: 3 })
    const before = writes.length
    const existing = await openDialog('后来')
    await user.click(within(existing).getByRole('button', { name: '创建这篇笔记' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', '已有正文')
    )
    expect(writes).toHaveLength(before)
    expect([...nodes.keys()].filter((path) => path.endsWith('/后来.md'))).toEqual([`${ROOT}/wiki/local/后来.md`])
  })

  it('shows unread coverage and filters indexed notes by tag', async () => {
    const invoke = vi.fn(async (command: string) => {
      throw new Error(command)
    })
    Object.defineProperty(globalThis, '__TAURI_INTERNALS__', {
      configurable: true,
      value: { invoke }
    })
    const files: FixtureFile[] = []
    for (let index = 0; index <= NOTE_INDEX_LIMIT; index += 1) {
      const name = `n${String(index).padStart(3, '0')}.md`
      const body = index === 0 ? '共同词\n#标记' : index === NOTE_INDEX_LIMIT ? '独有词\n#标记' : '普通'
      files.push({ name, bytes: encode(body), modifiedMs: index + 1 })
    }
    renderNotes(files)
    const user = userEvent.setup()
    const coverage = `已载入 ${NOTE_INDEX_LIMIT} / ${NOTE_INDEX_LIMIT + 1} 篇`
    await user.click(await within(listNav()).findByRole('button', { name: 'n000' }))
    const rail = await screen.findByRole('complementary', { name: '这篇笔记' })
    await waitFor(() => expect(within(rail).getByText(coverage)).toBeInTheDocument())

    fireEvent.change(screen.getByPlaceholderText('搜索笔记'), { target: { value: '共同词' } })
    expect(screen.queryByRole('navigation', { name: '正文' })).not.toBeInTheDocument()
    expect(screen.queryByRole('radio', { name: '正文' })).not.toBeInTheDocument()
    expect(within(listNav()).queryByRole('button', { name: 'n000' })).not.toBeInTheDocument()
    fireEvent.change(screen.getByPlaceholderText('搜索笔记'), { target: { value: '' } })
    expect(invoke.mock.calls.map((call) => call[0])).not.toContain('search_workspace_okf')

    openRailGroup('标签')
    await user.click(within(rail).getByRole('button', { name: '标记' }))
    expect(within(listNav()).getByRole('button', { name: 'n000' })).toBeInTheDocument()
    expect(within(listNav()).queryByRole('button', { name: 'n200' })).not.toBeInTheDocument()
    expect(listNav().parentElement).toHaveTextContent(coverage)
    await user.click(within(rail).getByRole('button', { name: '标记' }))
    expect(within(listNav()).getByRole('button', { name: 'n200' })).toBeInTheDocument()
  })

  it('shows skipped and failed counts from the saved index without compiling', async () => {
    const invoke = vi.fn(async (command: string) => {
      throw new Error(command)
    })
    Object.defineProperty(globalThis, '__TAURI_INTERNALS__', {
      configurable: true,
      value: { invoke }
    })
    const index = [
      '---',
      'okf_version: "0.2"',
      '---',
      '',
      '- [甲](/concepts/甲.md)',
      '- [乙文](/concepts/乙.md)',
      '',
      '## Skipped',
      '',
      '- `空.md` — empty',
      '',
      '## Failed',
      '',
      '- `坏.md` — frontmatter',
      ''
    ].join('\n')
    renderNotes([
      { name: '甲.md', bytes: encode('笔记') },
      { name: '乙.md', bytes: encode('乙') },
      { name: '空.md', bytes: encode('\n') },
      { name: '坏.md', bytes: encode('坏') },
      { name: 'index.md', bytes: encode(index) },
      {
        name: 'concepts/甲.md',
        bytes: encode('---\ntype: Document\ntitle: "甲"\nresource: "../甲.md"\n---\n\n概念正文\n')
      },
      { name: 'concepts/乙.md', bytes: encode('---\ntype: Document\ntitle: "乙文"\n---\n\n乙概念\n') }
    ])
    expect(screen.queryByText(compileCount('2 个概念'))).not.toBeInTheDocument()
    const toolbar = screen.getByRole('group', { name: '知识操作' })
    expect(within(toolbar).queryByRole('button', { name: '1 篇跳过' })).not.toBeInTheDocument()
    expect(within(toolbar).queryByRole('button', { name: '1 篇失败' })).not.toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: '知识编译' }))
    const dialog = await screen.findByRole('dialog', { name: '知识编译' })
    const compiled = within(dialog).getByText(compileCount('2 个概念'))
    expect(compiled.querySelector('span')).toHaveClass('tabular-nums', 'text-[#171717]')
    expect(toolbar).not.toContainElement(compiled)
    expect(within(dialog).getByRole('button', { name: '1 篇跳过' })).toBeInTheDocument()
    expect(within(dialog).getByRole('button', { name: '1 篇失败' })).toBeInTheDocument()
    expect(within(dialog).queryByText(/篇未变/)).not.toBeInTheDocument()
    expect(invoke).not.toHaveBeenCalled()
    await userEvent.click(within(dialog).getByRole('button', { name: '1 篇跳过' }))
    expect(screen.queryByRole('dialog', { name: '知识编译' })).not.toBeInTheDocument()
    expect(within(listNav()).getByRole('button', { name: '空' })).toBeInTheDocument()
    expect(within(listNav()).queryByRole('button', { name: '甲' })).not.toBeInTheDocument()
    expect(screen.queryByRole('navigation', { name: '概念' })).not.toBeInTheDocument()
    await userEvent.click(screen.getByRole('button', { name: '知识编译' }))
    const again = await screen.findByRole('dialog', { name: '知识编译' })
    expect(within(again).getByRole('button', { name: '1 篇跳过' })).toHaveAttribute('aria-pressed', 'true')
    await userEvent.click(within(again).getByRole('button', { name: '取消' }))
    await userEvent.click(screen.getByRole('button', { name: '返回全部笔记' }))
    expect(within(listNav()).getByRole('button', { name: '甲' })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: '返回全部笔记' })).not.toBeInTheDocument()
    await userEvent.click(screen.getByRole('radio', { name: '概念' }))
    const concepts = await screen.findByRole('navigation', { name: '概念' })
    expect(within(concepts).getByText('来自上次编译')).toBeInTheDocument()
    const titled = within(concepts).getByRole('button', { name: '乙文' })
    expect(titled).not.toHaveTextContent('只读')
    expect(within(concepts).getAllByText('只读')).toHaveLength(1)
    expect(titled).toHaveAccessibleName('乙文')
    await userEvent.click(within(concepts).getByRole('button', { name: '甲' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute(
        'data-markdown',
        expect.stringContaining('概念正文')
      )
    )
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-preview', 'true')
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-extensions', '')
    expect(screen.queryByRole('button', { name: '保存' })).not.toBeInTheDocument()
    expect(invoke).not.toHaveBeenCalled()
  })

  it('keeps the compile dialog open while a3s code is running', async () => {
    compileAgent.start.mockResolvedValue({ sessionId: 'compile-session', agentId: 'work-agent' })
    renderNotes([{ name: '甲.md', bytes: encode('甲') }])
    const user = userEvent.setup()
    await within(listNav()).findByRole('button', { name: '甲' })
    await user.click(await screen.findByRole('button', { name: '知识编译' }))
    const dialog = await screen.findByRole('dialog', { name: '知识编译' })
    await user.click(within(dialog).getByRole('button', { name: '开始编译' }))
    expect(await within(dialog).findByRole('log', { name: '编译记录' })).toHaveAttribute(
      'data-ui',
      'knowledge.compile.agent'
    )
    expect(within(dialog).queryByText('这个库没有要编译的笔记。')).not.toBeInTheDocument()
    expect(screen.getByRole('dialog', { name: '知识编译' })).toBeInTheDocument()
  })

  it('shows a compile failure in the dialog and leaves start compile ready', async () => {
    renderNotes([{ name: '甲.md', bytes: encode('甲') }])
    const user = userEvent.setup()
    await user.click(await screen.findByRole('button', { name: '知识编译' }))
    const dialog = await screen.findByRole('dialog', { name: '知识编译' })
    await user.click(within(dialog).getByRole('button', { name: '开始编译' }))
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('知识编译没有完成。')
    expect(within(dialog).queryByRole('log', { name: '编译记录' })).not.toBeInTheDocument()
    expect(within(dialog).getByRole('button', { name: '开始编译' })).toBeEnabled()
    expect(screen.getByRole('button', { name: '知识编译' })).toBeEnabled()
    await user.click(within(dialog).getByRole('button', { name: '开始编译' }))
    await waitFor(() => expect(screen.getByRole('alert')).toHaveTextContent('知识编译没有完成。'))
    expect(within(dialog).getByRole('button', { name: '开始编译' })).toBeEnabled()
    expect(compileAgent.start).toHaveBeenCalledTimes(2)
  })

  it('keeps compiling after the dialog closes', async () => {
    let finishAccept: ((counts: { concepts: number; skipped: number; failed: number }) => void) | null = null
    const invoke = vi.fn(
      compileShell(['甲.md'], () => {
        return new Promise((resolve) => {
          finishAccept = resolve
        })
      })
    )
    Object.defineProperty(globalThis, '__TAURI_INTERNALS__', {
      configurable: true,
      value: { invoke }
    })
    compileAgent.start.mockResolvedValue({ sessionId: 'compile-session', agentId: 'work-agent' })
    const user = userEvent.setup()
    renderNotes([{ name: '甲.md', bytes: encode('甲') }])
    await user.click(await screen.findByRole('button', { name: '知识编译' }))
    const dialog = await screen.findByRole('dialog', { name: '知识编译' })
    await user.click(within(dialog).getByRole('button', { name: '开始编译' }))
    expect(await within(dialog).findByRole('log', { name: '编译记录' })).toBeInTheDocument()
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(screen.queryByRole('dialog', { name: '知识编译' })).not.toBeInTheDocument()
    expect(screen.queryByRole('log', { name: '编译记录' })).not.toBeInTheDocument()
    act(() => {
      cacheService.setShared(compileStatusKey(), {
        status: 'awaiting-approval',
        activeExecutions: [],
        awaitingApprovalAnchors: []
      })
    })
    expect(invoke.mock.calls.map((call) => call[0])).toEqual(['plan_personal_okf', 'pin_personal_okf'])
    act(() => {
      cacheService.setShared(compileStatusKey(), {
        status: 'done',
        activeExecutions: [],
        awaitingApprovalAnchors: [],
        lastCompletedAt: 1
      })
    })
    await waitFor(() =>
      expect(invoke.mock.calls.map((call) => call[0])).toEqual([
        'plan_personal_okf',
        'pin_personal_okf',
        'release_personal_okf',
        'remember_personal_okf',
        'accept_personal_okf'
      ])
    )
    await user.click(screen.getByRole('button', { name: '知识编译' }))
    const again = await screen.findByRole('dialog', { name: '知识编译' })
    expect(within(again).getByText(/正在核对这个库/)).toBeInTheDocument()
    expect(within(again).getByRole('log', { name: '编译记录' })).toBeInTheDocument()
    expect(within(again).queryByRole('button', { name: '关闭' })).not.toBeInTheDocument()
    await act(async () => {
      finishAccept?.({ concepts: 1, skipped: 0, failed: 0 })
    })
    expect(await within(again).findByRole('button', { name: '关闭' })).toBeInTheDocument()
    expect(invoke.mock.calls.map((call) => call[0])).not.toContain('compile_personal_okf')
  })

  it('reports a closed compile that failed and does not write the index', async () => {
    const invoke = vi.fn(compileShell(['甲.md'], () => Promise.reject(new Error('accept_personal_okf'))))
    Object.defineProperty(globalThis, '__TAURI_INTERNALS__', {
      configurable: true,
      value: { invoke }
    })
    compileAgent.start.mockResolvedValue({ sessionId: 'compile-session', agentId: 'work-agent' })
    const user = userEvent.setup()
    renderNotes([{ name: '甲.md', bytes: encode('甲') }])
    await user.click(await screen.findByRole('button', { name: '知识编译' }))
    const dialog = await screen.findByRole('dialog', { name: '知识编译' })
    await user.click(within(dialog).getByRole('button', { name: '开始编译' }))
    expect(await within(dialog).findByRole('log', { name: '编译记录' })).toBeInTheDocument()
    fireEvent.keyDown(document, { key: 'Escape' })
    expect(screen.queryByRole('dialog', { name: '知识编译' })).not.toBeInTheDocument()
    act(() => {
      cacheService.setShared(compileStatusKey(), {
        status: 'error',
        activeExecutions: [],
        awaitingApprovalAnchors: []
      })
    })
    await waitFor(() =>
      expect(invoke.mock.calls.map((call) => call[0])).toEqual([
        'plan_personal_okf',
        'pin_personal_okf',
        'release_personal_okf'
      ])
    )
    expect(invoke.mock.calls.map((call) => call[0])).not.toContain('remember_personal_okf')
    expect(invoke.mock.calls.map((call) => call[0])).not.toContain('accept_personal_okf')
    await user.click(screen.getByRole('button', { name: '知识编译' }))
    const again = await screen.findByRole('dialog', { name: '知识编译' })
    expect(await within(again).findByRole('alert')).toHaveTextContent('知识编译没有完成。')
    expect(within(again).getByRole('log', { name: '编译记录' })).toBeInTheDocument()
    expect(within(again).getByRole('button', { name: '开始编译' })).toBeEnabled()
  })

  it('draws saved links in the graph and opens the chosen note', async () => {
    renderNotes([
      { name: '甲.md', bytes: encode('见 [[乙]] 和 [[还没有]]') },
      { name: '乙.md', bytes: encode('乙的正文') },
      { name: '丙.md', bytes: encode('单独') }
    ])
    const user = userEvent.setup()
    await user.click(screen.getByRole('button', { name: '图谱' }))
    const graph = await screen.findByRole('navigation', { name: '图谱' })
    expect(await within(graph).findByRole('button', { name: '甲' })).toBeInTheDocument()
    expect(within(graph).getByRole('button', { name: '乙' })).toBeInTheDocument()
    expect(within(graph).queryByRole('button', { name: '丙' })).not.toBeInTheDocument()
    expect(within(graph).queryByRole('button', { name: '还没有' })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '放大' })).toBeInTheDocument()
    await waitFor(() => expect(graph.querySelector('line')).toBeInTheDocument())
    await user.click(screen.getByRole('button', { name: '显示孤立笔记' }))
    expect(await within(graph).findByRole('button', { name: '丙' })).toBeInTheDocument()
    expect(screen.getByText('整个库')).toBeInTheDocument()
    expect(screen.getByText('打开一篇笔记后，这里只显示它和相连的笔记。')).toBeInTheDocument()
    await user.click(within(graph).getByRole('button', { name: '乙' }))
    await waitFor(() =>
      expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', '乙的正文')
    )
    expect(screen.getByRole('heading', { level: 2, name: '乙' })).toBeInTheDocument()
    expect(screen.getByRole('navigation', { name: '图谱' })).toBeInTheDocument()
    expect(screen.getByText('当前笔记')).toBeInTheDocument()
    expect(screen.queryByRole('complementary', { name: '这篇笔记' })).not.toBeInTheDocument()
    const pane = screen.getByRole('navigation', { name: '图谱' }).parentElement?.parentElement
    expect(pane).toHaveClass('w-[42%]', 'min-w-[320px]')
    await user.click(screen.getByRole('button', { name: '图谱' }))
    expect(screen.queryByRole('navigation', { name: '图谱' })).not.toBeInTheDocument()
    expect(screen.getByRole('heading', { level: 2, name: '乙' })).toBeInTheDocument()
    expect(await screen.findByRole('complementary', { name: '这篇笔记' })).toBeInTheDocument()
    expect(document.querySelector('[data-editor="markdown"]')).toHaveAttribute('data-markdown', '乙的正文')
  })
})
