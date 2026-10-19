import { MessageChannel as NodeMessageChannel } from 'node:worker_threads'

import { afterEach, expect, it, vi } from 'vitest'

import { AixResourceRefusedError, type AixResourceOperation } from '@shared/types/aixResource'

import { AixResourceProvider, type AixResourceCall } from '../aixResourceProvider'
import { WorkbenchPluginHost } from '../workbenchPluginHost'

const hosts: WorkbenchPluginHost[] = []
const ports: MessagePort[] = []

async function document(call: AixResourceCall) {
  vi.stubGlobal('MessageChannel', NodeMessageChannel)
  const host = new WorkbenchPluginHost('resource-member')
  hosts.push(host)
  let provider!: AixResourceProvider
  const refused = vi.fn()
  await host.setPlugins([
    {
      id: 'resources',
      generation: 1,
      scopes: ['window'],
      contributions: [],
      activate(context) {
        provider = new AixResourceProvider(context, call, refused)
        context.defer(() => provider.close())
      }
    }
  ])
  const scope = await host.openScope({ id: 'main', kind: 'window' })
  const channel = new MessageChannel()
  ports.push(channel.port1, channel.port2)
  channel.port1.onmessage = ({ data }) => provider.receive(data)
  channel.port1.start()
  channel.port2.start()
  provider.connect(channel.port1)
  return { provider, peer: channel.port2, scope, refused }
}

function send(peer: MessagePort, id: number, request: AixResourceOperation) {
  peer.postMessage({ protocolVersion: 1, type: 'aix.resource.request', id, request })
}
function next(peer: MessagePort): Promise<unknown> {
  return new Promise((resolve) => peer.addEventListener('message', ({ data }) => resolve(data), { once: true }))
}

afterEach(async () => {
  await Promise.all(hosts.splice(0).map((host) => host.close()))
  ports.splice(0).forEach((port) => port.close())
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

it('correlates resource results and refuses overlapping native operations', async () => {
  let resolve!: (value: unknown) => void
  const call = vi.fn<AixResourceCall>(
    () =>
      new Promise((done) => {
        resolve = done
      })
  )
  const doc = await document(call)
  send(doc.peer, 1, { operation: 'workspaces' })
  await vi.waitFor(() => expect(call).toHaveBeenCalledTimes(1))
  const busy = next(doc.peer)
  send(doc.peer, 2, { operation: 'workspaces' })
  expect(await busy).toEqual({ protocolVersion: 1, type: 'aix.resource.refused', id: 2, reason: 'busy' })
  const result = next(doc.peer)
  resolve([{ id: 'workspace-1', name: 'Workspace' }])
  expect(await result).toEqual({
    protocolVersion: 1,
    type: 'aix.resource.result',
    id: 1,
    value: [{ id: 'workspace-1', name: 'Workspace' }]
  })
  call.mockRejectedValueOnce(new AixResourceRefusedError('conflict'))
  const conflict = next(doc.peer)
  send(doc.peer, 3, { operation: 'workspaces' })
  expect(await conflict).toEqual({ protocolVersion: 1, type: 'aix.resource.refused', id: 3, reason: 'conflict' })
})

it('withdrawal aborts the request and suppresses a late result', async () => {
  let resolve!: (value: unknown) => void
  let signal!: AbortSignal
  const call: AixResourceCall = (_operation, abort) => {
    signal = abort
    return new Promise((done) => {
      resolve = done
    })
  }
  const doc = await document(call)
  const replies: unknown[] = []
  doc.peer.addEventListener('message', ({ data }) => replies.push(data))
  send(doc.peer, 1, { operation: 'workspaces' })
  await vi.waitFor(() => expect(signal).toBeDefined())
  await doc.scope.close()
  expect(signal.aborted).toBe(true)
  resolve('late private data')
  await new Promise((done) => setTimeout(done, 20))
  expect(replies).toEqual([])
})

it('withdraws malformed path-bearing requests before invoking the native boundary', async () => {
  const call = vi.fn<AixResourceCall>(async () => null)
  const doc = await document(call)
  doc.peer.postMessage({
    protocolVersion: 1,
    type: 'aix.resource.request',
    id: 1,
    request: { operation: 'workspaces', url: '/admin' }
  })
  await vi.waitFor(() => expect(doc.refused).toHaveBeenCalledOnce())
  expect(call).not.toHaveBeenCalled()
  send(doc.peer, 2, { operation: 'workspaces' })
  await new Promise((done) => setTimeout(done, 20))
  expect(call).not.toHaveBeenCalled()
})
