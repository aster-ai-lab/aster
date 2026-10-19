import { describe, expect, it, vi } from 'vitest'

import {
  agentProtocolStart,
  aixDeclarationFromUnknown,
  cloudCreateBody,
  declarationError,
  httpAixTransport,
  rendererSuppliesAgentOutput,
  runAixAgent,
  type AixAgentDeclaration,
  type AixCloudBinding,
  type AixRunEvent,
  type AixRuntimeTransport
} from '../aixAgentRuntime'

const resources = { cpuMillis: 500, memoryBytes: 268435456 }
const card = 'https://agents.example.com/.well-known/agent-card.json'

function scripted(events: AixRunEvent[], calls: string[]): AixRuntimeTransport {
  return {
    async start() {
      calls.push('start')
      return { runId: 'run-1' }
    },
    async page(_runId, after) {
      calls.push('page')
      return events.filter((event) => event.sequence > after)
    },
    async stop() {
      calls.push('stop')
    }
  }
}

describe('aix agent runtime', () => {
  it('streams an edge run and destroys it', async () => {
    const calls: string[] = []
    const outcome = await runAixAgent(
      {
        declaration: { runtime: 'edge', harness: 'builtin', resources },
        text: '审查这份合同'
      },
      scripted(
        [
          { sequence: 1, type: 'text_delta', text: '第一条' },
          { sequence: 2, type: 'text_delta', text: '通过' },
          { sequence: 3, type: 'completed' }
        ],
        calls
      )
    )
    expect(outcome).toEqual({ delivery: 'stream', text: '第一条通过', destroyed: true })
    expect(calls).toEqual(['start', 'page', 'stop'])
  })

  it('returns a cloud run in one shot and destroys it', async () => {
    const calls: string[] = []
    const outcome = await runAixAgent(
      {
        declaration: { runtime: 'cloud', harness: 'package', entry: 'agent/run.mjs', resources },
        text: '上海天气',
        parameters: { city: '上海' }
      },
      scripted(
        [
          { sequence: 1, type: 'output', text: '22°C' },
          { sequence: 2, type: 'completed' }
        ],
        calls
      )
    )
    expect(outcome.delivery).toBe('once')
    expect(outcome.text).toBe('22°C')
    expect(calls.at(-1)).toBe('stop')
  })

  it('sends a remote form to the agent card and destroys the run', async () => {
    const calls: string[] = []
    const outcome = await runAixAgent(
      { declaration: { runtime: 'remote', card }, text: '上海天气', parameters: { city: '上海' } },
      scripted(
        [
          { sequence: 1, type: 'output', text: '多云' },
          { sequence: 2, type: 'completed' }
        ],
        calls
      )
    )
    expect(outcome.text).toBe('多云')
    expect(calls).toEqual(['start', 'page', 'stop'])
  })

  it('destroys the run when the page fails', async () => {
    const calls: string[] = []
    const transport: AixRuntimeTransport = {
      async start() {
        calls.push('start')
        return { runId: 'run-1' }
      },
      async page() {
        throw new Error('page failed')
      },
      async stop() {
        calls.push('stop')
      }
    }
    await expect(
      runAixAgent({ declaration: { runtime: 'edge', harness: 'builtin', resources }, text: '审查' }, transport)
    ).rejects.toThrow('page failed')
    expect(calls).toEqual(['start', 'stop'])
  })

  it('refuses a mismatched declaration before start', async () => {
    const start = vi.fn()
    const transport: AixRuntimeTransport = {
      start,
      async page() {
        return []
      },
      async stop() {}
    }
    const refused: AixAgentDeclaration[] = [
      { runtime: 'remote', card, harness: 'builtin' },
      { runtime: 'edge', harness: 'builtin', resources, card },
      { runtime: 'cloud', harness: 'package', resources },
      { runtime: 'remote' }
    ]
    for (const declaration of refused) {
      await expect(runAixAgent({ declaration, text: '提交' }, transport)).rejects.toThrow()
    }
    expect(start).not.toHaveBeenCalled()
  })

  it('keeps a valid declaration from the catalog row', () => {
    expect(aixDeclarationFromUnknown({ runtime: 'edge', harness: 'builtin', resources, card: 'http://bad' })).toBeNull()
    expect(aixDeclarationFromUnknown({ runtime: 'remote', card })).toEqual({ runtime: 'remote', card })
    expect(declarationError({ runtime: 'edge', harness: 'builtin', resources })).toBeNull()
  })

  it('posts a remote task to the card and an edge command to the runtime', async () => {
    const seen: { url: string; body: string }[] = []
    const fetchImpl = async (url: string, init: { method: string; body: string }) => {
      seen.push({ url, body: init.body })
      if (url.startsWith('https://agents.example.com')) {
        return { ok: true, json: async () => ({ result: { text: '多云' } }) }
      }
      return {
        ok: true,
        json: async () => ({
          events: [
            { sequence: 1, type: 'text_delta', text: '好' },
            { sequence: 2, type: 'completed' }
          ]
        })
      }
    }
    const remote = httpAixTransport({}, fetchImpl)
    const remoteOutcome = await runAixAgent(
      { declaration: { runtime: 'remote', card }, text: '天气', parameters: { city: '上海' } },
      remote
    )
    expect(remoteOutcome).toMatchObject({ delivery: 'once', text: '多云', destroyed: true })
    expect(seen[0]?.url).toBe(card)
    expect(JSON.parse(seen[0]?.body ?? '{}').method).toBe('tasks/send')

    const edge = httpAixTransport({ edge: 'http://127.0.0.1:9' }, fetchImpl)
    const edgeOutcome = await runAixAgent(
      { declaration: { runtime: 'edge', harness: 'builtin', resources }, text: '审查' },
      edge
    )
    expect(edgeOutcome.delivery).toBe('stream')
    expect(seen[1]?.url).toBe('http://127.0.0.1:9/v1/agent/commands')
    expect(
      agentProtocolStart({ declaration: { runtime: 'edge', harness: 'builtin', resources }, text: '审查' }, 'id').schema
    ).toBe('a3s.code.agent-run-start.v1')
    await expect(
      runAixAgent(
        { declaration: { runtime: 'cloud', harness: 'builtin', resources }, text: '审查' },
        httpAixTransport({}, fetchImpl)
      )
    ).rejects.toThrow('not bound to an asset release')
    expect(seen).toHaveLength(2)
  })

  it('creates a bound cloud run and stops it', async () => {
    const seen: { url: string; body?: string }[] = []
    const binding: AixCloudBinding = {
      packageId: 'use/a3s/weather',
      digest: `sha256:${'ab'.padStart(64, '0')}`,
      agentAssetId: '11111111-1111-4111-8111-111111111111',
      agentAssetReleaseId: '22222222-2222-4222-8222-222222222222'
    }
    const fetchImpl = async (url: string, init: { method: string; body?: string }) => {
      seen.push({ url, body: init.body })
      if (init.method === 'POST' && url.endsWith('/agent-runtimes')) {
        const body = JSON.parse(init.body ?? '{}') as { agentAssetId?: string }
        expect(body.agentAssetId).toBe(binding.agentAssetId)
        expect(body.agentAssetId).not.toBe('weather')
        return {
          ok: true,
          json: async () => ({ projection: { agentExecutionId: '33333333-3333-4333-8333-333333333333' } })
        }
      }
      if (init.method === 'GET') return { ok: true, json: async () => ({ text: '22°C' }) }
      return { ok: true, json: async () => ({}) }
    }
    const outcome = await runAixAgent(
      {
        declaration: { runtime: 'cloud', harness: 'package', entry: 'agent/run.mjs', resources },
        text: '上海天气',
        parameters: { city: '上海' },
        cloudBinding: binding,
        conversationId: '44444444-4444-4444-8444-444444444444',
        installSlug: 'weather'
      },
      httpAixTransport({ cloud: 'http://127.0.0.1:9' }, fetchImpl)
    )
    expect(outcome).toMatchObject({ delivery: 'once', text: '22°C', destroyed: true })
    expect(seen.at(-1)?.url).toBe('http://127.0.0.1:9/agent-runtimes/33333333-3333-4333-8333-333333333333/stop')
    expect(() =>
      cloudCreateBody({
        declaration: { runtime: 'cloud', harness: 'package', entry: 'agent/run.mjs', resources },
        text: 'x',
        installSlug: 'weather',
        cloudBinding: { ...binding, agentAssetId: 'weather' }
      })
    ).toThrow('not bound to an asset release')
  })

  it('leaves a bound cloud turn on the host when the renderer has no cloud base', async () => {
    const calls: string[] = []
    const binding: AixCloudBinding = {
      packageId: 'use/a3s/weather',
      digest: `sha256:${'ab'.padStart(64, '0')}`,
      agentAssetId: '11111111-1111-4111-8111-111111111111',
      agentAssetReleaseId: '22222222-2222-4222-8222-222222222222'
    }
    const text = await rendererSuppliesAgentOutput(
      {
        declaration: { runtime: 'cloud', harness: 'package', entry: 'agent/run.mjs', resources },
        text: '上海天气',
        cloudBinding: binding,
        conversationId: '44444444-4444-4444-8444-444444444444',
        installSlug: 'weather'
      },
      httpAixTransport({}, async (url) => {
        calls.push(url)
        throw new Error('renderer posted')
      })
    )
    expect(text).toBeUndefined()
    expect(calls).toEqual([])
  })

  it('does not fetch a remote card from the renderer send path', async () => {
    const calls: string[] = []
    const text = await rendererSuppliesAgentOutput(
      { declaration: { runtime: 'remote', card }, text: '天气' },
      scripted(
        [
          { sequence: 1, type: 'output', text: '多云' },
          { sequence: 2, type: 'completed' }
        ],
        calls
      )
    )
    expect(text).toBeUndefined()
    expect(calls).toEqual([])
  })

  it('cancels a remote task and still destroys the run', async () => {
    const calls: string[] = []
    const controller = new AbortController()
    const transport: AixRuntimeTransport = {
      async start() {
        calls.push('start')
        controller.abort()
        return { runId: 'run-1' }
      },
      async page() {
        return []
      },
      async cancel() {
        calls.push('cancel')
      },
      async stop() {
        calls.push('stop')
      }
    }
    await expect(
      runAixAgent({ declaration: { runtime: 'remote', card }, text: '天气' }, transport, controller.signal)
    ).rejects.toThrow('cancelled')
    expect(calls).toEqual(['start', 'cancel', 'stop'])
  })
})
