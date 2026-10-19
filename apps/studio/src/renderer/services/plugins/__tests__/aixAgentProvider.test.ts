import { MessageChannel as NodeMessageChannel } from 'node:worker_threads'

import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  AIX_AGENT_IMAGE_BYTES,
  AIX_AGENT_MESSAGE_LIMIT,
  AIX_AGENT_PROMPT_BYTES,
  AixAgentRefusedError
} from '@shared/types/aixAgent'

import { AixAgentProvider, type AixAgentTurn } from '../aixAgentProvider'
import { WorkbenchPluginHost } from '../workbenchPluginHost'

const hosts: WorkbenchPluginHost[] = []
const ports: globalThis.MessagePort[] = []

function next(peer: globalThis.MessagePort): Promise<unknown> {
  return new Promise((resolve) => peer.addEventListener('message', ({ data }) => resolve(data), { once: true }))
}

function invoke(id: number, prompt = 'step', extra: Record<string, unknown> = {}) {
  return { protocolVersion: 1, type: 'aix.agent.invoke', id, agent: 'lab', prompt, ...extra }
}

function jpeg(bytes = 64): string {
  return `data:image/jpeg;base64,${Buffer.alloc(bytes, 0xff).toString('base64')}`
}

async function document(turn: AixAgentTurn) {
  vi.stubGlobal('MessageChannel', NodeMessageChannel)
  const host = new WorkbenchPluginHost('agent-member')
  hosts.push(host)
  let provider!: AixAgentProvider
  const refused = vi.fn()
  const channel = new globalThis.MessageChannel()
  ports.push(channel.port1, channel.port2)
  await host.setPlugins([
    {
      id: 'agents',
      generation: 1,
      scopes: ['window'],
      contributions: [],
      activate: (context) => {
        provider = new AixAgentProvider(context, turn, refused)
        context.defer(() => provider.close())
      }
    }
  ])
  const scope = await host.openScope({ id: 'main', kind: 'window' })
  channel.port1.onmessage = ({ data }) => provider.receive(data)
  channel.port1.start()
  channel.port2.start()
  provider.connect(channel.port1)
  return { scope, provider, peer: channel.port2, refused }
}

