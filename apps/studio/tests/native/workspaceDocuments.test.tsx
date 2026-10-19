import { once } from 'node:events'
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { MessageChannel } from 'node:worker_threads'

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { DocumentOpenView } from '@renderer/pages/documents/DocumentOpenView'
import { localDocumentFromFile } from '@renderer/pages/documents/useLocalDocuments'
import { WorkbenchPluginHost } from '@renderer/services/plugins'
import { VsxDocumentSession, type VsxDocumentPort } from '@renderer/services/VsxDocumentSession'
import { WorkspaceDocumentClient } from '@renderer/services/WorkspaceDocumentService'
import type { VsxDocumentHostMessage, VsxDocumentPublication } from '@shared/types/vsxDocument'

import { NativeWorkspaceDocuments } from '../helpers/nativeWorkspaceDocuments'

let native: NativeWorkspaceDocuments
let root: string
let file: string
const clients: WorkspaceDocumentClient[] = []
const vsxHosts: WorkbenchPluginHost[] = []
const vsxPeers: (() => void)[] = []

beforeEach(async () => {
  root = await mkdtemp(path.join(tmpdir(), 'a3s-native-document-'))
  file = path.join(root, 'note.txt')
  await writeFile(file, 'disk text')
  native = new NativeWorkspaceDocuments()
  Object.defineProperty(globalThis, '__A3S_STUDIO_RUNTIME__', {
    configurable: true,
    value: { apiBaseUrl: 'http://127.0.0.1:9', token: 'ab'.repeat(32) }
  })
  vi.stubGlobal('fetch', native.fetch)
})

afterEach(async () => {
  cleanup()
  for (const host of vsxHosts.splice(0)) await host.close()
  for (const close of vsxPeers.splice(0)) close()
  vi.useRealTimers()
  for (const client of clients.splice(0)) await client.close()
  await waitFor(() => expect(native.activeLeases.size).toBe(0))
  await native.close()
  await rm(root, { recursive: true, force: true })
  vi.unstubAllGlobals()
  delete (globalThis as { __A3S_STUDIO_RUNTIME__?: unknown }).__A3S_STUDIO_RUNTIME__
})

async function open(): Promise<WorkspaceDocumentClient> {
  const client = await WorkspaceDocumentClient.open(root, file)
  clients.push(client)
  return client
}

async function pluginOwner() {
  const host = new WorkbenchPluginHost('tenant')
  await host.setPlugins([
    {
      id: 'test.document',
      generation: 1,
      scopes: ['window'],
      contributions: [{ id: 'read', labelKey: 'common.edit', slot: 'composer', invoke: (_context, text) => text }]
    }
  ])
  const scope = await host.openScope({ id: 'editor', kind: 'window' })
  return { host, context: scope.getSnapshot().contributions[0].context }
}

async function vsxSession(owner: WorkspaceDocumentClient) {
  const plugin = await pluginOwner()
  vsxHosts.push(plugin.host)
  const channel = new MessageChannel()
  vsxPeers.push(() => channel.port2.close())
  const replies: VsxDocumentHostMessage[] = []
  channel.port2.on('message', (message: VsxDocumentHostMessage) => replies.push(message))
  const first = once(channel.port2, 'message')
  const session = await VsxDocumentSession.connect(owner, plugin.context, channel.port1 as unknown as VsxDocumentPort)
  await first
  expect(replies[0]?.type).toBe('document')
  const document = replies[0]
  if (document.type !== 'document') throw new Error('Host document missing')
  const input: VsxDocumentPublication = {
    protocol: document.protocol,
    type: 'diagnostics',
    sessionId: document.sessionId,
    documentId: document.snapshot.documentId,
    sequence: 1,
    version: document.snapshot.version,
    items: [
      { start: { line: 0, character: 0 }, end: { line: 0, character: 4 }, severity: 'error', message: 'VSX finding' }
    ]
  }
  return { ...plugin, session, replies, input, port: channel.port2 }
}

