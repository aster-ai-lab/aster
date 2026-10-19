import { MessageChannel as NodeMessageChannel } from 'node:worker_threads'

import { waitFor } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AIX_COMMAND_MESSAGE_LIMIT } from '@shared/types/aixCommand'

import { AixCommandProvider } from '../aixCommandProvider'
import { WorkbenchPluginHost } from '../workbenchPluginHost'

const hosts: WorkbenchPluginHost[] = []
const ports: globalThis.MessagePort[] = []

function next(peer: globalThis.MessagePort): Promise<unknown> {
  return new Promise((resolve) => peer.addEventListener('message', ({ data }) => resolve(data), { once: true }))
}

async function document(timeout = 5000) {
  vi.stubGlobal('MessageChannel', NodeMessageChannel)
  const host = new WorkbenchPluginHost('command-member')
  hosts.push(host)
  let provider!: AixCommandProvider
  const channel = new globalThis.MessageChannel()
  ports.push(channel.port1, channel.port2)
  await host.setPlugins([
    {
      id: 'commands',
      generation: 1,
      scopes: ['window'],
      contributions: [],
      activate: (context) => {
        provider = new AixCommandProvider(
          context,
          'owned-view',
          ['refresh', 'clear'],
          () => channel.port1.close(),
          timeout
        )
        context.defer(() => {
          provider.close()
          channel.port1.close()
        })
      }
    }
  ])
  const scope = await host.openScope({ id: 'main', kind: 'window' })
  channel.port1.onmessage = ({ data }) => provider.receive(data)
  channel.port1.start()
  channel.port2.start()
  const greeting = next(channel.port2)
  provider.connect(channel.port1)
  expect(await greeting).toEqual({
    protocolVersion: 1,
    type: 'aix.command.connect',
    surface: 'owned-view',
    commands: ['clear', 'refresh']
  })
  const ready = async () => {
    channel.port2.postMessage({ protocolVersion: 1, type: 'aix.command.ready' })
    await waitFor(() => expect(provider.canExecute('refresh')).toBe(true))
  }
  return { host, scope, provider, peer: channel.port2, ready }
}