afterEach(async () => {
  await Promise.all(hosts.splice(0).map((host) => host.close()))
  ports.splice(0).forEach((port) => port.close())
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('AIX view agent turns', () => {
  // Catch overlapping turns on one warm box or a lost correlation id.
  it('runs one turn at a time and answers each invoke with its own id', async () => {
    let release!: (text: string) => void
    const turn = vi.fn<AixAgentTurn>(() => new Promise((resolve) => (release = resolve)))
    const p = await document(turn)
    p.peer.postMessage(invoke(1, 'first', { parameters: { step: 1 } }))
    await vi.waitFor(() => expect(turn).toHaveBeenCalledTimes(1))
    expect(turn.mock.calls[0]?.slice(0, 3)).toEqual(['lab', 'first', { step: 1 }])
    const busy = next(p.peer)
    p.peer.postMessage(invoke(2))
    expect(await busy).toEqual({ protocolVersion: 1, type: 'aix.agent.refused', id: 2, reason: 'busy' })
    const result = next(p.peer)
    release('hold')
    expect(await result).toEqual({ protocolVersion: 1, type: 'aix.agent.result', id: 1, text: 'hold' })
    const again = next(p.peer)
    p.peer.postMessage(invoke(3))
    await vi.waitFor(() => expect(turn).toHaveBeenCalledTimes(2))
    release('move')
    expect(await again).toEqual({ protocolVersion: 1, type: 'aix.agent.result', id: 3, text: 'move' })
    expect(p.refused).not.toHaveBeenCalled()
  })

  // Catch camera frames lost between the document and the native turn.
  it('forwards named camera images with the turn', async () => {
    const turn = vi.fn<AixAgentTurn>(async () => 'hold')
    const p = await document(turn)
    const images = [
      { name: 'external', data: jpeg() },
      { name: 'wrist', data: 'data:image/png;base64,iVBORw0KGgo=' }
    ]
    const result = next(p.peer)
    p.peer.postMessage(invoke(1, 'look', { images }))
    expect(await result).toEqual({ protocolVersion: 1, type: 'aix.agent.result', id: 1, text: 'hold' })
    expect(turn.mock.calls[0]?.slice(0, 4)).toEqual(['lab', 'look', undefined, images])
  })

  // Catch host failures leaking detail or an unknown reason crossing to the frame.
  it('reduces failures to the closed refusal set', async () => {
    const turn = vi
      .fn<AixAgentTurn>()
      .mockRejectedValueOnce(new AixAgentRefusedError('capacity'))
      .mockRejectedValueOnce(new Error('stderr: /Users/secret/path'))
    const p = await document(turn)
    const capacity = next(p.peer)
    p.peer.postMessage(invoke(1))
    expect(await capacity).toEqual({ protocolVersion: 1, type: 'aix.agent.refused', id: 1, reason: 'capacity' })
    const failed = next(p.peer)
    p.peer.postMessage(invoke(2))
    expect(await failed).toEqual({ protocolVersion: 1, type: 'aix.agent.refused', id: 2, reason: 'agent_failed' })
  })

  // Catch a frame smuggling authority fields, other protocols, or oversize prompts to the host.
  it.each([
    invoke(1, 'step', { token: 'x' }),
    { ...invoke(1), protocolVersion: 2 },
    invoke(1, ''),
    invoke(1, 'x'.repeat(AIX_AGENT_PROMPT_BYTES + 1)),
    invoke(1, 'step', { agent: '../other' }),
    invoke(1, 'step', { parameters: ['not', 'an', 'object'] }),
    invoke(1, 'step', { images: [] }),
    invoke(1, 'step', { images: ['a', 'b', 'c'].map((name) => ({ name, data: jpeg() })) }),
    invoke(1, 'step', { images: ['a', 'a'].map((name) => ({ name, data: jpeg() })) }),
    invoke(1, 'step', { images: [{ name: 'External', data: jpeg() }] }),
    invoke(1, 'step', { images: [{ name: 'a', data: 'https://example.com/cube.jpg' }] }),
    invoke(1, 'step', { images: [{ name: 'a', data: 'data:image/gif;base64,R0lGODlh' }] }),
    invoke(1, 'step', { images: [{ name: 'a', data: jpeg(AIX_AGENT_IMAGE_BYTES + 64) }] }),
    invoke(1, 'step', { images: [{ name: 'a', data: jpeg(), url: 'x' }] })
  ])('withdraws the document on a malformed invoke: %#', async (message) => {
    const turn = vi.fn<AixAgentTurn>()
    const p = await document(turn)
    p.peer.postMessage(message)
    await vi.waitFor(() => expect(p.refused).toHaveBeenCalledTimes(1))
    expect(turn).not.toHaveBeenCalled()
  })

  // Catch a closed view receiving a late answer or keeping the turn running.
  it('aborts the running turn on scope withdrawal and drops its late answer', async () => {
    let signal!: globalThis.AbortSignal
    let release!: (text: string) => void
    const p = await document((_agent, _prompt, _parameters, _images, abort) => {
      signal = abort
      return new Promise((resolve) => (release = resolve))
    })
    const posted = vi.fn()
    p.peer.addEventListener('message', posted)
    p.peer.postMessage(invoke(1))
    await vi.waitFor(() => expect(signal).toBeDefined())
    await p.scope.close()
    expect(signal.aborted).toBe(true)
    release('late')
    await new Promise((resolve) => setTimeout(resolve, 20))
    expect(posted).not.toHaveBeenCalled()
  })

  // Catch an unbounded message stream on one document port.
  it('bounds the messages one document may send', async () => {
    const p = await document(async () => 'ok')
    for (let id = 1; id <= AIX_AGENT_MESSAGE_LIMIT; id++) p.provider.receive(invoke(id))
    expect(p.refused).not.toHaveBeenCalled()
    p.provider.receive(invoke(AIX_AGENT_MESSAGE_LIMIT + 1))
    expect(p.refused).toHaveBeenCalledTimes(1)
  })
})
