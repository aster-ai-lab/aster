import { MessageChannel as NodeMessageChannel } from 'node:worker_threads'

import { afterEach, describe, expect, it, vi } from 'vitest'

import type { PluginComposerDraft } from '@renderer/types/workbenchPlugin'
import { AIX_COMPOSER_TEXT_BYTES } from '@shared/types/aixComposer'

import { AixComposerProvider } from '../aixComposerProvider'
import { WorkbenchPluginHost } from '../workbenchPluginHost'

const hosts: WorkbenchPluginHost[] = []
const peers: MessagePort[] = []
const draft: PluginComposerDraft = { id: 'private-session/draft', revision: 7, text: '  Hello\n world  ' }

async function provider(timeout = 5000) {
  vi.stubGlobal('MessageChannel', NodeMessageChannel)
  const host = new WorkbenchPluginHost('composer-member')
  hosts.push(host)
  let composer!: AixComposerProvider
  await host.setPlugins([
    {
      id: 'composer',
      generation: 1,
      scopes: ['window'],
      contributions: [],
      activate: (context) => {
        composer = new AixComposerProvider(context, 'normalize', timeout)
        context.defer(() => composer.close())
      }
    }
  ])
  await host.openScope({ id: 'main', kind: 'window' })
  const channel = new MessageChannel()
  const peer = channel.port2
  peers.push(peer)
  peer.start()
  const next = () =>
    new Promise<unknown>((resolve) => peer.addEventListener('message', ({ data }) => resolve(data), { once: true }))
  const greeting = next()
  composer.connect(channel.port1)
  expect(await greeting).toEqual({ protocolVersion: 1, type: 'aix.composer.connect', surface: 'normalize' })
  return { composer, host, peer, next }
}

afterEach(async () => {
  await Promise.all(hosts.splice(0).map((host) => host.close()))
  for (const peer of peers.splice(0)) peer.close()
  vi.unstubAllGlobals()
})

describe('isolated AIX composer protocol', () => {
  // Catch draft identity disclosure, readiness bypass, or multiple results per invocation.
  it('sends only bounded text and revision after readiness and accepts one replacement', async () => {
    const p = await provider()
    const result = p.composer.invoke(draft)
    const request = p.next()
    p.peer.postMessage({ protocolVersion: 1, type: 'aix.composer.ready' })
    expect(await request).toEqual({
      protocolVersion: 1,
      type: 'aix.composer.request',
      id: 1,
      revision: 7,
      text: draft.text
    })
    p.peer.postMessage({ protocolVersion: 1, type: 'aix.composer.result', id: 1, revision: 7, text: 'Hello world' })
    await expect(result).resolves.toBe('Hello world')
    await expect(p.composer.invoke(draft)).rejects.toThrow('unavailable')
  })

  // Catch uncorrelated or oversized output, protocol switching and forged resource requests.
  it.each([
    ['revision', { protocolVersion: 1, type: 'aix.composer.result', id: 1, revision: 8, text: 'wrong' }],
    ['request', { protocolVersion: 1, type: 'aix.composer.result', id: 2, revision: 7, text: 'wrong' }],
    ['version', { protocolVersion: 2, type: 'aix.composer.result', id: 1, revision: 7, text: 'wrong' }],
    ['UTF-8 bytes', { protocolVersion: 1, type: 'aix.composer.result', id: 1, revision: 7, text: '界'.repeat(21846) }],
    [
      'extra command',
      { protocolVersion: 1, type: 'aix.composer.result', id: 1, revision: 7, text: 'wrong', command: 'readFile' }
    ],
    ['readiness replay', { protocolVersion: 1, type: 'aix.composer.ready' }],
    ['provider refusal', { protocolVersion: 1, type: 'aix.composer.refused', id: 1, revision: 7 }]
  ])('withdraws the invocation on %s', async (_name, message) => {
    const p = await provider()
    const result = expect(p.composer.invoke(draft)).rejects.toThrow()
    const request = p.next()
    p.peer.postMessage({ protocolVersion: 1, type: 'aix.composer.ready' })
    await request
    p.peer.postMessage(message)
    await result
    await expect(p.composer.invoke(draft)).rejects.toThrow('unavailable')
  })

  // Catch a result accepted before provider readiness, without the corresponding request.
  it('refuses a result before readiness', async () => {
    const p = await provider()
    const result = expect(p.composer.invoke(draft)).rejects.toThrow()
    p.peer.postMessage({ protocolVersion: 1, type: 'aix.composer.result', id: 1, revision: 7, text: 'early' })
    await result
  })

  // Catch a UTF-16 count being mistaken for bytes or invalid revision entering the guest.
  it('rejects oversized input and invalid revisions while admitting the exact byte limit', async () => {
    const p = await provider()
    for (const change of [{ text: '界'.repeat(21846) }, { revision: -1 }, { revision: Number.MAX_SAFE_INTEGER + 1 }])
      await expect(p.composer.invoke({ ...draft, ...change })).rejects.toThrow('Invalid')
    const text = 'x'.repeat(AIX_COMPOSER_TEXT_BYTES)
    const result = p.composer.invoke({ ...draft, text })
    const request = p.next()
    p.peer.postMessage({ protocolVersion: 1, type: 'aix.composer.ready' })
    expect(await request).toEqual({ protocolVersion: 1, type: 'aix.composer.request', id: 1, revision: 7, text })
    p.peer.postMessage({ protocolVersion: 1, type: 'aix.composer.result', id: 1, revision: 7, text })
    await expect(result).resolves.toBe(text)
  })

  // Catch silent providers or withdrawn publications leaving executable channels alive.
  it.each(['silent', 'withdrawn'] as const)('settles a %s provider without applying late output', async (mode) => {
    const p = await provider(30)
    const closed = new Promise<void>((resolve) => p.peer.addEventListener('close', () => resolve(), { once: true }))
    const result = expect(p.composer.invoke(draft)).rejects.toThrow('closed or timed out')
    if (mode === 'withdrawn') await p.host.setPlugins([])
    await result
    await closed
    await expect(p.composer.invoke(draft)).rejects.toThrow('unavailable')
  })
})
