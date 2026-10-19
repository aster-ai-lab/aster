import { MessageChannel as NodeMessageChannel } from 'node:worker_threads'

import { waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AIX_TREE_DEPTH_LIMIT, AIX_TREE_NODE_LIMIT, type AixTreeNode } from '@shared/types/aixTree'

import { AixTreeProvider } from '../aixTreeProvider'
import { WorkbenchPluginHost } from '../workbenchPluginHost'

const root: AixTreeNode = { id: 'root', parent: null, label: 'Root', hasChildren: true }
const hosts: WorkbenchPluginHost[] = []
const peers: MessagePort[] = []

async function provider(timeout = 5000) {
  vi.stubGlobal('MessageChannel', NodeMessageChannel)
  const host = new WorkbenchPluginHost('tree-member')
  hosts.push(host)
  let tree!: AixTreeProvider
  await host.setPlugins([
    {
      id: 'tree',
      generation: 1,
      scopes: ['window'],
      contributions: [],
      activate: (context) => {
        tree = new AixTreeProvider(context, 'owned-tree', timeout)
        context.defer(() => tree.close())
      }
    }
  ])
  await host.openScope({ id: 'main', kind: 'window' })
  const channel = new MessageChannel()
  const peer = channel.port2
  peers.push(peer)
  peer.start()
  const next = () =>
    new Promise<any>((resolve) => peer.addEventListener('message', ({ data }) => resolve(data), { once: true }))
  const greeting = next()
  tree.connect(channel.port1)
  expect(await greeting).toEqual({ protocolVersion: 1, type: 'aix.tree.connect', surface: 'owned-tree' })
  const initial = next()
  peer.postMessage({ protocolVersion: 1, type: 'aix.tree.ready' })
  expect(await initial).toEqual({
    protocolVersion: 1,
    type: 'aix.tree.request',
    id: 1,
    action: 'refresh',
    nodeId: null
  })
  const reply = async (id: number, nodes: AixTreeNode[]) => {
    peer.postMessage({ protocolVersion: 1, type: 'aix.tree.data', id, nodes })
    await waitFor(() => expect(tree.getSnapshot().busy).toBe(false))
  }
  return { host, tree, peer, next, reply }
}