describe('native-backed plugin diagnostic editor surface', () => {
  const editor = (name = 'note.txt', location = file) => (
    <DocumentOpenView
      document={localDocumentFromFile({ name, path: location, sizeBytes: 9, modifiedMs: 1, folder: '' })}
      workspaceRoot={root}
      cloudReady={false}
      frame="workspace"
    />
  )

  it('shows port findings as text, qualifies empty results and withdraws a retired publisher', async () => {
    render(editor())
    await screen.findByRole('textbox', { name: 'note.txt' }, { timeout: 3000 })
    const owner = await open()
    const host = await vsxSession(owner)
    const message = '<img src=x onerror=alert(1)> finding'
    host.port.postMessage({ ...host.input, items: [{ ...host.input.items[0], message }] })
    const panel = await screen.findByRole('region', { name: '诊断' }, { timeout: 3000 })
    expect(within(panel).getByText(message)).toBeVisible()
    expect(within(panel).getByText('错误')).toBeVisible()
    expect(within(panel).getByText('第 1 行，第 1 列')).toBeVisible()
    expect(within(panel).getByText('诊断还没有覆盖整个文件。')).toBeVisible()
    expect(within(panel).queryByRole('img')).not.toBeInTheDocument()
    expect(await readFile(file, 'utf8')).toBe('disk text')
    host.port.postMessage({ ...host.input, sequence: 2, items: [] })
    await waitFor(() => expect(within(panel).queryByText(message)).not.toBeInTheDocument(), { timeout: 3000 })
    expect(within(panel).getByText('0')).toBeVisible()
    expect(within(panel).getByText('诊断还没有覆盖整个文件。')).toBeVisible()
    await host.host.close()
    await waitFor(() => expect(screen.queryByRole('region', { name: '诊断' })).not.toBeInTheDocument(), {
      timeout: 3000
    })
  })

  it('clears old findings on an edit even when the user restores the original text', async () => {
    const user = userEvent.setup()
    render(editor())
    const box = await screen.findByRole('textbox', { name: 'note.txt' })
    const owner = await open()
    const host = await vsxSession(owner)
    host.port.postMessage(host.input)
    await screen.findByText('VSX finding', {}, { timeout: 3000 })
    await user.clear(box)
    expect(screen.queryByText('VSX finding')).not.toBeInTheDocument()
    await user.type(box, 'disk text')
    await waitFor(async () => {
      const observed = await owner.read()
      expect(observed.text).toBe('disk text')
      expect(observed.version).toBeGreaterThan(host.input.version)
    })
    await waitFor(async () => expect((await owner.diagnostics())?.diagnostics).toEqual([]))
    expect(screen.queryByText('VSX finding')).not.toBeInTheDocument()
  })

  it('discards a delayed old-document reply after edits and navigation without losing either draft', async () => {
    const user = userEvent.setup()
    const owner = await open()
    const host = await vsxSession(owner)
    host.port.postMessage(host.input)
    await waitFor(() => expect(host.replies.some((reply) => reply.type === 'accepted')).toBe(true))
    let release!: () => void
    const held = new Promise<void>((resolve) => {
      release = resolve
    })
    let reached!: () => void
    const requested = new Promise<void>((resolve) => {
      reached = resolve
    })
    let once = true
    vi.stubGlobal('fetch', async (...args: Parameters<typeof globalThis.fetch>) => {
      const [input, init] = args
      const response = await native.fetch(input, init)
      if (
        once &&
        new globalThis.URL(String(input)).pathname === '/api/workspace-documents' &&
        JSON.parse(String(init?.body)).operation === 'diagnostics'
      ) {
        once = false
        reached()
        await held
      }
      return response
    })
    const view = render(editor())
    try {
      const box = await screen.findByRole('textbox', { name: 'note.txt' })
      await requested
      await user.clear(box)
      await user.type(box, 'local draft')
      const next = path.join(root, 'next.txt')
      await writeFile(next, 'next text')
      view.rerender(editor('next.txt', next))
      const nextBox = await screen.findByRole('textbox', { name: 'next.txt' })
      expect(nextBox).toHaveValue('next text')
      release()
      await waitFor(async () => expect((await owner.read()).text).toBe('local draft'))
      await user.clear(nextBox)
      await user.type(nextBox, 'second draft')
      await user.keyboard('{Meta>}s{/Meta}')
      await waitFor(async () => expect(await readFile(next, 'utf8')).toBe('second draft'))
      expect(screen.queryByText('VSX finding')).not.toBeInTheDocument()
      expect(await readFile(file, 'utf8')).toBe('disk text')
    } finally {
      release()
    }
  })

  it.each(['unavailable', 'malformed'])('keeps editing and saving usable when diagnostics are %s', async (fault) => {
    const user = userEvent.setup()
    vi.stubGlobal('fetch', async (...args: Parameters<typeof globalThis.fetch>) => {
      const [input, init] = args
      const response = await native.fetch(input, init)
      if (
        new globalThis.URL(String(input)).pathname === '/api/workspace-documents' &&
        JSON.parse(String(init?.body)).operation === 'diagnostics'
      )
        return new globalThis.Response(fault === 'malformed' ? '{}' : '', { status: fault === 'malformed' ? 200 : 503 })
      return response
    })
    render(editor())
    const box = await screen.findByRole('textbox', { name: 'note.txt' })
    await screen.findByText('诊断没有完成。', {}, { timeout: 3000 })
    await user.clear(box)
    await user.type(box, 'saved despite diagnostic failure')
    await user.keyboard('{Meta>}s{/Meta}')
    await waitFor(async () => expect(await readFile(file, 'utf8')).toBe('saved despite diagnostic failure'))
  })
})