afterEach(async () => {
  vi.useRealTimers()
  await Promise.all(hosts.splice(0).map((host) => host.close()))
  ports.splice(0).forEach((port) => port.close())
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('owned AIX view operations', () => {
  // Catch unready/undeclared dispatch, cross-document delivery and refusal disabling valid retries.
  it('dispatches only declared ready commands to their own document and permits a refused operation to retry', async () => {
    const left = await document()
    const right = await document()
    await expect(left.provider.execute('refresh', new globalThis.AbortController().signal)).rejects.toThrow(
      'unavailable'
    )
    await left.ready()
    await right.ready()
    await expect(left.provider.execute('workbench.delete', new globalThis.AbortController().signal)).rejects.toThrow(
      'unavailable'
    )
    const request = next(left.peer)
    const pending = left.provider.execute('refresh', new globalThis.AbortController().signal)
    const refusal = expect(pending).rejects.toThrow('refused')
    expect(await request).toEqual({ protocolVersion: 1, type: 'aix.command.execute', id: 1, command: 'refresh' })
    expect(left.provider.canExecute('clear')).toBe(false)
    expect(right.provider.canExecute('refresh')).toBe(true)
    await expect(left.provider.execute('clear', new globalThis.AbortController().signal)).rejects.toThrow('unavailable')
    left.peer.postMessage({ protocolVersion: 1, type: 'aix.command.refused', id: 1, command: 'refresh' })
    await refusal
    const retry = next(left.peer)
    const cleared = left.provider.execute('clear', new globalThis.AbortController().signal)
    expect(await retry).toEqual({ protocolVersion: 1, type: 'aix.command.execute', id: 2, command: 'clear' })
    left.peer.postMessage({ protocolVersion: 1, type: 'aix.command.completed', id: 2, command: 'clear' })
    await expect(cleared).resolves.toBeUndefined()
  })

  // Catch a valid final operation being dispatched after the host has no budget for its receipt.
  it('reserves readiness and operation receipts before sending any operation', async () => {
    const p = await document()
    let executed = 0
    p.peer.onmessage = ({ data }) => {
      if (data.type === 'aix.command.execute') {
        executed++
        p.peer.postMessage({ protocolVersion: 1, type: 'aix.command.completed', id: data.id, command: data.command })
      }
    }
    await p.ready()
    const operation = new globalThis.AbortController()
    for (let i = 0; i < AIX_COMMAND_MESSAGE_LIMIT - 1; i++) await p.provider.execute('refresh', operation.signal)
    expect(executed).toBe(AIX_COMMAND_MESSAGE_LIMIT - 1)
    expect(p.provider.canExecute('refresh')).toBe(false)
    await expect(p.provider.execute('refresh', operation.signal)).rejects.toThrow('unavailable')
    expect(executed).toBe(AIX_COMMAND_MESSAGE_LIMIT - 1)
    expect(p.provider.getSnapshot()).toEqual({ status: 'ready', busy: false })
  })

  // Catch protocol/authority fields or responses from another command settling a current operation.
  it.each([
    { protocolVersion: 2, type: 'aix.command.completed', id: 1, command: 'refresh' },
    { protocolVersion: 1, type: 'aix.command.completed', id: 2, command: 'refresh' },
    { protocolVersion: 1, type: 'aix.command.completed', id: 1, command: 'clear' },
    { protocolVersion: 1, type: 'aix.command.completed', id: 1, command: 'refresh', resource: '/private' },
    { protocolVersion: 1, type: 'aix.command.ready' }
  ])('withdraws rather than accepting an uncorrelated response: %j', async (message) => {
    const p = await document()
    await p.ready()
    const request = next(p.peer)
    const pending = expect(p.provider.execute('refresh', new globalThis.AbortController().signal)).rejects.toThrow(
      'closed'
    )
    await request
    p.peer.postMessage(message)
    await pending
    expect(p.provider.signal.aborted).toBe(true)
    expect(p.provider.getSnapshot()).toEqual({ status: 'failed', busy: false })
  })

  // Catch a withdrawn owner accepting late completion or preserving an executable command.
  it('cancels pending operations on scope withdrawal and cannot be revived by late completion', async () => {
    const p = await document()
    await p.ready()
    const request = next(p.peer)
    const pending = expect(p.provider.execute('refresh', new globalThis.AbortController().signal)).rejects.toThrow(
      'closed'
    )
    await request
    await p.scope.close()
    await pending
    p.provider.receive({ protocolVersion: 1, type: 'aix.command.completed', id: 1, command: 'refresh' })
    expect(p.provider.getSnapshot()).toEqual({ status: 'closed', busy: false })
    await expect(p.provider.execute('refresh', new globalThis.AbortController().signal)).rejects.toThrow('unavailable')
  })

  // Catch silent readiness or operation handlers hanging the document indefinitely.
  it('bounds readiness and pending operations, and observes operation cancellation', async () => {
    vi.useFakeTimers()
    const silent = await document(100)
    await vi.advanceTimersByTimeAsync(100)
    expect(silent.provider.getSnapshot().status).toBe('failed')
    vi.useRealTimers()
    const p = await document(100)
    await p.ready()
    vi.useFakeTimers()
    const timedOut = expect(p.provider.execute('refresh', new globalThis.AbortController().signal)).rejects.toThrow(
      'closed'
    )
    await vi.advanceTimersByTimeAsync(100)
    await timedOut
    vi.useRealTimers()
    const cancelled = await document()
    await cancelled.ready()
    const abort = new globalThis.AbortController()
    const pending = expect(cancelled.provider.execute('refresh', abort.signal)).rejects.toThrow('closed')
    abort.abort()
    await pending
    expect(cancelled.provider.signal.aborted).toBe(true)
  })
})