afterEach(async () => {
  await Promise.all(hosts.splice(0).map((host) => host.close()))
  for (const peer of peers.splice(0)) peer.close()
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('isolated AIX tree provider', () => {
  // Catch provider data bypassing owner-bound action admission or becoming a global host command.
  it('serializes refresh/expand/select/invoke and admits only current nodes and their own commands', async () => {
    const p = await provider()
    expect(p.tree.request('expand', 'root')).toBe(false)
    await p.reply(1, [root])
    expect(p.tree.request('invoke', 'root')).toBe(false)
    expect(p.tree.request('select', 'unknown')).toBe(false)
    const expanded = p.next()
    expect(p.tree.request('expand', 'root')).toBe(true)
    expect(p.tree.request('refresh', null)).toBe(false)
    expect(await expanded).toEqual({
      protocolVersion: 1,
      type: 'aix.tree.request',
      id: 2,
      action: 'expand',
      nodeId: 'root'
    })
    const leaf: AixTreeNode = { id: 'leaf', parent: 'root', label: 'Leaf', hasChildren: false, command: 'open' }
    await p.reply(2, [root, leaf])
    expect(p.tree.request('expand', 'leaf')).toBe(false)
    for (const [action, id] of [
      ['select', 3],
      ['invoke', 4],
      ['collapse', 5],
      ['refresh', 6]
    ] as const) {
      const message = p.next()
      const nodeId = action === 'refresh' ? null : action === 'collapse' ? 'root' : 'leaf'
      expect(p.tree.request(action, nodeId)).toBe(true)
      expect(await message).toEqual({
        protocolVersion: 1,
        type: 'aix.tree.request',
        id,
        action,
        nodeId,
        ...(action === 'invoke' ? { command: 'open' } : {})
      })
      await p.reply(id, [root, leaf])
    }
    await p.host.close()
    expect(p.tree.request('invoke', 'leaf')).toBe(false)
    expect(p.tree.getSnapshot()).toEqual({ status: 'closed', busy: false, nodes: [] })
  })

  // Catch invalidation dropping behind an outstanding response or creating unbounded concurrent work.
  it('coalesces provider invalidation behind its outstanding request and refuses replayed invalidation', async () => {
    const p = await provider()
    const refreshed = p.next()
    p.peer.postMessage({ protocolVersion: 1, type: 'aix.tree.changed', revision: 1 })
    p.peer.postMessage({ protocolVersion: 1, type: 'aix.tree.changed', revision: 2 })
    p.peer.postMessage({ protocolVersion: 1, type: 'aix.tree.data', id: 1, nodes: [root] })
    expect(await refreshed).toEqual({
      protocolVersion: 1,
      type: 'aix.tree.request',
      id: 2,
      action: 'refresh',
      nodeId: null
    })
    await p.reply(2, [root])
    p.peer.postMessage({ protocolVersion: 1, type: 'aix.tree.changed', revision: 2 })
    await waitFor(() => expect(p.tree.getSnapshot()).toEqual({ status: 'failed', busy: false, nodes: [] }))
  })

  // Catch a forged, cyclic, unbounded or unmatched forest entering the native tree renderer.
  it.each([
    ['duplicate IDs', [root, root]],
    ['unknown parent', [{ ...root, parent: 'missing' }]],
    ['self cycle', [{ ...root, parent: 'root' }]],
    [
      'cycle',
      [
        { ...root, parent: 'other' },
        { ...root, id: 'other', parent: 'root' }
      ]
    ],
    [
      'leaf parent',
      [
        { ...root, hasChildren: false },
        { ...root, id: 'leaf', parent: 'root' }
      ]
    ],
    [
      'depth',
      Array.from({ length: AIX_TREE_DEPTH_LIMIT + 1 }, (_, index) => ({
        ...root,
        id: String(index),
        parent: index ? String(index - 1) : null
      }))
    ],
    ['node count', Array.from({ length: AIX_TREE_NODE_LIMIT + 1 }, (_, index) => ({ ...root, id: String(index) }))],
    [
      'UTF-8 size',
      Array.from({ length: AIX_TREE_NODE_LIMIT }, (_, index) => ({
        ...root,
        id: String(index),
        label: '界'.repeat(128),
        description: '界'.repeat(240)
      }))
    ],
    ['untyped native action', [{ ...root, nativeCommand: 'readFile' }]]
  ])('refuses %s before rendering', async (_reason, nodes) => {
    const p = await provider()
    p.peer.postMessage({ protocolVersion: 1, type: 'aix.tree.data', id: 1, nodes })
    await waitFor(() => expect(p.tree.getSnapshot()).toEqual({ status: 'failed', busy: false, nodes: [] }))
  })

  // Catch an out-of-order provider response replacing a newer admitted action's tree.
  it('refuses a reply not correlated to the pending request', async () => {
    const p = await provider()
    await p.reply(1, [root])
    p.tree.request('refresh', null)
    p.peer.postMessage({ protocolVersion: 1, type: 'aix.tree.data', id: 1, nodes: [] })
    await waitFor(() => expect(p.tree.getSnapshot().status).toBe('failed'))
  })

  // Catch a missing provider response keeping the view busy forever.
  it('bounds a silent provider response and keeps another owner independent', async () => {
    const first = await provider(30)
    const second = await provider()
    await second.reply(1, [root])
    await waitFor(() => expect(first.tree.getSnapshot().status).toBe('failed'))
    expect(second.tree.getSnapshot()).toEqual({ status: 'ready', busy: false, nodes: [root] })
  })
})