describe('native-backed compatible host document ports', () => {
  it('closes the transferred port when its plugin owner cannot admit a document grant', async () => {
    const owner = await open()
    const plugin = await pluginOwner()
    await plugin.host.close()
    const channel = new MessageChannel()
    vsxPeers.push(() => channel.port2.close())
    let peerClosed = false
    channel.port2.on('close', () => {
      peerClosed = true
    })
    await expect(
      VsxDocumentSession.connect(owner, plugin.context, channel.port1 as unknown as VsxDocumentPort)
    ).rejects.toThrow('closed')
    await waitFor(() => expect(peerClosed).toBe(true))
    expect(native.activeLeases.size).toBe(1)
    expect(owner.observation.text).toBe('disk text')
  })

  it('passes unsaved text without a bearer and acknowledges a native diagnostic publication', async () => {
    const owner = await open()
    await owner.replace('unsaved text')
    const host = await vsxSession(owner)
    const packet = host.replies[0]
    expect(packet.type === 'document' && packet.snapshot.text).toBe('unsaved text')
    expect(JSON.stringify(packet)).not.toContain('ab'.repeat(32))
    expect(Object.keys(packet).sort()).toEqual(['protocol', 'sessionId', 'snapshot', 'type'])
    host.port.postMessage(host.input)
    await waitFor(() => expect(host.replies.some((reply) => reply.type === 'accepted')).toBe(true))
    const observation = await native.command({ operation: 'diagnostics', lease: [...native.activeLeases][0] })
    expect(
      (observation.value as { diagnostics: { version: number; items: { message: string }[] }[] }).diagnostics
    ).toEqual([
      expect.objectContaining({ version: 2, sequence: 1, items: [expect.objectContaining({ message: 'VSX finding' })] })
    ])
    expect(await readFile(file, 'utf8')).toBe('disk text')
    await host.host.close()
    host.port.close()
  })

  it('refuses forged sessions, command fields and changed retries without replaying native publication', async () => {
    const owner = await open()
    const host = await vsxSession(owner)
    for (const input of [
      { ...host.input, sessionId: 'foreign' },
      { ...host.input, command: 'save' },
      { ...host.input, sequence: 9 }
    ]) {
      const before = host.replies.length
      host.port.postMessage(input)
      await waitFor(() =>
        expect(host.replies.slice(before)).toContainEqual(
          expect.objectContaining({ type: 'refused', reason: 'invalid' })
        )
      )
    }
    host.port.postMessage(host.input)
    await waitFor(() => expect(host.replies.filter((reply) => reply.type === 'accepted')).toHaveLength(1))
    host.port.postMessage({
      ...host.input,
      items: host.input.items.map((item) => ({
        message: item.message,
        severity: item.severity,
        end: item.end,
        start: item.start
      }))
    })
    await waitFor(() => expect(host.replies.filter((reply) => reply.type === 'accepted')).toHaveLength(2))
    host.port.postMessage({ ...host.input, items: [] })
    await waitFor(() =>
      expect(host.replies.at(-1)).toEqual(expect.objectContaining({ type: 'refused', reason: 'invalid' }))
    )
    const observation = await native.command({ operation: 'diagnostics', lease: [...native.activeLeases][0] })
    expect((observation.value as { diagnostics: { sequence: number }[] }).diagnostics[0].sequence).toBe(1)
    await host.host.close()
    host.port.close()
  })

  it('refuses stale and foreign document replies and mirrors the new native version', async () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval'] })
    const owner = await open()
    const host = await vsxSession(owner)
    const exchange = (input: unknown) => {
      const reply = once(host.port, 'message')
      host.port.postMessage(input)
      return reply.then(([message]) => message as VsxDocumentHostMessage)
    }
    await owner.replace('changed text')
    await owner.replace('disk text')
    expect(await exchange(host.input)).toMatchObject({ type: 'refused', reason: 'stale' })
    expect(await exchange({ ...host.input, version: 3 })).toMatchObject({ type: 'refused', reason: 'stale' })
    const mirrored = once(host.port, 'message')
    await host.session.sync()
    expect((await mirrored)[0]).toMatchObject({ type: 'document', snapshot: { version: 3, text: 'disk text' } })
    expect(await exchange({ ...host.input, version: 3, documentId: 'foreign-document' })).toMatchObject({
      type: 'refused',
      reason: 'stale'
    })
    expect(await exchange({ ...host.input, version: 3 })).toMatchObject({ type: 'accepted', version: 3 })
    await host.host.close()
    host.port.close()
  })

  it('withdraws host publications on acknowledged owner close and preserves the independent editor', async () => {
    const owner = await open()
    const host = await vsxSession(owner)
    host.port.postMessage(host.input)
    await waitFor(() => expect(host.replies.some((reply) => reply.type === 'accepted')).toBe(true))
    expect(await host.host.close()).toEqual({ failed: [], timedOut: [] })
    expect(native.activeLeases.size).toBe(1)
    const reply = await native.command({ operation: 'diagnostics', lease: [...native.activeLeases][0] })
    expect((reply.value as { diagnostics: unknown[] }).diagnostics).toEqual([])
    await expect(host.session.sync()).rejects.toThrow('closed')
    await owner.replace('editor still works')
    expect(owner.observation.text).toBe('editor still works')
    host.port.close()
  })

  it('bounds admitted host work and refuses another publication while native acknowledgement is pending', async () => {
    const owner = await open()
    const host = await vsxSession(owner)
    let release!: () => void
    let admitted!: () => void
    const waiting = new Promise<void>((resolve) => {
      release = resolve
    })
    const entered = new Promise<void>((resolve) => {
      admitted = resolve
    })
    vi.stubGlobal('fetch', async (input: RequestInfo | URL, init?: RequestInit) => {
      const reply = await native.fetch(input, init)
      if (JSON.parse(String(init?.body)).operation === 'publish_diagnostics') {
        admitted()
        await waiting
      }
      return reply
    })
    host.port.postMessage(host.input)
    await entered
    try {
      host.port.postMessage({ ...host.input, sequence: 2, items: [] })
      await waitFor(() =>
        expect(host.replies.at(-1)).toEqual(expect.objectContaining({ type: 'refused', reason: 'busy' }))
      )
    } finally {
      release()
    }
    await waitFor(() => expect(host.replies.at(-1)).toEqual(expect.objectContaining({ type: 'accepted', sequence: 1 })))
    expect(await host.host.close()).toEqual({ failed: [], timedOut: [] })
    host.port.close()
  })
})

describe('actual native shared documents', () => {
  it('requires a distinct diagnostic grant and releases its publications with actual plugin cleanup', async () => {
    const owner = await open()
    await owner.replace('unsaved error')
    const plugin = await pluginOwner()
    const readonly = await owner.delegate(plugin.context, { edit: false, save: false })
    const diagnostic = await owner.delegateDiagnostics(plugin.context)
    const items = [
      {
        start: { line: 0, character: 0 },
        end: { line: 0, character: 7 },
        severity: 'error' as const,
        message: 'unsaved finding'
      }
    ]
    await expect(readonly.publishDiagnostics(readonly.observation, items)).rejects.toThrow('refused')
    const published = await diagnostic.publishDiagnostics(diagnostic.observation, items)
    expect(published.sequence).toBe(1)
    const observation = await readonly.diagnostics()
    expect(observation.snapshot.text).toBe('unsaved error')
    expect(observation.diagnostics[0].version).toBe(observation.snapshot.version)
    expect(observation.diagnostics[0].items[0].message).toBe('unsaved finding')
    await expect(diagnostic.apply(diagnostic.observation, 'denied')).rejects.toThrow('refused')
    await expect(diagnostic.save()).rejects.toThrow('refused')
    expect(await readFile(file, 'utf8')).toBe('disk text')
    expect(await plugin.host.close()).toEqual({ failed: [], timedOut: [] })
    await expect(diagnostic.publishDiagnostics(diagnostic.observation, items)).rejects.toThrow('owner closed')
    const after = await native.command({ operation: 'diagnostics', lease: [...native.activeLeases][0] })
    expect(after.status).toBe(200)
    expect((after.value as { diagnostics: unknown[] }).diagnostics).toEqual([])
    expect(native.activeLeases.size).toBe(1)
  })

  it('refuses late diagnostics after peer edits and never revives them when text returns', async () => {
    const owner = await open()
    const plugin = await pluginOwner()
    const publisher = await owner.delegateDiagnostics(plugin.context)
    const original = publisher.observation
    const items = [
      {
        start: { line: 0, character: 0 },
        end: { line: 0, character: 4 },
        severity: 'warning' as const,
        message: 'original observation'
      }
    ]
    await publisher.publishDiagnostics(original, items)
    await owner.replace('peer text')
    await expect(publisher.publishDiagnostics(original, items)).rejects.toThrow('(409)')
    expect((await publisher.diagnostics()).diagnostics).toEqual([])
    await owner.replace('disk text')
    expect((await publisher.diagnostics()).diagnostics).toEqual([])
    const next = await publisher.publishDiagnostics(publisher.observation, items)
    expect(next.sequence).toBe(2)
    expect(next.version).toBe(3)
    expect(await plugin.host.close()).toEqual({ failed: [], timedOut: [] })
  })

  it('does not mix diagnostics between workspaces with matching file names and text', async () => {
    const owner = await open()
    const secondRoot = path.join(root, 'second-workspace')
    await mkdir(secondRoot)
    await writeFile(path.join(secondRoot, 'note.txt'), 'disk text')
    const peer = await WorkspaceDocumentClient.open(secondRoot, path.join(secondRoot, 'note.txt'))
    clients.push(peer)
    const plugin = await pluginOwner()
    const publisher = await owner.delegateDiagnostics(plugin.context)
    const reader = await peer.delegate(plugin.context, { edit: false, save: false })
    await publisher.publishDiagnostics(publisher.observation, [
      {
        start: { line: 0, character: 0 },
        end: { line: 0, character: 1 },
        severity: 'error',
        message: 'first workspace'
      }
    ])
    const observation = await reader.diagnostics()
    expect(observation.snapshot.documentId).not.toBe(publisher.observation.documentId)
    expect(observation.diagnostics).toEqual([])
    expect(await plugin.host.close()).toEqual({ failed: [], timedOut: [] })
  })

  it('fences a publication reply that arrives after its actual plugin owner closes', async () => {
    const owner = await open()
    const plugin = await pluginOwner()
    const publisher = await owner.delegateDiagnostics(plugin.context)
    let release!: () => void
    let admitted!: () => void
    const waiting = new Promise<void>((resolve) => {
      release = resolve
    })
    const entered = new Promise<void>((resolve) => {
      admitted = resolve
    })
    vi.stubGlobal('fetch', async (input: RequestInfo | URL, init?: RequestInit) => {
      const reply = await native.fetch(input, init)
      if (JSON.parse(String(init?.body)).operation === 'publish_diagnostics') {
        admitted()
        await waiting
      }
      return reply
    })
    const work = publisher.publishDiagnostics(publisher.observation, [
      { start: { line: 0, character: 0 }, end: { line: 0, character: 1 }, severity: 'error', message: 'late result' }
    ])
    const refused = expect(work).rejects.toThrow('not acknowledged')
    await entered
    const closing = plugin.host.close()
    release()
    await refused
    expect(await closing).toEqual({ failed: [], timedOut: [] })
    expect(native.activeLeases.size).toBe(1)
    const after = await native.command({ operation: 'diagnostics', lease: [...native.activeLeases][0] })
    expect((after.value as { diagnostics: unknown[] }).diagnostics).toEqual([])
  })

  it('owns a readonly native document capability through the actual plugin host', async () => {
    const owner = await open()
    await owner.replace('shared unsaved')
    const plugin = await pluginOwner()
    const headers: string[] = []
    vi.stubGlobal('fetch', async (input: RequestInfo | URL, init?: RequestInit) => {
      if (String(input).endsWith('/api/plugin-documents'))
        headers.push(new Headers(init?.headers).get('Authorization') ?? '')
      return native.fetch(input, init)
    })
    const document = await owner.delegate(plugin.context, { edit: false, save: false })
    expect((await document.read()).text).toBe('shared unsaved')
    await expect(document.apply(document.observation, 'denied')).rejects.toThrow('refused')
    await expect(document.save()).rejects.toThrow('refused')
    expect(await readFile(file, 'utf8')).toBe('disk text')
    expect((await owner.read()).text).toBe('shared unsaved')
    expect(await plugin.host.close()).toEqual({ failed: [], timedOut: [] })
    await expect(document.read()).rejects.toThrow('owner closed')
    expect(native.activeLeases.size).toBe(1)
    expect(headers.length).toBeGreaterThan(0)
    expect(headers.every((header) => header.startsWith('Bearer ') && header !== `Bearer ${'ab'.repeat(32)}`)).toBe(true)
  })

  it('version-checks delegated edits and revokes them when the editor lease closes', async () => {
    const owner = await open()
    const plugin = await pluginOwner()
    const document = await owner.delegate(plugin.context, { edit: true, save: false })
    const before = document.observation
    await owner.replace('newer user input')
    await expect(document.apply(before, 'obsolete fix')).rejects.toThrow('(409)')
    const current = await document.read()
    await document.apply(current, 'accepted fix')
    expect((await owner.read()).text).toBe('accepted fix')
    expect(await readFile(file, 'utf8')).toBe('disk text')
    await expect(document.save()).rejects.toThrow('refused')
    await owner.close()
    await expect(document.read()).rejects.toThrow('(404)')
    expect(await plugin.host.close()).toEqual({ failed: [], timedOut: [] })
    const replacement = await open()
    expect(replacement.observation.text).toBe('accepted fix')
    await replacement.save()
    expect(await readFile(file, 'utf8')).toBe('accepted fix')
  })

  it('releases a native grant whose response arrives after the plugin scope closed', async () => {
    const owner = await open()
    const plugin = await pluginOwner()
    let release!: () => void
    let started!: () => void
    const gate = new Promise<void>((resolve) => {
      release = resolve
    })
    const issued = new Promise<void>((resolve) => {
      started = resolve
    })
    vi.stubGlobal('fetch', async (input: RequestInfo | URL, init?: RequestInit) => {
      const reply = await native.fetch(input, init)
      if (String(init?.body).includes('"operation":"delegate"')) {
        started()
        await gate
      }
      return reply
    })
    const pending = owner.delegate(plugin.context, { edit: false, save: false })
    const refused = expect(pending).rejects.toThrow('owner closed')
    await issued
    expect(native.activeLeases.size).toBe(2)
    await plugin.host.close()
    release()
    await refused
    expect(native.activeLeases.size).toBe(1)
    expect((await owner.read()).text).toBe('disk text')
  })

  it('disables foreign recovery while its creator is live and enables it after close', async () => {
    const user = await open()
    const peer = await open()
    await peer.replace('peer')
    await expect(user.replace('retained draft')).rejects.toThrow('retained')
    render(
      <DocumentOpenView
        document={localDocumentFromFile({ name: 'note.txt', path: file, sizeBytes: 9, modifiedMs: 1, folder: '' })}
        workspaceRoot={root}
        cloudReady={false}
        frame="workspace"
      />
    )
    const box = await screen.findByRole('textbox', { name: 'note.txt' })
    fireEvent.click(await screen.findByRole('button', { name: '查看草稿 1' }))
    const dialog = await screen.findByRole('dialog', { name: '恢复未保存草稿' })
    expect(within(dialog).getByRole('button', { name: '恢复' })).toBeDisabled()
    expect(within(dialog).getByRole('button', { name: '删除' })).toBeDisabled()
    await user.close()
    fireEvent.click(within(dialog).getByRole('button', { name: '重新加载文件' }))
    await waitFor(() => expect(within(dialog).getByRole('button', { name: '删除' })).toBeEnabled())
    fireEvent.click(within(dialog).getByRole('button', { name: '删除' }))
    await waitFor(() => expect(screen.queryByRole('button', { name: '查看草稿 1' })).toBeNull())
    expect(box).toHaveValue('peer')
    expect(await readFile(file, 'utf8')).toBe('disk text')
  })

  it('retains the latest refused input through close without automatically applying it', async () => {
    const user = await open()
    const peer = await open()
    await peer.replace('accepted peer')
    await peer.save()
    await expect(user.replace('first refused')).rejects.toThrow()
    await expect(user.replace('latest refused')).rejects.toThrow()
    await user.close()
    const reopened = await open()
    expect(reopened.observation.text).toBe('accepted peer')
    const wire = await native.command({ operation: 'recoveries', lease: [...native.activeLeases].at(-1) })
    expect(wire.value).toHaveLength(1)
    const drafts = await reopened.recoveries()
    expect(drafts).toHaveLength(1)
    const review = await reopened.reviewRecovery(drafts[0].id)
    expect(review.text).toBe('latest refused')
    await reopened.resolveRecovery(review, true)
    expect(reopened.text).toBe('latest refused')
    expect(await readFile(file, 'utf8')).toBe('accepted peer')
    await reopened.save()
    expect(await readFile(file, 'utf8')).toBe('latest refused')
  })

  it('refuses a recovery reviewed before another peer edit and keeps its draft', async () => {
    const user = await open()
    const peer = await open()
    await peer.replace('peer')
    await expect(user.replace('refused')).rejects.toThrow('retained')
    await user.close()
    const drafts = await peer.recoveries()
    const review = await peer.reviewRecovery(drafts[0].id)
    await peer.replace('newer peer')
    await expect(peer.resolveRecovery(review, true)).rejects.toThrow('stale')
    expect(peer.text).toBe('newer peer')
    expect((await peer.reviewRecovery(drafts[0].id)).text).toBe('refused')
    expect(await readFile(file, 'utf8')).toBe('disk text')
  })

  it('drops a reviewed recovery while preserving accepted shared and disk content', async () => {
    const user = await open()
    const peer = await open()
    await peer.replace('peer')
    await expect(user.replace('refused')).rejects.toThrow('retained')
    await user.close()
    const drafts = await peer.recoveries()
    await peer.resolveRecovery(await peer.reviewRecovery(drafts[0].id), false)
    expect(await peer.recoveries()).toHaveLength(0)
    expect(peer.text).toBe('peer')
    expect(await readFile(file, 'utf8')).toBe('disk text')
  })

  it('reopens a refused editor draft for explicit review and recovery', async () => {
    const props = {
      document: localDocumentFromFile({ name: 'note.txt', path: file, sizeBytes: 9, modifiedMs: 1, folder: '' }),
      workspaceRoot: root,
      cloudReady: false,
      frame: 'workspace' as const
    }
    const view = render(<DocumentOpenView {...props} />)
    const box = await screen.findByRole('textbox', { name: 'note.txt' })
    const peer = await open()
    await peer.replace('accepted peer')
    fireEvent.change(box, { target: { value: 'refused editor draft' } })
    await screen.findByRole('button', { name: '查看草稿 1' })
    view.unmount()
    await waitFor(() => expect(native.activeLeases.size).toBe(1))
    render(<DocumentOpenView {...props} />)
    const reopened = await screen.findByRole('textbox', { name: 'note.txt' })
    expect(reopened).toHaveValue('accepted peer')
    fireEvent.click(await screen.findByRole('button', { name: '查看草稿 1' }))
    const dialog = await screen.findByRole('dialog', { name: '恢复未保存草稿' })
    expect(within(dialog).getByRole('textbox', { name: '当前共享内容' })).toHaveValue('accepted peer')
    expect(within(dialog).getByRole('textbox', { name: '保留的草稿' })).toHaveValue('refused editor draft')
    fireEvent.click(within(dialog).getByRole('button', { name: '恢复' }))
    await waitFor(() => expect(reopened).toHaveValue('refused editor draft'))
    expect(await readFile(file, 'utf8')).toBe('disk text')
    fireEvent.keyDown(reopened, { key: 's', metaKey: true })
    await waitFor(async () => expect(await readFile(file, 'utf8')).toBe('refused editor draft'))
  })

  it('shows a stale recovery refusal in its dialog and allows a fresh review', async () => {
    const user = await open()
    const peer = await open()
    await peer.replace('peer')
    await expect(user.replace('retained draft')).rejects.toThrow('retained')
    await user.close()
    render(
      <DocumentOpenView
        document={localDocumentFromFile({ name: 'note.txt', path: file, sizeBytes: 9, modifiedMs: 1, folder: '' })}
        workspaceRoot={root}
        cloudReady={false}
        frame="workspace"
      />
    )
    const box = await screen.findByRole('textbox', { name: 'note.txt' })
    fireEvent.click(await screen.findByRole('button', { name: '查看草稿 1' }))
    const dialog = await screen.findByRole('dialog', { name: '恢复未保存草稿' })
    await peer.replace('newer peer')
    fireEvent.click(within(dialog).getByRole('button', { name: '恢复' }))
    await within(dialog).findByText('内容或草稿已变化，请重新查看后再恢复。')
    expect((await peer.read()).text).toBe('newer peer')
    expect(await readFile(file, 'utf8')).toBe('disk text')
    fireEvent.click(within(dialog).getByRole('button', { name: '重新加载文件' }))
    await waitFor(() => expect(within(dialog).getByRole('textbox', { name: '当前共享内容' })).toHaveValue('newer peer'))
    fireEvent.click(within(dialog).getByRole('button', { name: '恢复' }))
    await waitFor(() => expect(box).toHaveValue('retained draft'))
    expect(await readFile(file, 'utf8')).toBe('disk text')
  })

  it('shares unsaved text across clients and saves one accepted observation', async () => {
    const first = await open()
    const second = await open()
    await first.replace('unsaved draft')
    const observed = await second.read()
    expect(observed.text).toBe('unsaved draft')
    expect(observed.dirty).toBe(true)
    expect(await readFile(file, 'utf8')).toBe('disk text')
    await second.apply(observed, 'fixed draft')
    await second.save()
    expect(await readFile(file, 'utf8')).toBe('fixed draft')
    expect((await first.read()).dirty).toBe(false)
  })

  it('refuses a stale provider edit after user changes and restores the same text', async () => {
    const client = await open()
    const observed = await client.read()
    const first = client.replace('temporary')
    const restored = client.replace('disk text')
    const stale = client.apply(observed, 'stale result')
    await expect(stale).rejects.toThrow('stale')
    await first
    await restored
    expect((await client.read()).text).toBe('disk text')
    expect(await readFile(file, 'utf8')).toBe('disk text')
  })

  it('refuses an edit based on a different client’s obsolete model version', async () => {
    const first = await open()
    const second = await open()
    const old = second.observation
    await first.replace('newer writer')
    await expect(second.apply(old, 'old provider')).rejects.toThrow('409')
    expect((await first.read()).text).toBe('newer writer')
    expect(await readFile(file, 'utf8')).toBe('disk text')
  })

  it('refuses an explicit discard based on an obsolete peer observation', async () => {
    const first = await open()
    const peer = await open()
    await first.replace('newer unsaved text')
    await expect(peer.discard()).rejects.toThrow('409')
    expect((await first.read()).text).toBe('newer unsaved text')
    expect(await readFile(file, 'utf8')).toBe('disk text')
  })

  it('drains admitted user drafts, cancels an unstarted save and acknowledges close', async () => {
    const client = await open()
    const draft = client.replace('retained user draft')
    const save = client.save()
    const refused = expect(save).rejects.toThrow('closed')
    const closed = client.close()
    await draft
    await refused
    await closed
    const next = await open()
    expect(next.observation.text).toBe('retained user draft')
    expect(await readFile(file, 'utf8')).toBe('disk text')
    await expect(client.replace('late')).rejects.toThrow('closed')
  })

  it('keeps external bytes on conflict and recovers only by explicit discard', async () => {
    const client = await open()
    await client.replace('local draft')
    await writeFile(file, 'external change')
    await expect(client.save()).rejects.toThrow('409')
    expect(client.text).toBe('local draft')
    expect(await readFile(file, 'utf8')).toBe('external change')
    const discarded = await client.discard()
    expect(discarded.text).toBe('external change')
    await client.replace('recomputed draft')
    await client.save()
    expect(await readFile(file, 'utf8')).toBe('recomputed draft')
  })

  it('preserves user input admitted while an explicit discard response is in flight', async () => {
    const client = await open()
    await client.replace('old draft')
    await writeFile(file, 'external')
    const admitted = Promise.withResolvers<void>()
    const release = Promise.withResolvers<void>()
    vi.stubGlobal('fetch', async (input: RequestInfo | URL, init?: RequestInit) => {
      const result = await native.fetch(input, init)
      if (init?.body && JSON.parse(String(init.body)).operation === 'discard') {
        admitted.resolve()
        await release.promise
      }
      return result
    })
    const discard = client.discard()
    await admitted.promise
    const input = client.replace('typed during reload')
    release.resolve()
    await discard
    await input
    expect((await client.read()).text).toBe('typed during reload')
    expect(await readFile(file, 'utf8')).toBe('external')
  })

  it('observes a peer’s disk conflict even when the text version stays unchanged', async () => {
    const first = await open()
    await first.replace('draft')
    const peer = await open()
    const version = peer.observation.version
    await writeFile(file, 'external')
    await expect(first.save()).rejects.toThrow('409')
    const changed = await peer.observe()
    expect(changed?.version).toBe(version)
    expect(changed?.diskConflict).toBe(true)
    expect(changed?.text).toBe('draft')
    expect(await peer.observe()).toBeUndefined()
  })

  it('offers explicit discard after an editor save conflict and saves a recomputed draft', async () => {
    render(
      <DocumentOpenView
        document={localDocumentFromFile({ name: 'note.txt', path: file, sizeBytes: 9, modifiedMs: 1, folder: '' })}
        workspaceRoot={root}
        cloudReady={false}
        frame="workspace"
      />
    )
    const box = await screen.findByRole('textbox', { name: 'note.txt' })
    fireEvent.change(box, { target: { value: 'local draft' } })
    await writeFile(file, 'external change')
    fireEvent.keyDown(box, { key: 's', metaKey: true })
    fireEvent.click(await screen.findByRole('button', { name: '放弃修改' }))
    await waitFor(() => expect(box).toHaveValue('external change'))
    expect(await readFile(file, 'utf8')).toBe('external change')
    fireEvent.change(box, { target: { value: 'recomputed text' } })
    fireEvent.keyDown(box, { key: 's', metaKey: true })
    await waitFor(async () => expect(await readFile(file, 'utf8')).toBe('recomputed text'))
  })

  it('publishes the actual editor’s unsaved input for another native consumer', async () => {
    const view = render(
      <DocumentOpenView
        document={localDocumentFromFile({ name: 'note.txt', path: file, sizeBytes: 9, modifiedMs: 1, folder: '' })}
        workspaceRoot={root}
        cloudReady={false}
        frame="workspace"
      />
    )
    const box = await screen.findByRole('textbox', { name: 'note.txt' })
    const consumer = await open()
    fireEvent.change(box, { target: { value: 'editor unsaved text' } })
    await waitFor(async () => expect((await consumer.read()).text).toBe('editor unsaved text'))
    expect(await readFile(file, 'utf8')).toBe('disk text')
    fireEvent.keyDown(box, { key: 's', metaKey: true })
    await waitFor(async () => expect(await readFile(file, 'utf8')).toBe('editor unsaved text'))
    view.unmount()
    await act(async () => {
      await new Promise((resolve) => globalThis.setTimeout(resolve, 0))
    })
    expect((await consumer.read()).text).toBe('editor unsaved text')
  })

  it('shows an accepted native consumer fix in the actual open editor', async () => {
    render(
      <DocumentOpenView
        document={localDocumentFromFile({ name: 'note.txt', path: file, sizeBytes: 9, modifiedMs: 1, folder: '' })}
        workspaceRoot={root}
        cloudReady={false}
        frame="workspace"
      />
    )
    const box = await screen.findByRole('textbox', { name: 'note.txt' })
    const consumer = await open()
    await consumer.apply(consumer.observation, 'provider fix')
    await waitFor(() => expect(box).toHaveValue('provider fix'), { timeout: 3000 })
    expect(await readFile(file, 'utf8')).toBe('disk text')
    fireEvent.keyDown(box, { key: 's', metaKey: true })
    await waitFor(async () => expect(await readFile(file, 'utf8')).toBe('provider fix'))
  })
})
